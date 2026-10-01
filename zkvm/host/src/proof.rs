use anyhow::{ensure, Result};
use risc0_zkvm::{ExecutorEnv, ExternalProver, InnerReceipt, Prover, ProverOpts, Receipt};
use serde::Serialize;

pub fn prove(input: &impl Serialize, elf: &[u8]) -> Result<Receipt> {
    let env = ExecutorEnv::builder().write(input)?.build()?;
    let prover = ExternalProver::new("local", "r0vm");
    let opts = ProverOpts::groth16().with_dev_mode(false);

    let receipt = prover.prove_with_opts(env, elf, &opts)?.receipt;
    ensure!(
        matches!(receipt.inner, InnerReceipt::Groth16(_)),
        "Prover did not return a Groth16 receipt"
    );

    Ok(receipt)
}
