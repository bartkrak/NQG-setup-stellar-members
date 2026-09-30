use crate::cli::Neuron;
use anyhow::{Context, Result};

pub fn decode_journal(journal: &risc0_zkvm::Journal, neuron: Neuron) -> Result<String> {
    match neuron {
        Neuron::PriorVotingHistory => {
            let output: prior_voting_history_core::Output =
                journal.decode().context("Invalid public result")?;
            Ok(serde_json::to_string_pretty(&output)?)
        }
        Neuron::AssignedReputation => {
            let output: assigned_reputation_core::Output =
                journal.decode().context("Invalid public result")?;
            Ok(serde_json::to_string_pretty(&output)?)
        }
    }
}
