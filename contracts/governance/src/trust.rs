//! Encrypted trust lists. Each member stores who they trust, encrypted with
//! a data key that is itself sealed twice: for the member's key, derived
//! from a wallet signature, and for the operator's key (`set_trust_key`).
//! The list's format is defined off-chain; the contract sees the two sealed
//! copies of its key, so it can check the operator's and, later, replace one
//! copy without touching the list.

use soroban_sdk::{Bytes, BytesN, contracttype};

pub mod traits;

/// Largest encrypted list accepted, in bytes. Version 1 takes 286.
pub const MAX_TRUST_LIST_SIZE: u32 = 512;

/// One recipient's copy of a trust list's data key.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeyWrap {
    /// The recipient's X25519 public key.
    pub key: BytesN<32>,
    /// The one-time X25519 public key this copy was sealed with.
    pub ephemeral: BytesN<32>,
    /// The data key, sealed with AES-256-GCM, tag included.
    pub sealed: BytesN<48>,
}

/// A member's encrypted trust list.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrustList {
    /// Version, nonce and ciphertext of the list, opaque to the contract.
    pub list: Bytes,
    /// The member's copy of the data key.
    pub member: KeyWrap,
    /// The operator's copy, sealed for the key `get_trust_key` returns.
    pub operator: KeyWrap,
}
