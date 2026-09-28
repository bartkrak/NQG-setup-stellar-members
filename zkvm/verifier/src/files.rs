//! Shared receipt encoding and JSON file access; no guest build dependency.
use anyhow::{ensure, Context, Result};
use bincode::Options;
use risc0_zkvm::Receipt;
use serde::de::DeserializeOwned;
use std::{
    fs,
    io::{Read, Write},
    path::Path,
};

const MAX_RECEIPT_BYTES: u64 = 64 * 1024 * 1024;

pub fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let file = fs::File::open(path).with_context(|| format!("Cannot open {}", path.display()))?;
    serde_json::from_reader(file).with_context(|| format!("Invalid JSON in {}", path.display()))
}

pub fn read_receipt(path: &Path) -> Result<Receipt> {
    let file = fs::File::open(path).with_context(|| format!("Cannot open {}", path.display()))?;
    ensure!(
        file.metadata()?.len() <= MAX_RECEIPT_BYTES,
        "Receipt exceeds 64 MiB"
    );
    let mut bytes = Vec::new();
    file.take(MAX_RECEIPT_BYTES + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= MAX_RECEIPT_BYTES,
        "Receipt exceeds 64 MiB"
    );
    bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .with_limit(MAX_RECEIPT_BYTES)
        .reject_trailing_bytes()
        .deserialize(&bytes)
        .context("Invalid receipt encoding")
}

pub fn save_receipt(path: &Path, receipt: &Receipt) -> Result<()> {
    let bytes = bincode::serialize(receipt)?;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .with_context(|| {
            format!(
                "Cannot create {} (existing files are not overwritten)",
                path.display()
            )
        })?;
    file.write_all(&bytes).context("Cannot write receipt")
}
