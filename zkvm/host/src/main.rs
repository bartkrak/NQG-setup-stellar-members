mod cli;
mod proof;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Command};
use prior_voting_history_core::Input;
use std::time::Instant;
use verifier::files;

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Prove { input, receipt } => {
            let input: Input = files::read_json(&input)?;
            let start = Instant::now();
            let proven = proof::prove(&input)?;
            files::save_receipt(&receipt, &proven)?;
            eprintln!(
                "Proof generated in {:.2}s: {}",
                start.elapsed().as_secs_f64(),
                receipt.display()
            );
        }
        Command::ImageId => {
            println!(
                "{}",
                risc0_zkvm::sha::Digest::from(methods::PRIOR_VOTING_HISTORY_GUEST_ID)
            );
        }
    }
    Ok(())
}
