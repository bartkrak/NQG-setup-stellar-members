//! A stand-in for the Stellar Membership contract: the reads the governance
//! contract relies on, plus `mint` and `revoke` to arrange the state a test
//! needs. It fails the way the real contract fails, with the same error
//! codes, so the governance contract sees on the test ledger what it will
//! see on the network.

use soroban_sdk::{
    Address, Env, contract, contracterror, contractimpl, contracttype, panic_with_error,
};

/// The error codes of the real contract raised by `owner_of`.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum MembershipError {
    NonExistentToken = 200,
    TokenRevoked = 206,
}

#[contracttype]
pub enum DataKey {
    NextTokenId,
    /// `token_id` -> current address, absent once revoked
    Owner(u32),
    /// address -> `token_id`
    TokenOf(Address),
    /// `token_id` -> (), written at mint and kept through a revocation
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
        env.storage()
            .instance()
            .set(&DataKey::Member(token_id), &());
        token_id
    }

    /// Release the address of `token_id` and keep its record, as the real
    /// contract does.
    pub fn revoke(env: &Env, token_id: u32) {
        let owner = Self::owner_of(env, token_id);
        env.storage().instance().remove(&DataKey::Owner(token_id));
        env.storage().instance().remove(&DataKey::TokenOf(owner));
    }

    /// The address holding `token_id`. Fails for a revoked token and for one
    /// never minted, told apart by the error code.
    pub fn owner_of(env: &Env, token_id: u32) -> Address {
        let owner: Option<Address> = env.storage().instance().get(&DataKey::Owner(token_id));
        match owner {
            Some(owner) => owner,
            None if env.storage().instance().has(&DataKey::Member(token_id)) => {
                panic_with_error!(env, MembershipError::TokenRevoked)
            }
            None => panic_with_error!(env, MembershipError::NonExistentToken),
        }
    }

    pub fn token_of(env: &Env, owner: Address) -> Option<u32> {
        env.storage().instance().get(&DataKey::TokenOf(owner))
    }
}
