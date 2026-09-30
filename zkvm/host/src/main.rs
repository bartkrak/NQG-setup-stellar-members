mod cli;
mod proof;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Command, Neuron};
use std::time::Instant;
use verifier::files;

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Prove { input, receipt } => {
            let start = Instant::now();
            let proven = match cli.neuron {
                Neuron::PriorVotingHistory => {
                    let input: prior_voting_history_core::Input = files::read_json(&input)?;
                    input.validate()?;
                    proof::prove(&input, methods::PRIOR_VOTING_HISTORY_GUEST_ELF)?
                }
                Neuron::AssignedReputation => {
                    let input: assigned_reputation_core::Input = files::read_json(&input)?;
                    proof::prove(&input, methods::ASSIGNED_REPUTATION_GUEST_ELF)?
                }
            };
            files::save_receipt(&receipt, &proven)?;
            eprintln!(
                "Proof generated in {:.2}s: {}",
                start.elapsed().as_secs_f64(),
                receipt.display()
            );
        }
        Command::ImageId => {
            let id = match cli.neuron {
                Neuron::PriorVotingHistory => methods::PRIOR_VOTING_HISTORY_GUEST_ID,
                Neuron::AssignedReputation => methods::ASSIGNED_REPUTATION_GUEST_ID,
            };
            println!("{}", risc0_zkvm::sha::Digest::from(id));
        }
    }
    Ok(())
}
