//! The link to the Stellar Membership contract, whose token ids are the
//! voter identifiers of this contract.

use soroban_sdk::{Env, IntoVal, InvokeError, Map, Symbol, TryFromVal, Val, symbol_short, vec};

use crate::ContractResult;
use crate::storage::read_membership_contract;
use crate::types::VotingSystemError;

/// `Status::Active` of the membership contract.
const STATUS_ACTIVE: u32 = 0;

/// Require that `member_id` names an active member.
///
/// Reads `member(token_id).status`. The record is decoded as a map and only
/// `status` is read, so changes to its other fields do not break this
/// contract. Any failure reads as `NotAMember`: a token never minted, a
/// revoked one, or a membership contract that cannot answer.
pub(crate) fn require_member(env: &Env, member_id: u32) -> ContractResult<()> {
    let record = env.try_invoke_contract::<Map<Symbol, Val>, InvokeError>(
        &read_membership_contract(env),
        &Symbol::new(env, "member"),
        vec![env, member_id.into_val(env)],
    );
    let status = match record {
        Ok(Ok(record)) => record
            .get(symbol_short!("status"))
            .and_then(|status| u32::try_from_val(env, &status).ok()),
        _ => None,
    };
    match status {
        Some(STATUS_ACTIVE) => Ok(()),
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
