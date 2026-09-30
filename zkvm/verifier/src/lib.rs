//! Receipt verification without compiling or executing a guest.
pub mod files;

use anyhow::{ensure, Context, Result};
use risc0_zkvm::{sha::Digest, InnerReceipt, Receipt};

/// The caller must supply an independently approved program identity.
pub fn verify(receipt: &Receipt, image_id: Digest) -> Result<()> {
    ensure!(
        !matches!(receipt.inner, InnerReceipt::Fake(_)),
        "Development receipts are not proofs"
    );
    receipt
        .verify(image_id)
        .context("Invalid proof or unexpected program")?;
    Ok(())
}
