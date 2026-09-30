#![no_std]
#![allow(clippy::needless_pass_by_value)]

extern crate alloc;

use crate::fixed_mul_floor::fixed_mul_floor;
use alloc::string::ToString;
use soroban_sdk::{
    Address, BytesN, ContractExecutable, Env, Map, String, Vec, contract, contractimpl,
    contracttype,
};

use admin::require_admin;

use crate::admin::set_admin;
use crate::admin::traits::Admin;
use crate::membership::require_members;
pub use crate::neural_governance::LayerAggregator;
use crate::neural_governance::traits::Governance;
use crate::neural_governance::{Layer, NGQ, Neuron, aggregate_result};
use crate::storage::{
    LayerKeyData, NeuronKeyData, NeuronResultKeyData, VotingPowersKeyData, read_layer,
    read_membership_contract, read_neural_governance, read_neuron, read_neuron_result,
    read_voting_powers, remove_layer, remove_neuron, write_layer, write_membership_contract,
    write_neural_governance, write_neuron, write_neuron_result, write_voting_powers,
};
use crate::types::VotingSystemError;

mod admin;
mod fixed_mul_floor;
mod membership;
mod neural_governance;
mod storage;
pub mod types;

/// Neuron results, weights and voting powers are `i64` fixed point numbers
/// with 6 decimals: 1.0 is `1_000_000`, the largest value about 9.2 trillion.
pub const DECIMALS: i64 = 1_000_000;

#[contract]
pub struct VotingSystem;

type ContractResult<T> = Result<T, VotingSystemError>;

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub enum DataKey {
    /// storage type: instance
    NeuralGovernance,
    /// storage type: instance
    /// u32
    CurrentLayerId,
    Admin,
    /// u32
    CurrentRound,
    /// storage type: instance
    /// Address of the Stellar Membership contract whose token ids identify voters
    MembershipContract,
    NeuronKey(NeuronKeyData),
    NeuronResultKey(NeuronResultKeyData),
    LayerKey(LayerKeyData),
    VotingPowers(VotingPowersKeyData),
}

#[contractimpl]
impl VotingSystem {
    /// Initialize the governance contract.
    ///
    /// # Arguments
    ///
    /// * `admin`: account allowed to configure the contract and upload neuron results.
    /// * `current_round`: the active voting round.
    /// * `membership_contract`: Stellar Membership contract whose token ids identify voters.
    pub fn __constructor(
        env: Env,
        admin: Address,
        current_round: u32,
        membership_contract: Address,
    ) {
        set_admin(&env, &admin);
        write_membership_contract(&env, &membership_contract);

        let neural_governance = NGQ::new(&env);
        env.storage()
            .instance()
            .set(&DataKey::CurrentRound, &current_round);
        write_neural_governance(&env, neural_governance);
    }

    /// Get the current active round.
    pub fn get_current_round(env: &Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::CurrentRound)
            .unwrap()
    }

    /// Change the active round.
    pub fn set_current_round(env: Env, round: u32) {
        require_admin(&env);

        env.storage().instance().set(&DataKey::CurrentRound, &round);
    }

    /// Get the Stellar Membership contract whose token ids identify voters.
    pub fn get_membership_contract(env: &Env) -> Address {
        read_membership_contract(env)
    }

    /// Change the Stellar Membership contract. Admin only.
    ///
    /// Voters are validated against it on every upload, so a wrong address
    /// makes every upload fail with `NotAMember` rather than letting unknown
    /// ids through.
    pub fn set_membership_contract(env: Env, membership_contract: Address) {
        require_admin(&env);

        write_membership_contract(&env, &membership_contract);
    }

    /// Get the voting power (NQG score) of a member for the active round,
    /// with 6 decimals.
    ///
    /// # Arguments
    ///
    /// * `member_id`: the member's Stellar Membership token id.
    pub fn get_voting_power_for_user(env: Env, member_id: u32) -> Result<i64, VotingSystemError> {
        match read_voting_powers(&env, Self::get_current_round(&env)) {
            Ok(voting_powers) => {
                if let Some(voting_power) = voting_powers.get(member_id) {
                    return Ok(voting_power);
                }
                Err(VotingSystemError::NGQResultForVoterMissing)
            }
            Err(_) => Err(VotingSystemError::UnknownError),
        }
    }
}

#[contractimpl]
impl Admin for VotingSystem {
    fn transfer_admin(env: Env, new_admin: Address) {
        require_admin(&env);

        set_admin(&env, &new_admin);
    }

    fn upgrade(env: Env, wasm_hash: BytesN<32>) {
        require_admin(&env);

        env.deployer()
            .update_current_contract(ContractExecutable::Wasm(wasm_hash));
    }
}

#[contractimpl]
impl Governance for VotingSystem {
    fn add_layer(
        env: Env,
        raw_neurons: Vec<(String, i64)>,
        layer_aggregator: LayerAggregator,
    ) -> Result<(), VotingSystemError> {
        require_admin(&env);

        let layer_id = next_layer_id(&env);
        let layer_id = String::from_str(&env, layer_id.to_string().as_str());

        create_or_update_layer(env, layer_id, raw_neurons, layer_aggregator);

        Ok(())
    }

    fn remove_layer(env: Env, layer_id: String) -> Result<(), VotingSystemError> {
        require_admin(&env);

        let mut neural_governance = read_neural_governance(&env).unwrap();
        let index = neural_governance
            .layers
            .iter()
            .position(|id| id == layer_id)
            .ok_or(VotingSystemError::LayerMissing)?;
        let layer = read_layer(&env, &layer_id)?;

        for neuron_id in layer.neurons {
            remove_neuron(&env, &layer_id, &neuron_id);
        }
        remove_layer(&env, &layer_id);

        neural_governance
            .layers
            .remove(u32::try_from(index).unwrap());

        write_neural_governance(&env, neural_governance);

        Ok(())
    }

