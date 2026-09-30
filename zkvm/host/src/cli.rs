use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(about = "Prove voting-history scores without publishing the history")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Generate a real proof locally; input history stays on this machine.
    Prove {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        receipt: PathBuf,
    },
    /// Show the identity of the guest compiled into this trusted application.
    ImageId,
}
