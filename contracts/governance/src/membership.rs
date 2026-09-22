//! The link to the Stellar Membership contract, whose token ids are the
//! voter identifiers of this contract.

use soroban_sdk::{Address, Env, IntoVal, Map, TryFromVal, Val, contractclient};

use crate::ContractResult;
use crate::storage::read_membership_contract;
use crate::types::{MemberId, VotingSystemError};

/// The reads of the Stellar Membership contract this contract relies on.
/// Only the client generated from it is called, never the trait.
#[allow(dead_code)]
#[contractclient(name = "MembershipClient")]
pub trait Membership {
    /// The address holding `token_id`. Fails for a token never minted and
    /// for a revoked one, so a success means an active member.
    fn owner_of(env: Env, token_id: u32) -> Address;
}

/// Require that `member_id` names an active member.
///
/// Any failure of the membership contract reads as `NotAMember`: a token
/// never minted, a revoked one, or a membership contract that cannot
/// answer.
pub(crate) fn require_member(env: &Env, member_id: MemberId) -> ContractResult<()> {
    let membership = MembershipClient::new(env, &read_membership_contract(env));
    match membership.try_owner_of(&member_id) {
        Ok(Ok(_)) => Ok(()),
        _ => Err(VotingSystemError::NotAMember),
    }
}

/// Require that every key of an uploaded map names an active member.
pub(crate) fn require_members<V>(env: &Env, map: &Map<MemberId, V>) -> ContractResult<()>
where
    V: IntoVal<Env, Val> + TryFromVal<Env, Val>,
{
    for member_id in map.keys() {
        require_member(env, member_id)?;
    }
    Ok(())
}
