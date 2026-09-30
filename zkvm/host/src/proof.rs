use anyhow::Result;
use risc0_zkvm::{ExecutorEnv, ExternalProver, Prover, Receipt};
use serde::Serialize;

pub fn prove(input: &impl Serialize, elf: &[u8]) -> Result<Receipt> {
    let env = ExecutorEnv::builder().write(input)?.build()?;
    let prover = ExternalProver::new("local", "r0vm");

    let receipt = prover.prove(env, elf)?.receipt;

    Ok(receipt)
}
