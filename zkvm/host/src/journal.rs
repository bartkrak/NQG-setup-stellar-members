use crate::cli::Neuron;
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub fn decode_journal(journal: &risc0_zkvm::Journal, neuron: Neuron) -> Result<Value> {
    match neuron {
        Neuron::PriorVotingHistory => {
            let output: prior_voting_history_core::Output =
                journal.decode().context("Invalid public result")?;
            Ok(serde_json::to_value(&output)?)
        }
        Neuron::AssignedReputation => {
            let output: assigned_reputation_core::Output =
                journal.decode().context("Invalid public result")?;
            let mut scores = BTreeMap::new();
            for (id, score) in output.scores {
                if scores.insert(id.clone(), score).is_some() {
                    bail!("membership token {id} is used twice");
                }
            }
            Ok(json!({
                "currentRound": output.current_round,
                "scores": scores,
            }))
        }
    }
}
