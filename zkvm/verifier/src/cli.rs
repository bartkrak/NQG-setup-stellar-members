use clap::{Parser, Subcommand, ValueEnum};
use hex::FromHex;
use risc0_zkvm::sha::Digest;
use std::path::PathBuf;

#[derive(Parser)]
#[command(about = "Verify neuron receipts against an approved Image ID")]
pub struct Cli {
    /// Select the journal format
    #[arg(
        long,
        value_enum,
        global = true,
        default_value = "prior-voting-history"
    )]
    pub neuron: Neuron,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum Neuron {
    PriorVotingHistory,
    AssignedReputation,
}

#[derive(Subcommand)]
pub enum Command {
    /// Verify a receipt
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
