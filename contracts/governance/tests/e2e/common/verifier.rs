//! A stand-in for the RISC Zero Groth16 verifier contract: `verify` with the
//! real signature, accepting any proof unless told to reject. It records what
//! it was asked, so tests can check the Image ID and journal digest the
//! governance contract sends. The real verifier's error codes are not
//! reproduced; the governance contract reads any failure as `InvalidProof`.

use soroban_sdk::{
    Bytes, BytesN, Env, contract, contracterror, contractimpl, contracttype, panic_with_error,
};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum VerifierError {
    Rejected = 1,
}

#[contracttype]
pub enum DataKey {
    Reject,
    /// `(seal, image_id, journal digest)` of the last accepted `verify`
    Seen,
}

#[contract]
pub struct MockVerifier;

#[contractimpl]
impl MockVerifier {
    /// Reject every proof from now on, or accept again with `reject` false.
    pub fn reject(env: &Env, reject: bool) {
        env.storage().instance().set(&DataKey::Reject, &reject);
    }

    /// Fail if told to reject, otherwise accept and record the arguments.
    pub fn verify(env: &Env, seal: Bytes, image_id: BytesN<32>, journal: BytesN<32>) {
        if env
            .storage()
            .instance()
            .get(&DataKey::Reject)
            .unwrap_or(false)
        {
            panic_with_error!(env, VerifierError::Rejected);
        }
        env.storage()
            .instance()
            .set(&DataKey::Seen, &(seal, image_id, journal));
    }

    pub fn seen(env: &Env) -> Option<(Bytes, BytesN<32>, BytesN<32>)> {
        env.storage().instance().get(&DataKey::Seen)
    }
}

/// A journal and a seal for tests that are not about the proof: the mock
/// accepts any.
pub fn proof(env: &Env) -> (Bytes, Bytes) {
    (
        Bytes::from_slice(env, b"journal"),
        Bytes::from_slice(env, &[0x73, 0xc4, 0x57, 0xba]),
    )
}