    fn get_layer(env: Env, layer_id: String) -> Result<Layer, VotingSystemError> {
        read_layer(&env, &layer_id)
    }

    fn get_neuron(
        env: Env,
        layer_id: String,
        neuron_id: String,
    ) -> Result<Neuron, VotingSystemError> {
        read_neuron(&env, &layer_id, &neuron_id)
    }

    fn get_neuron_result_round(
        env: &Env,
        layer_id: String,
        neuron_id: String,
        round: u32,
    ) -> Result<Map<u32, i64>, VotingSystemError> {
        read_neuron_result(env, &layer_id, &neuron_id, round)
    }

    fn get_neuron_result(
        env: &Env,
        layer_id: String,
        neuron_id: String,
    ) -> Result<Map<u32, i64>, VotingSystemError> {
        Self::get_neuron_result_round(env, layer_id, neuron_id, Self::get_current_round(env))
    }

    fn set_neuron_result(
        env: Env,
        layer_id: String,
        neuron_id: String,
        result: Map<u32, i64>,
    ) -> Result<(), VotingSystemError> {
        require_admin(&env);
        require_members(&env, &result)?;

        write_neuron_result(
            &env,
            &layer_id,
            &neuron_id,
            Self::get_current_round(&env),
            &result,
        );
        Ok(())
    }

    /// Get a result of a whole layer
    ///
    /// Gets a result of each neuron and aggregates them using a configured aggregator function
    fn get_layer_result(env: Env, layer_id: String) -> Result<Map<u32, i64>, VotingSystemError> {
        let layer = read_layer(&env, &layer_id)?;
        let mut result: Map<u32, Vec<i64>> = Map::new(&env);

        for neuron_id in layer.neurons {
            let neuron_result = Self::get_neuron_result(&env, layer_id.clone(), neuron_id.clone())?;
            let neuron = read_neuron(&env, &layer_id, &neuron_id)?;
            let neuron_result = weigh_neuron_result(&env, neuron.weight, neuron_result)?;
            for (user, new) in neuron_result {
                let mut previous = result.get(user).unwrap_or_else(|| Vec::new(&env));
                previous.push_back(new);
                result.set(user, previous);
            }
        }

        aggregate_result(&env, result, layer.aggregator, DECIMALS)
    }

    fn calculate_voting_powers(env: Env) -> Result<(), VotingSystemError> {
        require_admin(&env);

        let neural_governance = read_neural_governance(&env).unwrap();
        let layers = neural_governance.layers;
        let layer_count = layers.len();
        let mut result: Map<u32, i64> = Map::new(&env);
        let mut expected_users = 0u32;

        for (layer_idx, layer_id) in (0u32..).zip(layers.iter()) {
            let layer_result = VotingSystem::get_layer_result(env.clone(), layer_id)?;
            if layer_idx == 0 {
                expected_users = layer_result.len();
            } else if layer_result.len() != expected_users {
                return Err(VotingSystemError::LayerResultsUsersMismatch);
            }
            let is_last_layer = layer_idx + 1 == layer_count;
            for (key, value) in layer_result.iter() {
                let summed = value
                    .checked_add(result.get(key).unwrap_or(0))
                    .ok_or(VotingSystemError::ArithmeticOverflow)?;
                let voting_power = if is_last_layer { summed.max(0) } else { summed };
                result.set(key, voting_power);
            }
        }

        if result.len() != expected_users {
            return Err(VotingSystemError::LayerResultsUsersMismatch);
        }

        write_voting_powers(&env, Self::get_current_round(&env), &result);
        Ok(())
    }

    fn get_voting_powers(env: Env) -> Result<Map<u32, i64>, VotingSystemError> {
        read_voting_powers(&env, Self::get_current_round(&env))
    }

    /// Get a current neural governance setup
    fn get_neural_governance(env: &Env) -> Result<NGQ, VotingSystemError> {
        read_neural_governance(env)
    }
}

fn create_or_update_layer(
    env: Env,
    layer_id: String,
    raw_neurons: Vec<(String, i64)>,
    layer_aggregator: LayerAggregator,
) {
    let mut neural_governance = read_neural_governance(&env).unwrap();

    let mut neurons = Vec::new(&env);

    for (neuron_id_raw, (name, weight)) in raw_neurons.into_iter().enumerate() {
        let neuron_id = String::from_str(&env, neuron_id_raw.to_string().as_str());

        let neuron_details = Neuron::create(name, weight);
        write_neuron(&env, &layer_id, &neuron_id, &neuron_details);

        neurons.push_back(neuron_id);
    }

    let layer = Layer::create(neurons, layer_aggregator);
    write_layer(&env, &layer_id, &layer);

    neural_governance.layers.push_back(layer_id);
    write_neural_governance(&env, neural_governance);
}

fn weigh_neuron_result(
    env: &Env,
    weight: i64,
    result: Map<u32, i64>,
) -> Result<Map<u32, i64>, VotingSystemError> {
    let mut scaled = Map::new(env);

    for (key, value) in result {
        let weighted = fixed_mul_floor(value, weight, DECIMALS)
            .ok_or(VotingSystemError::ArithmeticOverflow)?;
        scaled.set(key, weighted);
    }

    Ok(scaled)
}

fn next_layer_id(env: &Env) -> u32 {
    let id: u32 = env
        .storage()
        .instance()
        .get(&DataKey::CurrentLayerId)
        .unwrap_or(0);
    env.storage()
        .instance()
        .set(&DataKey::CurrentLayerId, &(id + 1));
    id
}
