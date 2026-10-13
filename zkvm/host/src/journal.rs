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
            for (public_key, score) in output.scores {
                if scores.insert(public_key.clone(), score).is_some() {
                    bail!("public key {public_key} is used twice");
                }
            }
            Ok(json!({
                "currentRound": output.current_round,
                "scores": scores,
            }))
        }
        Neuron::TrustGraph => {
            let output: trust_graph_core::Output =
                journal.decode().context("Invalid public result")?;
            Ok(serde_json::to_value(&output)?)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use risc0_zkvm::{default_executor, ExecutorEnv};
    use serde::Serialize;

    fn execute(input: &impl Serialize, elf: &[u8], neuron: Neuron) -> Value {
        let env = ExecutorEnv::builder()
            .write(&33u32)
            .unwrap()
            .write(input)
            .unwrap()
            .build()
            .unwrap();
        let session = default_executor().execute(env, elf).unwrap();
        decode_journal(&session.journal, neuron).unwrap()
    }

    #[test]
    fn public_keys_survive_guest_execution_and_journal_decoding() {
        let assigned: assigned_reputation_core::Input =
            serde_json::from_str(include_str!("../../data/example_assigned_reputation.json"))
                .unwrap();
        let first = assigned.users[0].public_key.clone();
        let second = assigned.users[1].public_key.clone();
        let output = execute(
            &assigned,
            methods::ASSIGNED_REPUTATION_GUEST_ELF,
            Neuron::AssignedReputation,
        );
        assert_eq!(output["currentRound"], 33);
        assert_eq!(output["scores"][&first], 3.0);
        assert_eq!(output["scores"][&second], 3.5);

        let history: prior_voting_history_core::Input =
            serde_json::from_str(include_str!("../../data/example_prior_voting_history.json"))
                .unwrap();
        let expected = prior_voting_history_core::calculate(33, history.clone()).unwrap();
        let output = execute(
            &history,
            methods::PRIOR_VOTING_HISTORY_GUEST_ELF,
            Neuron::PriorVotingHistory,
        );
        assert_eq!(output["currentRound"], 33);
        assert_eq!(output["scores"].as_object().unwrap().len(), 4);
        for (key, score) in expected.scores {
            assert!((output["scores"][&key].as_f64().unwrap() - score).abs() < 1e-12);
        }

        let third = history.users[2].clone();
        let trust = trust_graph_core::Input {
            users: vec![first.clone(), second.clone(), third.clone()],
            trusted_for_user: [(first, vec![second.clone()]), (second, vec![third.clone()])]
                .into_iter()
                .collect(),
        };
        let expected = trust_graph_core::calculate(33, trust.clone()).unwrap();
        let output = execute(&trust, methods::TRUST_GRAPH_GUEST_ELF, Neuron::TrustGraph);
        assert_eq!(output["currentRound"], 33);
        for (key, score) in expected.scores {
            assert!((output["scores"][&key].as_f64().unwrap() - score).abs() < 0.001);
        }
        assert_eq!(output["scores"].as_object().unwrap().len(), 3);
    }

    #[test]
    fn duplicate_public_keys_in_assigned_journal_are_rejected() {
        let input: assigned_reputation_core::Input =
            serde_json::from_str(include_str!("../../data/example_assigned_reputation.json"))
                .unwrap();
        let key = input.users[0].public_key.clone();
        let output = assigned_reputation_core::Output {
            current_round: 33,
            scores: vec![(key.clone(), 1.0), (key, 2.0)],
        };
        let bytes = risc0_zkvm::serde::to_vec(&output)
            .unwrap()
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect();
        let journal = risc0_zkvm::Journal::new(bytes);
        let error = decode_journal(&journal, Neuron::AssignedReputation).unwrap_err();
        assert!(error.to_string().contains("public key"));
    }
}
