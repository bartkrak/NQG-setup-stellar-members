use crate::neural_governance::{Layer, LayerAggregator, NGQ, Neuron};
use crate::types::{MemberId, VotingSystemError};
use soroban_sdk::{Env, I256, Map, String, Vec};

pub trait Governance {
    /// Add a new layer to the contract.
    ///
    /// # Arguments
    ///
    /// * `raw_neurons`: tuples of neuron names and their respective weights.
    /// * `layer_aggregator`: a function used to aggregate the neuron results within the layer.
    fn add_layer(
        env: Env,
        raw_neurons: Vec<(String, I256)>,
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
    ) -> Result<Map<MemberId, I256>, VotingSystemError>;

    /// Get a map of member ids and their voting powers for a neuron for the active round.
    fn get_neuron_result(
        env: &Env,
        layer_id: String,
        neuron_id: String,
    ) -> Result<Map<MemberId, I256>, VotingSystemError>;

    /// Set neuron result for the active round.
    ///
    /// Every key must be an active member of the Stellar Membership contract:
    /// `NotAMember` otherwise, and nothing is written.
    fn set_neuron_result(
        env: Env,
        layer_id: String,
        neuron_id: String,
        result: Map<MemberId, I256>,
    ) -> Result<(), VotingSystemError>;

    /// Get a map of member ids and their voting powers for a layer for the active round.
    fn get_layer_result(
        env: Env,
        layer_id: String,
    ) -> Result<Map<MemberId, I256>, VotingSystemError>;

    /// Calculate final voting powers for the active round and write them to contract storage.
    fn calculate_voting_powers(env: Env) -> Result<(), VotingSystemError>;

    /// Get a map of member ids and their voting powers for whole governance for the active round.
    fn get_voting_powers(env: Env) -> Result<Map<MemberId, I256>, VotingSystemError>;

    /// Get a representation of the current NGQ setup.
    fn get_neural_governance(env: &Env) -> Result<NGQ, VotingSystemError>;
}
