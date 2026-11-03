use soroban_sdk::{Address, BytesN, Env, Map, String};

use crate::neural_governance::{Layer, NGQ, Neuron};
use crate::storage::key_data::{
    get_layer_key, get_neuron_key, get_neuron_proof_key, get_neuron_result_key,
    get_voting_powers_key,
};
use crate::trust::TrustList;
use crate::types::VotingSystemError;
use crate::verifier::NeuronProof;
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
) -> ContractResult<Map<u32, i64>> {
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
    result: &Map<u32, i64>,
) {
    let key = get_neuron_result_key(layer_id, neuron_id, round);
    env.storage().temporary().set(&key, result);
}

pub(crate) fn read_neuron_proof(
    env: &Env,
    layer_id: &String,
    neuron_id: &String,
    round: u32,
) -> ContractResult<NeuronProof> {
    let key = get_neuron_proof_key(layer_id, neuron_id, round);
    env.storage()
        .persistent()
        .get(&key)
        .ok_or(VotingSystemError::NeuronProofNotSet)
}

pub(crate) fn write_neuron_proof(
    env: &Env,
    layer_id: &String,
    neuron_id: &String,
    round: u32,
    proof: &NeuronProof,
) {
    let key = get_neuron_proof_key(layer_id, neuron_id, round);
    env.storage().persistent().set(&key, proof);
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

/// Set by the constructor, so always present.
pub(crate) fn read_verifier(env: &Env) -> Address {
    env.storage().instance().get(&DataKey::Verifier).unwrap()
}

pub(crate) fn write_verifier(env: &Env, verifier: &Address) {
    env.storage().instance().set(&DataKey::Verifier, verifier);
}

pub(crate) fn read_trust_key(env: &Env) -> ContractResult<BytesN<32>> {
    env.storage()
        .instance()
        .get(&DataKey::TrustKey)
        .ok_or(VotingSystemError::TrustKeyNotSet)
}

pub(crate) fn write_trust_key(env: &Env, key: &BytesN<32>) {
    env.storage().instance().set(&DataKey::TrustKey, key);
}

pub(crate) fn read_trust_list(env: &Env, member_id: u32) -> Option<TrustList> {
    env.storage()
        .persistent()
        .get(&DataKey::TrustList(member_id))
}

/// Written lists live as long as the network allows: they are read again at
/// every round, not only the one they were written in.
pub(crate) fn write_trust_list(env: &Env, member_id: u32, trust_list: &TrustList) {
    let key = DataKey::TrustList(member_id);
    let max_ttl = env.storage().max_ttl();
    env.storage().persistent().set(&key, trust_list);
    env.storage()
        .persistent()
        .extend_ttl(&key, max_ttl, max_ttl);
}

pub(crate) fn read_voting_powers(env: &Env, round: u32) -> ContractResult<Map<u32, i64>> {
    let key = get_voting_powers_key(round);
    env.storage()
        .persistent()
        .get(&key)
        .ok_or(VotingSystemError::VotingPowersNotSet)
}

pub(crate) fn write_voting_powers(env: &Env, round: u32, voting_powers: &Map<u32, i64>) {
    let key = get_voting_powers_key(round);
    env.storage().persistent().set(&key, voting_powers);
}
