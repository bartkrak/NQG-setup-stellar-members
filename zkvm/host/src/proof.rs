use anyhow::Result;
use methods::PRIOR_VOTING_HISTORY_GUEST_ELF;
use prior_voting_history_core::Input;
use risc0_zkvm::{ExecutorEnv, ExternalProver, Prover, Receipt};

pub fn prove(input: &Input) -> Result<Receipt> {
    let env = ExecutorEnv::builder().write(input)?.build()?;
    let prover = ExternalProver::new("local", "r0vm");

    let receipt = prover.prove(env, PRIOR_VOTING_HISTORY_GUEST_ELF)?.receipt;

    Ok(receipt)
}
