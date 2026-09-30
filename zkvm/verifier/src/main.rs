use anyhow::Result;
use clap::{Parser, Subcommand};
use hex::FromHex;
use risc0_zkvm::sha::Digest;
use std::{path::PathBuf, time::Instant};
use verifier::{files, verify};

#[derive(Parser)]
#[command(about = "Verify voting-history receipts against an approved Image ID")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Verify a receipt without the private history or guest toolchain.
    Verify {
        #[arg(long)]
        receipt: PathBuf,
        /// Independently approved guest Image ID: 64 hexadecimal characters.
        #[arg(long, value_parser = parse_image_id)]
        image_id: Digest,
    },
}

fn parse_image_id(value: &str) -> Result<Digest, String> {
    Digest::from_hex(value)
        .map_err(|_| "Image ID must contain exactly 64 hexadecimal characters".to_owned())
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Verify { receipt, image_id } => {
            let receipt = files::read_receipt(&receipt)?;
            let start = Instant::now();
            let output = verify(&receipt, image_id)?;
            println!("{}", serde_json::to_string_pretty(&output)?);
            eprintln!("Proof verified in {:.3}s", start.elapsed().as_secs_f64());
        }
    }
    Ok(())
}
