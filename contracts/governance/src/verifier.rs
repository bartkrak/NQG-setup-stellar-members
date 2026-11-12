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
    TrustLoss,
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
    0x01, 0x78, 0x63, 0x3f, 0xea, 0x6a, 0x65, 0x1c, 0x49, 0xdf, 0xe7, 0xbc, 0x91, 0xcc, 0x75, 0x70,
    0xff, 0x91, 0x4b, 0xff, 0x13, 0x53, 0xc7, 0x2d, 0xb4, 0xc6, 0x49, 0xe8, 0x31, 0xfe, 0xd8, 0x55,
];

/// Image ID of the assigned-reputation guest (`host image-id`).
const ASSIGNED_REPUTATION_IMAGE_ID: [u8; 32] = [
    0x97, 0x46, 0x5c, 0x5f, 0x88, 0x7a, 0x17, 0xa2, 0x4e, 0x50, 0xf3, 0xf0, 0x36, 0x81, 0x2e, 0x8c,
    0x50, 0x18, 0x5a, 0x28, 0x21, 0x6b, 0xe3, 0x65, 0xbe, 0x03, 0x57, 0x7c, 0x66, 0x6b, 0x67, 0x1b,
];

/// Image ID of the trust-graph guest (`host image-id`).
const TRUST_GRAPH_IMAGE_ID: [u8; 32] = [
    0x88, 0xfa, 0xab, 0x94, 0x05, 0xe8, 0x18, 0x6b, 0x86, 0xe8, 0x00, 0x94, 0x9f, 0x79, 0xcc, 0xf3,
    0xd3, 0x4d, 0xeb, 0x69, 0x8f, 0x9a, 0x6a, 0x6b, 0x6a, 0xdd, 0x47, 0x6a, 0xef, 0x15, 0x13, 0xee,
];

/// Image ID of the trust-loss guest (`host image-id`).
const TRUST_LOSS_IMAGE_ID: [u8; 32] = [
    0xcc, 0xf7, 0xae, 0x25, 0xc2, 0x09, 0x56, 0x21, 0x08, 0x97, 0xdd, 0xc5, 0xd6, 0x50, 0x1c, 0xfb,
    0x86, 0xe9, 0x43, 0x27, 0x18, 0x7b, 0xbd, 0x59, 0x53, 0x70, 0x5f, 0x1f, 0x51, 0x04, 0x7e, 0x7f,
];

impl NeuronGuest {
    /// The Image ID the verifier checks this guest's proofs against.
    pub fn image_id(self, env: &Env) -> BytesN<32> {
        let image_id = match self {
            NeuronGuest::PriorVotingHistory => &PRIOR_VOTING_HISTORY_IMAGE_ID,
            NeuronGuest::AssignedReputation => &ASSIGNED_REPUTATION_IMAGE_ID,
            NeuronGuest::TrustGraph => &TRUST_GRAPH_IMAGE_ID,
            NeuronGuest::TrustLoss => &TRUST_LOSS_IMAGE_ID,
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
