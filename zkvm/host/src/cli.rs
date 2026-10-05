use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser)]
#[command(about = "Prove neuron scores without publishing the private input")]
pub struct Cli {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum Neuron {
    PriorVotingHistory,
    AssignedReputation,
}

#[derive(Subcommand)]
pub enum Command {
    /// Generate a real proof locally; private input stays on this machine.
    Prove {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        receipt: PathBuf,
        /// Decoded journal JSON. Existing files are not overwritten.
        #[arg(long)]
        output: PathBuf,
    },
    /// Show the identity of the compiled guest.
    ImageId,
}
