mod cli;
mod files;
mod journal;
mod proof;
mod seal;

use anyhow::{bail, Context, Result};
use clap::Parser;
use cli::{Cli, Command, Neuron};
use risc0_zkvm::{sha::Digest, Receipt};
use std::{path::Path, time::Instant};

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Prove { input, output } => prove(cli.neuron, &input, &output),
        Command::ImageId => {
            println!("{}", cli.neuron.image_id());
            Ok(())
        }
    }
}

fn current_round() -> Result<u32> {
    let entries =
        dotenvy::dotenv_iter().context("No .env file in the working directory or its parents")?;
    for entry in entries {
        let (key, value) = entry.context("Invalid .env file")?;
        if key == "CURRENT_ROUND" {
            return value
                .trim()
                .parse()
                .with_context(|| format!("CURRENT_ROUND must be a u32, got {value:?}"));
        }
    }
    bail!("CURRENT_ROUND is not set in the .env file")
}

fn prove(neuron: Neuron, input: &Path, output: &Path) -> Result<()> {
    let current_round = current_round()?;
    let start = Instant::now();
    let proven = neuron.prove(current_round, input)?;
    let proof = seal::on_chain_proof(&proven)?;
    let mut json = journal::decode_journal(&proven.journal, neuron)?;
    json["journal"] = hex::encode(&proven.journal.bytes).into();
    json["seal"] = hex::encode(&proof.seal).into();
    json["imageId"] = proof.image_id.to_string().into();
    files::save_output(output, &serde_json::to_string_pretty(&json)?)?;
    eprintln!(
        "Proof generated in {:.2}s: {}",
        start.elapsed().as_secs_f64(),
        output.display()
    );
    Ok(())
}

impl Neuron {
    fn prove(self, current_round: u32, input: &Path) -> Result<Receipt> {
        match self {
            Neuron::PriorVotingHistory => {
                prior_voting_history_core::validate_current_round(current_round)?;
                let input: prior_voting_history_core::Input = files::read_json(input)?;
                input.validate()?;
                proof::prove(
                    current_round,
                    &input,
                    methods::PRIOR_VOTING_HISTORY_GUEST_ELF,
                )
            }
            Neuron::AssignedReputation => {
                let input: assigned_reputation_core::Input = files::read_json(input)?;
                proof::prove(
                    current_round,
                    &input,
                    methods::ASSIGNED_REPUTATION_GUEST_ELF,
                )
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
