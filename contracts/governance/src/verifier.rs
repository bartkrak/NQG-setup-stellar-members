//! The link to the RISC Zero Groth16 verifier contract
//! (NethermindEth/stellar-risc0-verifier), which checks the zkvm proof behind
//! every uploaded neuron result.

use soroban_sdk::{Bytes, BytesN, Env, IntoVal, InvokeError, Symbol, contracttype, vec};

use crate::ContractResult;
use crate::storage::read_verifier;
use crate::types::VotingSystemError;

/// The zkvm guest that computed a neuron result. Its Image ID is fixed in this
/// contract, so a changed guest needs an `upgrade`.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NeuronGuest {
    PriorVotingHistory,
    AssignedReputation,
    TrustGraph,
}

/// A proof the verifier accepted. The journal itself is only in the
/// `set_neuron_result` transaction; `journal_digest` is its SHA-256.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NeuronProof {
    pub seal: Bytes,
    pub image_id: BytesN<32>,
    pub journal_digest: BytesN<32>,
}

/// Image ID of the prior-voting-history guest (`host image-id`).
const PRIOR_VOTING_HISTORY_IMAGE_ID: [u8; 32] = [
    0x7d, 0x77, 0xb5, 0x0f, 0x56, 0x44, 0x31, 0x0c, 0x1c, 0x8e, 0xec, 0xb2, 0x9f, 0x28, 0xe2, 0xe2,
    0xe2, 0xf6, 0xdb, 0xf4, 0x5c, 0x04, 0xd0, 0x63, 0xcb, 0x7b, 0x9d, 0x4a, 0x23, 0x86, 0xee, 0xab,
];

/// Image ID of the assigned-reputation guest (`host image-id`).
const ASSIGNED_REPUTATION_IMAGE_ID: [u8; 32] = [
    0x30, 0xff, 0x1d, 0x77, 0x82, 0xdb, 0x5b, 0x5b, 0x6a, 0x6f, 0x87, 0x9e, 0x65, 0xbc, 0x83, 0x8a,
    0x58, 0x83, 0xf6, 0xa6, 0x56, 0xe3, 0xe2, 0x64, 0x7e, 0x5d, 0x8e, 0x55, 0xea, 0x9f, 0xca, 0x32,
];

/// Image ID of the trust-graph guest (`host image-id`).
const TRUST_GRAPH_IMAGE_ID: [u8; 32] = [
    0x33, 0x1b, 0x3e, 0x16, 0xa4, 0x25, 0xd8, 0x18, 0x0b, 0x86, 0x43, 0x11, 0xd6, 0xe4, 0x25, 0xc3,
    0xf8, 0x1a, 0xbf, 0x8d, 0x44, 0x9f, 0x32, 0x11, 0x08, 0x9a, 0x7d, 0x3c, 0x1c, 0x67, 0x6a, 0x2e,
];

impl NeuronGuest {
    /// The Image ID the verifier checks this guest's proofs against.
    pub fn image_id(self, env: &Env) -> BytesN<32> {
        let image_id = match self {
            NeuronGuest::PriorVotingHistory => &PRIOR_VOTING_HISTORY_IMAGE_ID,
            NeuronGuest::AssignedReputation => &ASSIGNED_REPUTATION_IMAGE_ID,
            NeuronGuest::TrustGraph => &TRUST_GRAPH_IMAGE_ID,
        };
        BytesN::from_array(env, image_id)
    }
}

/// Require that `seal` proves `guest` committed `journal`.
///
/// Calls `verify(seal, image_id, sha256(journal))` on the verifier contract;
/// its third argument is the journal digest despite the name `journal` there.
/// The digest is computed here, so a caller cannot pass one that does not
/// belong to the journal. Any failure reads as `InvalidProof`: a rejected
/// proof, or a verifier that cannot answer.
pub(crate) fn verify_proof(
    env: &Env,
    guest: NeuronGuest,
    journal: &Bytes,
    seal: Bytes,
) -> ContractResult<NeuronProof> {
    let image_id = guest.image_id(env);
    let journal_digest: BytesN<32> = env.crypto().sha256(journal).into();
    let verified = env.try_invoke_contract::<(), InvokeError>(
        &read_verifier(env),
        &Symbol::new(env, "verify"),
        vec![
            env,
            seal.into_val(env),
            image_id.into_val(env),
            journal_digest.into_val(env),
        ],
    );
    match verified {
        Ok(Ok(())) => Ok(NeuronProof {
            seal,
            image_id,
            journal_digest,
        }),
        _ => Err(VotingSystemError::InvalidProof),
    }
}
