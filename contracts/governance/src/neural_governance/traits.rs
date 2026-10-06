use crate::neural_governance::{Layer, LayerAggregator, NGQ, Neuron};
use crate::types::VotingSystemError;
use crate::verifier::{NeuronGuest, NeuronProof};
use soroban_sdk::{Bytes, Env, Map, String, Vec};

pub trait Governance {
    /// Add a new layer to the contract.
    ///
    /// # Arguments
    ///
    /// * `raw_neurons`: tuples of neuron names and their weights, 6 decimals (1.0 is `1_000_000`).
    /// * `layer_aggregator`: a function used to aggregate the neuron results within the layer.
    fn add_layer(
        env: Env,
        raw_neurons: Vec<(String, i64)>,
        layer_aggregator: LayerAggregator,
    ) -> Result<(), VotingSystemError>;

    /// Remove a layer from the contract
    ///
    /// # Arguments
    ///
    /// * `layer_id`: ID of the layer to remove
    fn remove_layer(env: Env, layer_id: String) -> Result<(), VotingSystemError>;

    // TODO docs
    fn get_layer(env: Env, layer_id: String) -> Result<Layer, VotingSystemError>;

    // TODO docs
    fn get_neuron(
        env: Env,
        layer_id: String,
        neuron_id: String,
    ) -> Result<Neuron, VotingSystemError>;

    /// Get a map of member ids and their voting powers for a neuron for a specific round.
    fn get_neuron_result_round(
        env: &Env,
        layer_id: String,
        neuron_id: String,
        round: u32,
    ) -> Result<Map<u32, i64>, VotingSystemError>;

    /// Get a map of member ids and their voting powers for a neuron for the active round.
    fn get_neuron_result(
        env: &Env,
        layer_id: String,
        neuron_id: String,
    ) -> Result<Map<u32, i64>, VotingSystemError>;

    /// Get the proof stored with a neuron result for a specific round.
    fn get_neuron_proof_round(
        env: &Env,
        layer_id: String,
        neuron_id: String,
        round: u32,
    ) -> Result<NeuronProof, VotingSystemError>;

    /// Get the proof stored with a neuron result for the active round.
    fn get_neuron_proof(
        env: &Env,
        layer_id: String,
        neuron_id: String,
    ) -> Result<NeuronProof, VotingSystemError>;

    /// Set neuron result for the active round, with the zkvm proof of the
    /// guest that computed it.
    ///
    /// Every key must be an active member of the Stellar Membership contract:
    /// `NotAMember` otherwise. Then the verifier contract must accept `seal`
    /// for `guest`'s Image ID and the SHA-256 of `journal`: `InvalidProof`
    /// otherwise. Nothing is written unless both pass; then the result and
    /// the proof are stored, replacing any earlier upload for this neuron
    /// and round.
    ///
    /// `result` is not compared with the journal: the proof shows what the
    /// guest committed, not that `result` is it.
    ///
    /// # Arguments
    ///
    /// * `guest`: the zkvm guest that computed the result; selects the Image ID.
    /// * `journal`: the guest's public output as committed (`journal` of `host prove`).
    /// * `seal`: the 4-byte verifier selector and the Groth16 seal (`seal` of `host prove`).
    fn set_neuron_result(
        env: Env,
        layer_id: String,
        neuron_id: String,
        result: Map<u32, i64>,
        guest: NeuronGuest,
        journal: Bytes,
        seal: Bytes,
    ) -> Result<(), VotingSystemError>;

    /// Get a map of member ids and their voting powers for a layer for the active round.
    fn get_layer_result(env: Env, layer_id: String) -> Result<Map<u32, i64>, VotingSystemError>;

    /// Calculate final voting powers for the active round and write them to contract storage.
    fn calculate_voting_powers(env: Env) -> Result<(), VotingSystemError>;

    /// Get a map of member ids and their voting powers for whole governance for the active round.
    fn get_voting_powers(env: Env) -> Result<Map<u32, i64>, VotingSystemError>;

    /// Get a representation of the current NGQ setup.
    fn get_neural_governance(env: &Env) -> Result<NGQ, VotingSystemError>;
}
