use soroban_sdk::contracterror;

/// Error codes are kept stable across versions: variants removed with the
/// voting features leave gaps rather than renumbering the rest.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum VotingSystemError {
    UnknownError = 0,
    NeuralGovernanceNotSet = 1,
    UnexpectedValue = 3,
    NeuronResultNotSet = 7,
    InvalidLayerAggregator = 8,
    LayerMissing = 9,
    NeuronMissing = 10,
    NGQResultForVoterMissing = 11,
    VotingPowersNotSet = 15,
    LayerResultsUsersMismatch = 17,
    /// A weighted, aggregated or summed value does not fit an `i64`.
    ArithmeticOverflow = 19,
    /// The verifier contract did not accept the proof of a neuron result.
    InvalidProof = 20,
    NeuronProofNotSet = 21,
}
