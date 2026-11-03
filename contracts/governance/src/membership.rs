//! The link to the Stellar Membership contract, whose token ids are the
//! voter identifiers of this contract.

use soroban_sdk::{
    Address, Env, IntoVal, InvokeError, Map, Symbol, TryFromVal, Val, symbol_short, vec,
};

use crate::ContractResult;
use crate::storage::read_membership_contract;
use crate::types::VotingSystemError;

/// Require that `member_id` names an active member.
///
/// Reads `member(token_id).revoked`. The record is decoded as a map and only
/// `revoked` is read, so changes to its other fields do not break this
/// contract. Any failure reads as `NotAMember`: a token never minted, a
/// revoked one, a record without a `revoked` flag, or a membership contract
/// that cannot answer.
pub(crate) fn require_member(env: &Env, member_id: u32) -> ContractResult<()> {
    let record = env.try_invoke_contract::<Map<Symbol, Val>, InvokeError>(
        &read_membership_contract(env),
        &Symbol::new(env, "member"),
        vec![env, member_id.into_val(env)],
    );
    let revoked = match record {
        Ok(Ok(record)) => record
            .get(symbol_short!("revoked"))
            .and_then(|revoked| bool::try_from_val(env, &revoked).ok()),
        _ => None,
    };
    match revoked {
        Some(false) => Ok(()),
        _ => Err(VotingSystemError::NotAMember),
    }
}

/// The address holding `member_id`, which must name an active member.
///
/// `owner_of` answers for revoked tokens too, so `require_member` comes
/// first. A failure of either reads as `NotAMember`.
pub(crate) fn active_member_owner(env: &Env, member_id: u32) -> ContractResult<Address> {
    require_member(env, member_id)?;
    match env.try_invoke_contract::<Address, InvokeError>(
        &read_membership_contract(env),
        &Symbol::new(env, "owner_of"),
        vec![env, member_id.into_val(env)],
    ) {
        Ok(Ok(owner)) => Ok(owner),
        _ => Err(VotingSystemError::NotAMember),
    }
}

/// Require that every key of an uploaded map names an active member.
pub(crate) fn require_members<V>(env: &Env, map: &Map<u32, V>) -> ContractResult<()>
where
    V: IntoVal<Env, Val> + TryFromVal<Env, Val>,
{
    for member_id in map.keys() {
        require_member(env, member_id)?;
    }
    Ok(())
}
