use crate::generalised_logistic_function;
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum Vote {
    Yes,
    No,
    Delegate,
    Abstain,
}

/// The votes of one round: submission -> user -> vote.
pub type RoundVotes = HashMap<String, HashMap<String, Vote>>;

// The round weight curve is centred this many rounds before the current one. In the first
// rounds the centre is below round 1 (negative), which the curve handles like any other value.
const ROUND_IMPORTANCE_DECAY_OFFSET: u32 = 3;
// Round weight curve: generalised logistic from 0 to 1.
const ROUND_WEIGHT_B: f64 = 0.9;
const ROUND_WEIGHT_NU: f64 = 2.0;
// Final bonus curve: generalised logistic scaled to [0, FINAL_BONUS_K).
const FINAL_BONUS_K: f64 = 5.0;
const FINAL_BONUS_B: f64 = 1.9;
const FINAL_BONUS_X_OFF: f64 = 3.0;
const ACTIVE_VOTES_HISTORY_OLDEST_ROUND: u32 = 32;
const ACTIVE_VOTES_MIN_RATIO: f64 = 0.5;
const OLDEST_ROUND: u32 = 1;

pub fn validate_current_round(current_round: u32) -> Result<()> {
    ensure!(
        current_round >= OLDEST_ROUND,
        "CURRENT_ROUND must be at least {OLDEST_ROUND}"
    );
    Ok(())
}

pub(crate) fn validate_user_id(user: &str) -> Result<()> {
    ensure!(
        !user.is_empty() && user.bytes().all(|byte| byte.is_ascii_digit()),
        "User ID must be a nonempty string of ASCII digits: {user}"
    );
    Ok(())
}

/// File-independent input shared by the CLI and zkVM guest. User IDs are decimal strings.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PriorVotingHistoryInput {
    pub users: Vec<String>,
    pub users_round_history: HashMap<String, Vec<u32>>,
    /// Optional. Without it every participated round counts as fully active.
    #[serde(default)]
    pub votes_per_round: Option<HashMap<u32, RoundVotes>>,
    /// Optional. Without it nobody gets the submitter bonus.
    #[serde(default)]
    pub submitters_per_round: HashMap<u32, Vec<String>>,
}

impl PriorVotingHistoryInput {
    pub fn from_json(json: &str) -> Result<Self> {
        let input: Self =
            serde_json::from_str(json).context("Invalid prior voting history JSON")?;
        input.validate()?;
        Ok(input)
    }

    pub fn validate(&self) -> Result<()> {
        for user in &self.users {
            validate_user_id(user)?;
        }
        ensure!(
            self.users.iter().collect::<HashSet<_>>().len() == self.users.len(),
            "Users must not contain duplicates"
        );
        for (user, rounds) in &self.users_round_history {
            validate_user_id(user)?;
            ensure!(
                rounds.iter().collect::<HashSet<_>>().len() == rounds.len(),
                "Duplicate rounds in usersRoundHistory for {user}"
            );
        }
        for (round, users) in &self.submitters_per_round {
            for user in users {
                validate_user_id(user)?;
            }
            ensure!(
                users.iter().collect::<HashSet<_>>().len() == users.len(),
                "Duplicate submitters in round {round}"
            );
        }
        for (round, submissions) in self.votes_per_round.iter().flatten() {
            for votes in submissions.values() {
                for user in votes.keys() {
                    validate_user_id(user)?;
                }
            }
            ensure!(
                !submissions.is_empty(),
                "VotesPerRound round {round} must contain at least one submission"
            );
        }
        Ok(())
    }

    /// Validate even when the input was deserialized directly through serde.
    /// Sorted output gives stable JSON key ordering.
    pub fn calculate(self, current_round: u32) -> Result<BTreeMap<String, f64>> {
        validate_current_round(current_round)?;
        self.validate()?;
        let neuron = PriorVotingHistoryNeuron {
            current_round,
            users_round_history: self.users_round_history,
            votes_per_round: self.votes_per_round,
            submitters_per_round: self
                .submitters_per_round
                .into_iter()
                .map(|(round, users)| (round, users.into_iter().collect()))
                .collect(),
        };
        Ok(neuron.calculate_result(&self.users).into_iter().collect())
    }
}

#[derive(Clone, Debug)]
pub struct PriorVotingHistoryNeuron {
    users_round_history: HashMap<String, Vec<u32>>,
    votes_per_round: Option<HashMap<u32, RoundVotes>>, // round -> submission -> user -> vote
    submitters_per_round: HashMap<u32, HashSet<String>>, // round -> users who had a submission that round
    current_round: u32,
}

