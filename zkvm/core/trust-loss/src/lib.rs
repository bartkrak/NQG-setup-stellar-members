use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

mod math;
mod trust_loss;

pub use trust_loss::TrustLossNeuron;

pub type TrustLists = HashMap<String, Vec<String>>;
pub type TrustHistory = HashMap<u32, TrustLists>;

/// The reference neuron's supplied data, with membership IDs for this flow.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Input {
    pub users: Vec<String>,
    pub trusted_for_user_per_round: TrustHistory,
    pub neurons_results_sum: HashMap<String, f64>,
}

impl Input {
    pub fn validate(&self) -> Result<()> {
        for user in &self.users {
            validate_user_id(user)?;
        }
        for trusted_for_user in self.trusted_for_user_per_round.values() {
            for (user, trusted) in trusted_for_user {
                validate_user_id(user)?;
                for other in trusted {
                    validate_user_id(other)?;
                }
            }
        }
        for (user, score) in &self.neurons_results_sum {
            validate_user_id(user)?;
            ensure!(
                score.is_finite(),
                "Neuron results sum for user {user} must be finite"
            );
        }
        Ok(())
    }
}

fn validate_user_id(user: &str) -> Result<()> {
    ensure!(
        user.parse::<u32>().is_ok_and(|id| id.to_string() == user),
        "User ID must be a canonical u32 decimal string: {user}"
    );
    Ok(())
}

pub fn validate_current_round(current_round: u32) -> Result<()> {
    ensure!(
        current_round > 0,
        "Trust loss requires a positive current round"
    );
    Ok(())
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Output {
    pub current_round: u32,
    pub scores: BTreeMap<String, f64>,
}

pub fn calculate(current_round: u32, input: Input) -> Result<Output> {
    validate_current_round(current_round)?;
    input.validate()?;
    let neuron = TrustLossNeuron::from_data(
        current_round,
        input.trusted_for_user_per_round,
        input.neurons_results_sum,
    );
    Ok(Output {
        current_round,
        scores: neuron.calculate_result(&input.users).into_iter().collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Input {
        serde_json::from_str(include_str!("../../../data/example_trust_loss.json")).unwrap()
    }

    #[test]
    fn example_input_keeps_round_and_expected_scores() {
        let output = calculate(2, fixture()).unwrap();
        assert_eq!(output.current_round, 2);
        assert_eq!(output.scores.len(), 5);
        assert_eq!(output.scores["4"], -10.0);
        assert!(output
            .scores
            .iter()
            .all(|(id, score)| *score == if id == "4" { -10.0 } else { 0.0 }));
    }

    #[test]
    fn wrapper_preserves_duplicate_revokers_in_the_sum_like_the_reference() {
        let mut input = fixture();
        input.users.insert(0, "1".to_string());
        let output = calculate(2, input).unwrap();
        // The repeated user contributes twice to the reference's Vec-based sum,
        // while the threshold still requires three distinct users.
        let expected = -20.0
            * math::generalised_logistic_function(0.0, 100.0, 1.0, 1.0, 0.2, 1.0, 50.0, 60.0)
            / 100.0;
        assert_eq!(output.scores["4"], expected);
    }

    #[test]
    fn unused_score_sums_can_be_absent_like_the_reference() {
        let mut input = fixture();
        input.trusted_for_user_per_round.clear();
        input.neurons_results_sum.clear();
        assert!(calculate(2, input)
            .unwrap()
            .scores
            .values()
            .all(|score| *score == 0.0));
    }

    #[test]
    #[should_panic]
    fn a_consumed_missing_score_sum_fails_like_the_reference() {
        let mut input = fixture();
        input.neurons_results_sum.remove("4");
        let _ = calculate(2, input);
    }

    #[test]
    fn supplied_negative_sums_follow_the_reference_arithmetic() {
        let mut input = fixture();
        input.neurons_results_sum.insert("4".to_string(), -20.0);
        assert_eq!(calculate(2, input).unwrap().scores["4"], 10.0);
    }

    #[test]
    fn wrapper_allows_duplicate_and_self_trust_like_the_reference() {
        let mut input = fixture();
        input
            .trusted_for_user_per_round
            .get_mut(&1)
            .unwrap()
            .get_mut("1")
            .unwrap()
            .extend(["1".to_string(), "4".to_string()]);
        assert!(input.validate().is_ok());
    }

    #[test]
    fn wrapper_checks_membership_ids_finite_values_and_positive_round() {
        for invalid in ["", "-1", "+1", "01", "1.0", "a", "１２", "4294967296"] {
            assert!(validate_user_id(invalid).is_err());
        }
        for valid in ["0", "1", "4294967295"] {
            assert!(validate_user_id(valid).is_ok());
        }
        for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let mut input = fixture();
            input.neurons_results_sum.insert("1".to_string(), invalid);
            assert!(input.validate().is_err());
        }
        assert!(calculate(0, fixture()).is_err());
    }

    #[test]
    fn json_keeps_the_existing_guest_interface() {
        let value = serde_json::to_value(fixture()).unwrap();
        assert!(value.get("trustedForUserPerRound").is_some());
        assert!(value.get("neuronsResultsSum").is_some());
        let mut invalid = value;
        invalid["currentRound"] = serde_json::json!(2);
        assert!(serde_json::from_value::<Input>(invalid).is_err());
    }
}
