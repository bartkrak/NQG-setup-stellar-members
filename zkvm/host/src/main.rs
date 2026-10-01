mod cli;
mod files;
mod journal;
mod proof;
mod seal;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Command, Neuron};
use risc0_zkvm::{sha::Digest, Receipt};
use std::{path::Path, time::Instant};

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Prove { input, receipt } => prove(cli.neuron, &input, &receipt),
        Command::ImageId => {
            println!("{}", cli.neuron.image_id());
            Ok(())
        }
    }
}

fn prove(neuron: Neuron, input: &Path, receipt: &Path) -> Result<()> {
    let start = Instant::now();
    let proven = neuron.prove(input)?;
    files::save_receipt(receipt, &proven)?;
    println!("{}", journal::decode_journal(&proven.journal, neuron)?);
    println!("{}", seal::on_chain_proof(&proven)?);
    eprintln!(
        "Proof generated in {:.2}s: {}",
        start.elapsed().as_secs_f64(),
        receipt.display()
    );
    Ok(())
}

impl Neuron {
    fn prove(self, input: &Path) -> Result<Receipt> {
        match self {
            Neuron::PriorVotingHistory => {
                let input: prior_voting_history_core::Input = files::read_json(input)?;
                input.validate()?;
                proof::prove(&input, methods::PRIOR_VOTING_HISTORY_GUEST_ELF)
            }
            Neuron::AssignedReputation => {
                let input: assigned_reputation_core::Input = files::read_json(input)?;
                proof::prove(&input, methods::ASSIGNED_REPUTATION_GUEST_ELF)
            }
        }
    }

    fn image_id(self) -> Digest {
        let id = match self {
            Neuron::PriorVotingHistory => methods::PRIOR_VOTING_HISTORY_GUEST_ID,
            Neuron::AssignedReputation => methods::ASSIGNED_REPUTATION_GUEST_ID,
        };
        Digest::from(id)
    }
}