impl PriorVotingHistoryNeuron {
    /// Compatibility entry point for the existing WASM orchestration.
    /// Uses the same fixed scoring rules as the JSON input path.
    pub fn from_data(
        users_round_history: HashMap<String, Vec<u32>>,
        votes_per_round: HashMap<u32, HashMap<String, HashMap<String, Vote>>>,
        submitters_per_round: HashMap<u32, HashSet<String>>,
        current_round: u32,
    ) -> Self {
        Self {
            users_round_history,
            votes_per_round: Some(votes_per_round),
            submitters_per_round,
            current_round,
        }
    }

    /// Computes the prior-voting-history bonus for a single user.
    ///
    /// `current_round` may be smaller than the decay offset: the centre of the round weight
    /// curve is then negative and moves up as rounds pass.
    pub fn calculate_bonus(&self, user: String) -> f64 {
        // 1. loop over all rounds up to current (a submitter couldn't vote in their own round,
        //    so that round may be missing from their voting history)
        // 2. if user had a submission in this round count it as 100% active voting
        // 3. otherwise only rounds the user participated in contribute: full weight before
        //    the fixed data cutoff, active-votes ratio from that cutoff onwards, or full
        //    weight if no votes per round were provided at all
        let rounds_participated = self
            .users_round_history
            .get(&user)
            .cloned()
            .unwrap_or_else(Vec::new);
        let x_offset = f64::from(self.current_round) - f64::from(ROUND_IMPORTANCE_DECAY_OFFSET);
        let mut rounds_weights_sum = 0.0;
        for round in OLDEST_ROUND..=self.current_round {
            let round_weight: f64 = generalised_logistic_function(
                0.0,
                1.0,
                1.0,
                1.0,
                ROUND_WEIGHT_B,
                ROUND_WEIGHT_NU,
                x_offset,
                round as f64,
            );
            if self
                .submitters_per_round
                .get(&round)
                .is_some_and(|s| s.contains(&user))
            {
                rounds_weights_sum += round_weight;
            } else if rounds_participated.contains(&round) {
                if round < ACTIVE_VOTES_HISTORY_OLDEST_ROUND {
                    rounds_weights_sum += round_weight;
                } else if let Some(votes_per_round) = &self.votes_per_round {
                    if let Some(votes) = votes_per_round.get(&round) {
                        // multiply weight by ratio of active votes in given round
                        rounds_weights_sum += round_weight
                            * calculate_active_votes_ratio(&user, votes, ACTIVE_VOTES_MIN_RATIO);
                    }
                } else {
                    // no votes per round provided: count the round as 100% active voting
                    rounds_weights_sum += round_weight;
                }
            }
        }
        // the curve below is 0 at 0 only up to rounding; keep "no history" an exact 0
        if rounds_weights_sum == 0.0 {
            return 0.0;
        }
        // pass the value into logistic curve
        // a = -k * exp(-b * x_off) makes f(0) = 0; k scales the output to [0, k)
        let a = -FINAL_BONUS_K * f64::exp(-FINAL_BONUS_B * FINAL_BONUS_X_OFF);
        generalised_logistic_function(
            a,
            FINAL_BONUS_K,
            1.0,
            1.0,
            FINAL_BONUS_B,
            1.0,
            FINAL_BONUS_X_OFF,
            rounds_weights_sum,
        )
    }
}

impl PriorVotingHistoryNeuron {
    pub fn name(&self) -> String {
        "prior_voting_history_neuron".to_string()
    }

    pub fn calculate_result(&self, users: &[String]) -> HashMap<String, f64> {
        let mut result = HashMap::new();
        for user in users {
            let bonus = self.calculate_bonus(user.to_string());
            result.insert(user.into(), bonus);
        }
        result
    }
}

