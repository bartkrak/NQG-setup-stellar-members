//! Groth16 values submitted to the on-chain verifier.
use anyhow::{bail, Context, Result};
use risc0_zkvm::{
    sha::{Digest, Digestible},
    Groth16Receipt, InnerReceipt, Receipt, ReceiptClaim,
};
pub struct OnChainProof {
    pub seal: Vec<u8>,
    pub image_id: Digest,
}

/// Read the seal, Image ID, and journal digest from a Groth16 receipt.
pub fn on_chain_proof(receipt: &Receipt) -> Result<OnChainProof> {
    let groth16 = match &receipt.inner {
        InnerReceipt::Groth16(inner) => inner,
        _ => bail!("Receipt is not a Groth16 proof"),
    };
    let claim = groth16
        .claim
        .as_value()
        .context("Groth16 claim is pruned")?;

    Ok(OnChainProof {
        seal: encode_seal(groth16),
        image_id: claim.pre.digest(),
    })
}

/// Selector prefix plus the Groth16 seal.
///
/// Same layout as `risc0_ethereum_contracts::encode_seal` for a Groth16 receipt:
/// the first four bytes of the verifier parameters, then the seal.
fn encode_seal(receipt: &Groth16Receipt<ReceiptClaim>) -> Vec<u8> {
    let selector = &receipt.verifier_parameters.as_bytes()[..4];
    let mut encoded = Vec::with_capacity(selector.len() + receipt.seal.len());
    encoded.extend_from_slice(selector);
    encoded.extend_from_slice(&receipt.seal);
    encoded
}
