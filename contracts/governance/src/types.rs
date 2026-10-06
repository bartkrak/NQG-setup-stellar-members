use soroban_sdk::contracterror;

// Voters are identified by their Stellar Membership token id, a plain `u32`
// (`member_id` in signatures). The membership contract assigns token ids
// sequentially at mint and keeps them across key rotations and recoveries,
// so voting power follows the person rather than the key.
//
// Do not introduce a `type MemberId = u32` alias: the contract spec
// generator does not resolve aliases, so it would export an undefined
// `MemberId` type and break the stellar CLI and generated bindings.

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
    /// A voter id is not an active Stellar Membership token.
    NotAMember = 18,
    /// A weighted, aggregated or summed value does not fit an `i64`.
    ArithmeticOverflow = 19,
}
