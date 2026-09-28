use soroban_sdk::{Address, Env, I256, Map, String};

use crate::neural_governance::{Layer, NGQ, Neuron};
use crate::storage::key_data::{
    get_layer_key, get_neuron_key, get_neuron_result_key, get_voting_powers_key,
};
use crate::types::VotingSystemError;
use crate::{ContractResult, DataKey};

pub use crate::storage::key_data::{
    LayerKeyData, NeuronKeyData, NeuronResultKeyData, VotingPowersKeyData,
};

mod key_data;

pub(crate) fn read_layer(env: &Env, layer_id: &String) -> ContractResult<Layer> {
    let key = get_layer_key(layer_id);
    env.storage()
        .instance()
        .get(&key)
        .ok_or(VotingSystemError::LayerMissing)
}

pub(crate) fn write_layer(env: &Env, layer_id: &String, layer: &Layer) {
    let key = get_layer_key(layer_id);
    env.storage().instance().set(&key, layer);
}

pub(crate) fn remove_layer(env: &Env, layer_id: &String) {
    let key = get_layer_key(layer_id);
    env.storage().instance().remove(&key);
}

pub(crate) fn read_neuron(
    env: &Env,
    layer_id: &String,
    neuron_id: &String,
) -> ContractResult<Neuron> {
    let key = get_neuron_key(layer_id, neuron_id);
    env.storage()
        .instance()
        .get(&key)
        .ok_or(VotingSystemError::NeuronMissing)
}

pub(crate) fn write_neuron(env: &Env, layer_id: &String, neuron_id: &String, neuron: &Neuron) {
    let key = get_neuron_key(layer_id, neuron_id);
    env.storage().instance().set(&key, neuron);
}

pub(crate) fn remove_neuron(env: &Env, layer_id: &String, neuron_id: &String) {
    let key = get_neuron_key(layer_id, neuron_id);
    env.storage().instance().remove(&key);
}

pub(crate) fn read_neuron_result(
    env: &Env,
    layer_id: &String,
    neuron_id: &String,
    round: u32,
) -> ContractResult<Map<u32, I256>> {
    let key = get_neuron_result_key(layer_id, neuron_id, round);
    env.storage()
        .temporary()
        .get(&key)
        .ok_or(VotingSystemError::NeuronResultNotSet)
}

pub(crate) fn write_neuron_result(
    env: &Env,
    layer_id: &String,
    neuron_id: &String,
    round: u32,
    result: &Map<u32, I256>,
) {
    let key = get_neuron_result_key(layer_id, neuron_id, round);
    env.storage().temporary().set(&key, result);
}

pub(crate) fn read_neural_governance(env: &Env) -> ContractResult<NGQ> {
    env.storage()
        .instance()
        .get(&DataKey::NeuralGovernance)
        .ok_or(VotingSystemError::NeuralGovernanceNotSet)
}

pub(crate) fn write_neural_governance(env: &Env, neural_governance: NGQ) {
    env.storage()
        .instance()
        .set(&DataKey::NeuralGovernance, &neural_governance);
}

/// Set by the constructor, so always present.
pub(crate) fn read_membership_contract(env: &Env) -> Address {
    env.storage()
        .instance()
        .get(&DataKey::MembershipContract)
        .unwrap()
}

pub(crate) fn write_membership_contract(env: &Env, membership_contract: &Address) {
    env.storage()
        .instance()
        .set(&DataKey::MembershipContract, membership_contract);
}

pub(crate) fn read_voting_powers(env: &Env, round: u32) -> ContractResult<Map<u32, I256>> {
    let key = get_voting_powers_key(round);
    env.storage()
        .persistent()
        .get(&key)
        .ok_or(VotingSystemError::VotingPowersNotSet)
}

pub(crate) fn write_voting_powers(env: &Env, round: u32, voting_powers: &Map<u32, I256>) {
    let key = get_voting_powers_key(round);
    env.storage().persistent().set(&key, voting_powers);
}
