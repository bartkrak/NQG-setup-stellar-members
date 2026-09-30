mod cli;
mod journal;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Command};
use std::time::Instant;
use verifier::{files, verify};

fn main() -> Result<()> {
    let cli = Cli::parse();
    let Command::Verify { receipt, image_id } = cli.command;

    let receipt = files::read_receipt(&receipt)?;
    let start = Instant::now();

    verify(&receipt, image_id)?;

    println!("{}", journal::decode_journal(&receipt.journal, cli.neuron)?);
    eprintln!("Proof verified in {:.3}s", start.elapsed().as_secs_f64());
    Ok(())
}