fn calculate_active_votes_ratio(
    user: &str,
    votes: &HashMap<String, HashMap<String, Vote>>,
    min_ratio: f64,
) -> f64 {
    // Retain the legacy constructor behavior without relying on NaN.
    // JSON input rejects rounds with no submissions during validation.
    if votes.is_empty() {
        return min_ratio;
    }
    let active_votes_count = votes
        .values()
        .filter(|submission| matches!(submission.get(user), Some(Vote::Yes | Vote::No)))
        .count();
    (active_votes_count as f64 / votes.len() as f64).max(min_ratio)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    // Computed independently of this crate from the curve parameters (round weights B 0.9,
    // NU 2, decay offset 3; final bonus K 5, B 1.9, X_OFF 3).
    const W1_BONUS: f64 = 0.08670104357426563;
    const FULL_10_BONUS: f64 = 4.87838770192852;

    fn scores(input: Value) -> BTreeMap<String, f64> {
        PriorVotingHistoryInput::from_json(&input.to_string())
            .unwrap()
            .calculate(33)
            .unwrap()
    }

    fn scores_at(current_round: u32, input: Value) -> BTreeMap<String, f64> {
        PriorVotingHistoryInput::from_json(&input.to_string())
            .unwrap()
            .calculate(current_round)
            .unwrap()
    }

    fn assert_near(actual: f64, expected: f64) {
        assert!((actual - expected).abs() < 1e-9, "{actual} != {expected}");
    }

    #[test]
    fn rounds_start_at_1_with_a_negative_curve_offset() {
        // round 1, decay offset 3: the round weight curve is centred at -2
        let result = scores_at(
            1,
            json!({ "users": ["1", "2"], "usersRoundHistory": { "1": [1] } }),
        );
        // weight 1 / sqrt(1 + e^(-0.9 * 3)) = 0.96800, then the final bonus curve
        assert_near(result["1"], W1_BONUS);
        assert_eq!(result["2"], 0.0);
        assert!(
            PriorVotingHistoryInput::from_json(r#"{"users":[],"usersRoundHistory":{}}"#)
                .unwrap()
                .calculate(0)
                .is_err()
        );
    }

    #[test]
    fn full_history_approaches_the_maximum_of_5() {
        let rounds: Vec<u32> = (1..=10).collect();
        let result = scores_at(
            10,
            json!({ "users": ["1"], "usersRoundHistory": { "1": rounds } }),
        );
        assert_near(result["1"], FULL_10_BONUS);
        assert!(result["1"] < 5.0);
    }

    #[test]
    fn votes_and_submitters_per_round_are_optional() {
        let result = scores(json!({
            "users": ["1", "2"],
            "usersRoundHistory": { "1": [32, 33] },
        }));
        assert!(result["1"] > 0.0);
        assert_eq!(result["2"], 0.0);
    }

    #[test]
    fn without_votes_per_round_participation_counts_as_fully_active() {
        let without_votes = scores(json!({
            "users": ["1"],
            "usersRoundHistory": { "1": [32, 33] },
        }));
        let all_active = scores(json!({
            "users": ["1"],
            "usersRoundHistory": { "1": [32, 33] },
            "votesPerRound": {
                "32": { "a": { "1": "Yes" } },
                "33": { "b": { "1": "No" } },
            },
        }));
        assert_eq!(without_votes["1"], all_active["1"]);
    }

    #[test]
    fn votes_per_round_scale_by_active_votes_ratio_of_at_least_half() {
        let result = scores(json!({
            "users": ["1", "2", "3", "4"],
            "usersRoundHistory": { "1": [33], "2": [33], "3": [33], "4": [33] },
            "votesPerRound": {
                "33": {
                    "a": { "1": "Yes", "2": "Yes", "3": "Yes", "4": "Yes" },
                    "b": { "1": "No", "2": "Abstain", "3": "No", "4": "Yes" },
                    "c": { "1": "Delegate", "3": "Yes", "4": "No" },
                    "d": { "2": "Delegate", "3": "Yes" },
                },
            },
        }));
        let without_votes = scores(json!({
            "users": ["3"],
            "usersRoundHistory": { "3": [33] },
        }));
        // 2 of 4 active, and 1 of 4 raised to the 50% minimum
        assert_eq!(result["1"], result["2"]);
        assert!(result["1"] < result["4"]);
        assert!(result["4"] < result["3"]);
        assert_eq!(result["3"], without_votes["3"]);
    }

    #[test]
    fn votes_per_round_without_the_round_give_it_no_weight() {
        let result = scores(json!({
            "users": ["1"],
            "usersRoundHistory": { "1": [33] },
            "votesPerRound": { "32": { "a": { "1": "Yes" } } },
        }));
        assert_eq!(result["1"], 0.0);
    }

    #[test]
    fn submitters_get_the_full_round_weight() {
        let with_votes = scores(json!({
            "users": ["1", "2"],
            "usersRoundHistory": { "2": [33] },
            "votesPerRound": { "33": { "a": { "2": "Yes" } } },
            "submittersPerRound": { "33": ["1"] },
        }));
        let without_votes = scores(json!({
            "users": ["1", "2"],
            "usersRoundHistory": { "2": [33] },
            "submittersPerRound": { "33": ["1"] },
        }));
        assert!(with_votes["1"] > 0.0);
        assert_eq!(with_votes["1"], with_votes["2"]);
        assert_eq!(without_votes["1"], with_votes["1"]);
        assert_eq!(without_votes["2"], with_votes["2"]);
    }
}
