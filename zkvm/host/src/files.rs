use anyhow::{Context, Result};
use risc0_zkvm::Receipt;
use serde::de::DeserializeOwned;
use std::{fs, io::Write, path::Path};

pub fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let file = fs::File::open(path).with_context(|| format!("Cannot open {}", path.display()))?;
    serde_json::from_reader(file).with_context(|| format!("Invalid JSON in {}", path.display()))
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
