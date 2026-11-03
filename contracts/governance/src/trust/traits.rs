use crate::trust::TrustList;
use crate::types::VotingSystemError;
use soroban_sdk::{BytesN, Env};

/// Encrypted trust lists, one per member, and the operator key they are
/// sealed for. The contract cannot read a list: it checks who writes it, its
/// size, and that the operator's copy of its key is for the current key.
pub trait TrustLists {
    /// Set the operator's X25519 public key, which every trust list is
    /// sealed for besides its member. Admin only.
    fn set_trust_key(env: Env, key: BytesN<32>);

    /// Get the operator's X25519 public key.
    fn get_trust_key(env: Env) -> Result<BytesN<32>, VotingSystemError>;

    /// Store the encrypted trust list of a member, replacing the previous one.
    ///
    /// Requires the signature of the address holding `member_id` on the
    /// membership contract, which must be active.
    ///
    /// # Arguments
    ///
    /// * `member_id`: the member's Stellar Membership token id.
    /// * `trust_list`: the encrypted list, at most `MAX_TRUST_LIST_SIZE`
    ///   bytes, with its key sealed for the member and for the operator key.
    fn set_trust_list(
        env: Env,
        member_id: u32,
        trust_list: TrustList,
    ) -> Result<(), VotingSystemError>;

    /// Get the encrypted trust list of a member, if one was stored.
    fn get_trust_list(env: Env, member_id: u32) -> Option<TrustList>;
}
