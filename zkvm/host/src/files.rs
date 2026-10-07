use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use std::{fs, io::Write, path::Path};

pub fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let file = fs::File::open(path).with_context(|| format!("Cannot open {}", path.display()))?;
    serde_json::from_reader(file).with_context(|| format!("Invalid JSON in {}", path.display()))
}

pub fn save_output(path: &Path, json: &str) -> Result<()> {
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
    file.write_all(json.as_bytes())
        .and_then(|_| file.write_all(b"\n"))
        .context("Cannot write output")
}
