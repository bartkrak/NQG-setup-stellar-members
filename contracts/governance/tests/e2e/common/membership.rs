//! A stand-in for the Stellar Membership contract: the reads the governance
//! contract relies on, plus `mint` and `revoke` to arrange the state a test
//! needs. It fails the way the real contract fails, with the same error
//! codes, so the governance contract sees on the test ledger what it will
//! see on the network.

use soroban_sdk::{
    Address, Env, contract, contracterror, contractimpl, contracttype, panic_with_error,
};

/// The error code of the real contract for a token never minted.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum MembershipError {
    NonExistentToken = 200,
}

/// Same variants and values as the real `Status`.
#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Status {
    Active = 0,
    Revoked = 1,
}

/// The real `Member` has more fields (external accounts, bio, projects); the
/// governance contract reads only `status`. `role` is here so the record is
/// not a single-field map, as on the network.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Member {
    pub status: Status,
    pub role: u32,
}

#[contracttype]
pub enum DataKey {
    NextTokenId,
    /// `token_id` -> current address, kept through a revocation
    Owner(u32),
    /// address -> `token_id`
    TokenOf(Address),
    /// `token_id` -> member record
    Member(u32),
}

#[contract]
pub struct MockMembership;

#[contractimpl]
impl MockMembership {
    /// Mint the next token id to `to`. No authorization: this is test setup.
    pub fn mint(env: &Env, to: Address) -> u32 {
        let token_id: u32 = env
            .storage()
            .instance()
            .get(&DataKey::NextTokenId)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::NextTokenId, &(token_id + 1));
        env.storage().instance().set(&DataKey::Owner(token_id), &to);
        env.storage()
            .instance()
            .set(&DataKey::TokenOf(to), &token_id);
        env.storage().instance().set(
            &DataKey::Member(token_id),
            &Member {
                status: Status::Active,
                role: 0,
            },
        );
        token_id
    }

    /// Revoke `token_id`, or reinstate it with `revoked` false. Only the
    /// status changes, as in the real contract.
    pub fn revoke(env: &Env, token_id: u32, revoked: bool) {
        let mut member = Self::member(env, token_id);
        member.status = if revoked {
            Status::Revoked
        } else {
            Status::Active
        };
        env.storage()
            .instance()
            .set(&DataKey::Member(token_id), &member);
    }

    /// The member record. Fails only for a token never minted.
    pub fn member(env: &Env, token_id: u32) -> Member {
        env.storage()
            .instance()
            .get(&DataKey::Member(token_id))
            .unwrap_or_else(|| panic_with_error!(env, MembershipError::NonExistentToken))
    }

    /// The address holding `token_id`, revoked or not. Fails only for a token
    /// never minted.
    pub fn owner_of(env: &Env, token_id: u32) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::Owner(token_id))
            .unwrap_or_else(|| panic_with_error!(env, MembershipError::NonExistentToken))
    }

    pub fn token_of(env: &Env, owner: Address) -> Option<u32> {
        env.storage().instance().get(&DataKey::TokenOf(owner))
    }
}
