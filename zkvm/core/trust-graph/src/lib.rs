// Shared types and scoring logic for the trust-graph neuron.
// Used by both the host and the zkVM guest, so the proven computation
// is exactly the same code the host (or anyone else) can run natively.
//
// Ported from neurons/src/trust_graph.rs, including the highly trusted bonus
// saturation. The input is one round's trust lists and the output is the round
// plus the scores.

use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};

// users with top X % highest trust score (equal or above) will be considered highly trusted
const HIGHLY_TRUSTED_PERCENT_THRESHOLD: usize = 10;
// users trusted by highly trusted users will get a bonus of X % of their own score
const HIGHLY_TRUSTED_PERCENT_BONUS: f64 = 15.0;
// users that have their own trust list filled get a bonus of X % of their own score
const FILLED_TRUST_LIST_PERCENT_BONUS: f64 = 10.0;
// the total gain from the highly trusted bonus approaches this value and never exceeds it
const HIGHLY_TRUSTED_BONUS_SATURATION: f64 = 5.0;

/// Trust lists of the round being scored. User IDs are decimal strings.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Input {
    /// Users that get a score.
    pub users: Vec<String>,
    /// User -> users they trust. IDs outside `users` are allowed: they are part
    /// of the graph but get no score.
    pub trusted_for_user: HashMap<String, Vec<String>>,
}

impl Input {
    pub fn validate(&self) -> Result<()> {
        for user in &self.users {
            validate_user_id(user)?;
        }
        ensure!(
            self.users.iter().collect::<HashSet<_>>().len() == self.users.len(),
            "Users must not contain duplicates"
        );
        for (user, trusted) in &self.trusted_for_user {
            validate_user_id(user)?;
            for other in trusted {
                validate_user_id(other)?;
                ensure!(other != user, "User {user} must not trust themselves");
            }
        }
        Ok(())
    }
}

fn validate_user_id(user: &str) -> Result<()> {
    ensure!(
        !user.is_empty() && user.bytes().all(|byte| byte.is_ascii_digit()),
        "User ID must be a nonempty string of ASCII digits: {user}"
    );
    Ok(())
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Output {
    /// Not used in scoring; copied into the journal so it is proven with the scores.
    pub current_round: u32,
    /// Sorted output gives stable JSON key ordering.
    pub scores: BTreeMap<String, f64>,
}

/// Validate even when the input was deserialized directly through serde.
pub fn calculate(current_round: u32, input: Input) -> Result<Output> {
    input.validate()?;
    let neuron = TrustGraphNeuron::from_data(input.trusted_for_user);
    Ok(Output {
        current_round,
        scores: neuron.calculate_result(&input.users).into_iter().collect(),
    })
}

#[derive(Clone, Debug)]
pub struct TrustGraphNeuron {
    trusted_for_user: HashMap<String, Vec<String>>,
}

impl TrustGraphNeuron {
    pub fn from_data(trusted_for_user: HashMap<String, Vec<String>>) -> Self {
        Self { trusted_for_user }
    }

    pub fn calculate_result(&self, users: &[String]) -> HashMap<String, f64> {
        let page_rank_result = self.handle_page_rank(users);
        let highly_trusted_bonus_result = self.handle_highly_trusted_bonus(
            page_rank_result.clone(),
            HIGHLY_TRUSTED_PERCENT_THRESHOLD,
            HIGHLY_TRUSTED_PERCENT_BONUS,
        );
        let saturated_bonus_result = Self::handle_highly_trusted_bonus_saturation(
            &page_rank_result,
            highly_trusted_bonus_result,
        );
        self.handle_filled_trust_list_bonus(saturated_bonus_result)
    }

    fn handle_page_rank(&self, users: &[String]) -> HashMap<String, f64> {
        let mut result: HashMap<String, f64> = HashMap::new();
        let mut nodes = HashSet::new();
        let mut edges = Vec::new();

        for (user, edge) in &self.trusted_for_user {
            nodes.insert(user.clone());
            for other_user in edge {
                nodes.insert(other_user.clone());
            }
            edges.push((user.clone(), edge.clone()));
        }
        let nodes: Vec<String> = nodes.into_iter().collect();

        let page_rank_result = calculate_page_rank(&nodes, &edges, 1000, 0.85);
        let page_rank_result = min_max_normalize_result(page_rank_result);

        for user in users {
            let page_rank = *page_rank_result.get(user).unwrap_or(&0.0);
            result.insert(user.into(), page_rank);
        }
        result
    }

    fn handle_highly_trusted_bonus(
        &self,
        trust_map: HashMap<String, f64>,
        percent_threshold: usize,
        percent_bonus: f64,
    ) -> HashMap<String, f64> {
        // ADDITIONAL BONUS if you're trusted by highly trusted user
        let mut result_with_bonus: HashMap<String, f64> = trust_map.clone();

        // calculate who has top X % highest trust
        let high_trust_value = calculate_high_trust_value(&trust_map, percent_threshold);

        // if you're trusted by someone whos trust score is higher than this, you get additional X % of your own score
        for (user, score) in &trust_map {
            // is user considered highly trusted
            if score >= &high_trust_value {
                // get all users he trusts
                // (someone can be trusted by a lot of users, but not trust anyone himself, in such case just skip)
                if let Some(trusted_for_this_user) = self.trusted_for_user.get(user) {
                    // give everyone a bonus; users outside `users` get no score, so skip them
                    for u in trusted_for_this_user {
                        if let Some(res) = result_with_bonus.get_mut(u) {
                            *res += (*res / 100.0) * percent_bonus;
                        }
                    }
                }
            }
        }

        result_with_bonus
    }

    fn handle_filled_trust_list_bonus(
        &self,
        trust_map: HashMap<String, f64>,
    ) -> HashMap<String, f64> {
        trust_map
            .into_iter()
            .map(|(user, score)| {
                let score = if self.has_filled_trust_list(&user) {
                    score + (score / 100.0) * FILLED_TRUST_LIST_PERCENT_BONUS
                } else {
                    score
                };
                (user, score)
            })
            .collect()
    }

    fn has_filled_trust_list(&self, user: &str) -> bool {
        self.trusted_for_user
            .get(user)
            .is_some_and(|trusted_users| !trusted_users.is_empty())
    }

    /// Squashes only what the highly trusted bonus added; the page rank base is kept as is.
    fn handle_highly_trusted_bonus_saturation(
        base: &HashMap<String, f64>,
        with_bonus: HashMap<String, f64>,
    ) -> HashMap<String, f64> {
        with_bonus
            .into_iter()
            .map(|(user, score)| {
                let base_score = *base.get(&user).unwrap_or(&0.0);
                (
                    user,
                    base_score + saturate_highly_trusted_bonus(score - base_score),
                )
            })
            .collect()
    }
}

/// `SATURATION * tanh(gain / SATURATION)` written as the generalised logistic curve: close to
/// `gain` for small gains, never above `HIGHLY_TRUSTED_BONUS_SATURATION`.
fn saturate_highly_trusted_bonus(gain: f64) -> f64 {
    generalised_logistic_function(
        -HIGHLY_TRUSTED_BONUS_SATURATION,
        HIGHLY_TRUSTED_BONUS_SATURATION,
        1.0,
        1.0,
        2.0 / HIGHLY_TRUSTED_BONUS_SATURATION,
        1.0,
        0.0,
        gain,
    )
}

// Same helper and arithmetic as the reference types.rs.
#[allow(clippy::too_many_arguments)]
fn generalised_logistic_function(
    a: f64,
    k: f64,
    c: f64,
    q: f64,
    b: f64,
    nu: f64,
    x_off: f64,
    x: f64,
) -> f64 {
    a + (k - a) / (f64::powf(c + q * f64::exp(-b * (x - x_off)), 1.0 / nu))
}

#[allow(clippy::cast_precision_loss)]
fn calculate_page_rank(
    nodes: &Vec<String>,
    edges: &Vec<(String, Vec<String>)>,
    iterations: u32,
    damping_factor: f64,
) -> HashMap<String, f64> {
    let mut page_ranks: HashMap<String, f64> = HashMap::new();
    for node in nodes {
        page_ranks.insert(node.clone(), 1.0 / nodes.len() as f64);
    }
    for _ in 0..iterations {
        let mut new_ranks: HashMap<String, f64> = HashMap::new();
        for node in nodes {
            let mut rank = (1.0 - damping_factor) / nodes.len() as f64;
            for (other_node, other_node_edges) in edges {
                if other_node_edges.contains(node) {
                    let pr = page_ranks.get(other_node).unwrap_or(&0.0);
                    rank += (damping_factor * pr) / other_node_edges.len() as f64;
                }
            }
            new_ranks.insert(node.clone(), rank);
        }
        page_ranks = new_ranks;
    }

    page_ranks
}

// Scale factor for min-max normalization output (0 to SCALE)
const NORMALIZATION_SCALE: f64 = 3.0;

fn min_max_normalize_result(result: HashMap<String, f64>) -> HashMap<String, f64> {
    let min = result.values().copied().reduce(f64::min).unwrap();
    let max = result.values().copied().reduce(f64::max).unwrap();

    result
        .into_iter()
        .map(|(key, value)| {
            let new_value = ((value - min) / (max - min)) * NORMALIZATION_SCALE;
            (key, new_value)
        })
        .collect()
}

fn calculate_high_trust_value(trust_map: &HashMap<String, f64>, percent_threshold: usize) -> f64 {
    let mut trust_scores_sorted: Vec<f64> = trust_map.values().cloned().collect();
    trust_scores_sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let target_index =
        trust_scores_sorted.len() - ((trust_scores_sorted.len() * percent_threshold) / 100).max(1);

    trust_scores_sorted.get(target_index).unwrap().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! assert_f64_near {
        ( $a:expr, $b:expr ) => {
            let eps = 0.001f64;
            assert!(
                ($a - $b).abs() < eps,
                "Values a = {}, b = {} are not near",
                $a,
                $b
            );
        };
    }

    fn users_vec(names: &[&str]) -> Vec<String> {
        names.iter().map(|s| (*s).to_string()).collect()
    }

    fn trust(lists: &[(&str, &[&str])]) -> HashMap<String, Vec<String>> {
        lists
            .iter()
            .map(|(user, trusted)| (user.to_string(), users_vec(trusted)))
            .collect()
    }

    fn sample_trust() -> HashMap<String, Vec<String>> {
        trust(&[
            ("A", &["B", "C"]),
            ("B", &["A"]),
            ("C", &["A", "B"]),
            ("D", &["A"]),
            ("E", &[]),
        ])
    }

    #[test]
    fn calc_high_trust_value() {
        let mut trust_map: HashMap<String, f64> = HashMap::new();
        for x in 1..=100 {
            trust_map.insert(format!("u{x}"), x as f64);
        }
        assert_eq!(calculate_high_trust_value(&trust_map, 1), 100.0);
        assert_eq!(calculate_high_trust_value(&trust_map, 2), 99.0);
        assert_eq!(calculate_high_trust_value(&trust_map, 5), 96.0);
        assert_eq!(calculate_high_trust_value(&trust_map, 10), 91.0);
        assert_eq!(calculate_high_trust_value(&trust_map, 20), 81.0);
        assert_eq!(calculate_high_trust_value(&trust_map, 50), 51.0);
    }

    #[test]
    fn calc_highly_trusted_bonus() {
        let trust_graph_neuron = TrustGraphNeuron::from_data(sample_trust());

        let result = trust_graph_neuron.handle_page_rank(&users_vec(&["A", "B", "C", "D", "E"]));

        let with_bonus = trust_graph_neuron.handle_highly_trusted_bonus(result, 1, 100.0);

        // PageRank normalized to 0-3, then 100% bonus applied
        assert_f64_near!(with_bonus.get("B").unwrap(), &4.226);
        assert_f64_near!(with_bonus.get("C").unwrap(), &2.794);
    }

    #[test]
    fn simple_page_rank() {
        let trust_graph_neuron = TrustGraphNeuron::from_data(sample_trust());

        let result = trust_graph_neuron.handle_page_rank(&users_vec(&["A", "B", "C", "D", "E"]));

        // PageRank normalized to 0-3 range
        assert_f64_near!(result.get("A").unwrap(), &3.0);
        assert_f64_near!(result.get("B").unwrap(), &2.112);
        assert_f64_near!(result.get("C").unwrap(), &1.397);
        assert_f64_near!(result.get("D").unwrap(), &0.0);
        assert_f64_near!(result.get("E").unwrap(), &0.0);
    }

    #[test]
    fn more_trusters_means_higher_pagerank() {
        // Single population so the node set (and the normalization base) is fixed: three targets
        // trusted by 1, 5 and 10 distinct users respectively. PageRank is relative, so this is the
        // honest framing of "trusted by more users -> higher score".
        let mut trusted_for_user: HashMap<String, Vec<String>> = HashMap::new();
        for (target, count) in [("target1", 1), ("target5", 5), ("target10", 10)] {
            for i in 0..count {
                trusted_for_user.insert(format!("{target}_truster{i}"), vec![target.to_string()]);
            }
        }
        let neuron = TrustGraphNeuron::from_data(trusted_for_user);
        let ranks = neuron.handle_page_rank(&users_vec(&["target1", "target5", "target10"]));
        let r1 = ranks.get("target1").unwrap();
        let r5 = ranks.get("target5").unwrap();
        let r10 = ranks.get("target10").unwrap();
        assert!(r10 > r5, "10 trusters ({r10}) should beat 5 ({r5})");
        assert!(r5 > r1, "5 trusters ({r5}) should beat 1 ({r1})");
    }

    #[test]
    fn min_max_normalization_bounds() {
        // A clear hub: u1,u2,u3 all trust `hub`. After normalization the unique max is 3.0
        // and the (equal) trusters are the min at 0.0.
        let neuron = TrustGraphNeuron::from_data(trust(&[
            ("u1", &["hub"]),
            ("u2", &["hub"]),
            ("u3", &["hub"]),
        ]));
        let ranks = neuron.handle_page_rank(&users_vec(&["hub", "u1", "u2", "u3"]));
        assert_f64_near!(ranks.get("hub").unwrap(), &3.0);
        assert_f64_near!(ranks.get("u1").unwrap(), &0.0);
    }

    #[test]
    fn highly_trusted_bonus_threshold_boundary() {
        // 10 users, distinct scores 1..=10. At a 10% threshold only the single top-scoring user
        // counts as highly trusted: index = len - max(1, len*10/100) = 10 - 1 = 9 (the top score).
        let mut trust_map: HashMap<String, f64> = HashMap::new();
        for i in 1..=10 {
            trust_map.insert(format!("u{i}"), f64::from(i));
        }
        // top user u10 trusts u1,u2; u9 (NOT highly trusted at 10%) trusts u3.
        let neuron = TrustGraphNeuron::from_data(trust(&[("u10", &["u1", "u2"]), ("u9", &["u3"])]));

        let with_bonus = neuron.handle_highly_trusted_bonus(trust_map, 10, 15.0);

        // u1,u2 trusted by highly-trusted u10 -> +15% of their own score
        assert_f64_near!(with_bonus.get("u1").unwrap(), &(1.0 * 1.15));
        assert_f64_near!(with_bonus.get("u2").unwrap(), &(2.0 * 1.15));
        // u3 trusted only by u9 (below threshold) -> unchanged
        assert_f64_near!(with_bonus.get("u3").unwrap(), &3.0);
        // u10 itself untouched
        assert_f64_near!(with_bonus.get("u10").unwrap(), &10.0);
    }

    #[test]
    fn filled_trust_list_gets_extra_bonus_for_any_user_with_filled_list() {
        let mut trust_map: HashMap<String, f64> = HashMap::new();
        trust_map.insert("A".to_string(), 2.0);
        trust_map.insert("B".to_string(), 2.0);
        trust_map.insert("C".to_string(), 2.0);

        let neuron = TrustGraphNeuron::from_data(trust(&[("A", &["B"]), ("B", &[])]));

        let with_bonus = neuron.handle_filled_trust_list_bonus(trust_map);

        assert_f64_near!(with_bonus.get("A").unwrap(), &(2.0 * 1.10));
        assert_f64_near!(with_bonus.get("B").unwrap(), &2.0);
        assert_f64_near!(with_bonus.get("C").unwrap(), &2.0);
    }

    #[test]
    fn calculate_result_full_pipeline() {
        let neuron = TrustGraphNeuron::from_data(sample_trust());
        let users = users_vec(&["A", "B", "C", "D", "E"]);

        let via_public = neuron.calculate_result(&users);
        let base = neuron.handle_page_rank(&users);
        let with_bonus = neuron.handle_highly_trusted_bonus(
            base.clone(),
            HIGHLY_TRUSTED_PERCENT_THRESHOLD,
            HIGHLY_TRUSTED_PERCENT_BONUS,
        );
        let manual = neuron.handle_filled_trust_list_bonus(
            TrustGraphNeuron::handle_highly_trusted_bonus_saturation(&base, with_bonus),
        );

        assert_eq!(via_public.len(), manual.len());
        for (k, v) in &manual {
            assert_f64_near!(via_public.get(k).unwrap(), v);
        }
    }

    #[test]
    fn calculate_result_pins_the_sample_scores() {
        let neuron = TrustGraphNeuron::from_data(sample_trust());
        let result = neuron.calculate_result(&users_vec(&["A", "B", "C", "D", "E"]));
        let base = neuron.handle_page_rank(&users_vec(&["A", "B", "C"]));
        // page rank A 3.0, B 2.112, C 1.397; only A is highly trusted and trusts B and C:
        // gain 15% squashed by 5 * tanh(gain / 5), then +10% for a filled trust list
        let expected = |base: f64| (base + 5.0 * (base * 0.15 / 5.0).tanh()) * 1.10;
        assert_f64_near!(result["A"], 3.0 * 1.10);
        assert_f64_near!(result["B"], expected(base["B"]));
        assert_f64_near!(result["C"], expected(base["C"]));
        assert_f64_near!(result["B"], 2.672);
        assert_f64_near!(result["C"], 1.767);
        assert_f64_near!(result["D"], 0.0);
        assert_f64_near!(result["E"], 0.0);
    }

    #[test]
    fn saturation_constants_match_the_agreed_parameters() {
        assert_eq!(NORMALIZATION_SCALE, 3.0);
        assert_eq!(HIGHLY_TRUSTED_BONUS_SATURATION, 5.0);
    }

    #[test]
    fn saturation_is_zero_at_zero_and_identity_like_for_small_gains() {
        assert_eq!(saturate_highly_trusted_bonus(0.0), 0.0);
        // slope 1 at the origin: small gains pass through almost unchanged
        assert_f64_near!(saturate_highly_trusted_bonus(0.1), &0.1);
        // a single +15% bonus on a max base score loses well under 1%
        let single_bonus_gain = NORMALIZATION_SCALE * HIGHLY_TRUSTED_PERCENT_BONUS / 100.0;
        assert!(
            (saturate_highly_trusted_bonus(single_bonus_gain) - single_bonus_gain).abs()
                < single_bonus_gain * 0.01
        );
    }

    #[test]
    fn saturation_matches_tanh_form() {
        for x in [0.5, 1.0, 3.0, 5.0, 8.0, 20.0, 60.0] {
            let expected =
                HIGHLY_TRUSTED_BONUS_SATURATION * (x / HIGHLY_TRUSTED_BONUS_SATURATION).tanh();
            assert_f64_near!(saturate_highly_trusted_bonus(x), &expected);
        }
    }

    #[test]
    fn saturation_caps_runaway_gains() {
        // the compounding highly-trusted bonus can otherwise produce huge scores for a single
        // user; any gain far past the cap must land on the cap instead of dwarfing every other neuron
        const K: f64 = HIGHLY_TRUSTED_BONUS_SATURATION;
        assert_f64_near!(saturate_highly_trusted_bonus(K * 10.0), &K);
        assert_f64_near!(saturate_highly_trusted_bonus(K * 100.0), &K);
        assert!(saturate_highly_trusted_bonus(K * 3.0) < K);
        assert!(saturate_highly_trusted_bonus(K * 3.0) > K * 0.99);
    }

    #[test]
    fn saturation_is_monotonic() {
        // up to 5 x the cap; far beyond it f64 rounds the curve to the cap itself
        let mut prev = saturate_highly_trusted_bonus(0.0);
        let mut x = 0.25;
        while x <= 5.0 * HIGHLY_TRUSTED_BONUS_SATURATION {
            let cur = saturate_highly_trusted_bonus(x);
            assert!(cur > prev, "expected increasing at x={x}: {cur} !> {prev}");
            prev = cur;
            x += 0.25;
        }
    }

    #[test]
    fn saturation_applies_only_to_the_bonus_gain_not_the_base() {
        let base: HashMap<String, f64> = HashMap::from([
            ("A".to_string(), 2.0),
            ("B".to_string(), 3.0),
            ("C".to_string(), 1.0),
        ]);
        // A gained a runaway +100 from stacked bonuses, B gained a single +15%, C got nothing
        let with_bonus: HashMap<String, f64> = HashMap::from([
            ("A".to_string(), 102.0),
            ("B".to_string(), 3.45),
            ("C".to_string(), 1.0),
        ]);

        let result = TrustGraphNeuron::handle_highly_trusted_bonus_saturation(&base, with_bonus);

        // base is preserved untouched, only the gain is squashed
        assert_f64_near!(result["A"], 2.0 + HIGHLY_TRUSTED_BONUS_SATURATION);
        assert!(result["A"] <= 2.0 + HIGHLY_TRUSTED_BONUS_SATURATION);
        // 0.45 gain -> 5 * tanh(0.09) = 0.4488
        assert_f64_near!(result["B"], 3.0 + 5.0 * (0.45f64 / 5.0).tanh());
        assert_eq!(result["C"], 1.0);
    }

    #[test]
    fn calculate_result_bonus_gain_never_exceeds_saturation() {
        // 40 users all trust `hub`, and hub trusts one of them back
        let mut trusted_for_user: HashMap<String, Vec<String>> = HashMap::new();
        for i in 0..40 {
            trusted_for_user.insert(format!("t{i}"), vec!["hub".to_string()]);
        }
        trusted_for_user.insert("hub".to_string(), vec!["t0".to_string()]);
        let neuron = TrustGraphNeuron::from_data(trusted_for_user);
        let mut users: Vec<String> = (0..40).map(|i| format!("t{i}")).collect();
        users.push("hub".to_string());

        let base = neuron.handle_page_rank(&users);
        let result = neuron.calculate_result(&users);
        for (user, score) in &result {
            // base + saturated gain, then at most +10% filled-trust-list bonus on top
            let max_allowed = (base[user] + HIGHLY_TRUSTED_BONUS_SATURATION)
                * (1.0 + FILLED_TRUST_LIST_PERCENT_BONUS / 100.0);
            assert!(
                *score <= max_allowed,
                "{user} score {score} exceeds {max_allowed}"
            );
            assert!(*score >= 0.0, "{user} score {score} is negative");
        }
    }

    #[test]
    fn stacked_bonuses_stay_below_base_plus_saturation() {
        // 30 highly trusted users (equal top scores) all trust `target`: 1.15^30 compounding
        // would multiply the score by 66
        let mut trust_map: HashMap<String, f64> = HashMap::new();
        let mut lists: HashMap<String, Vec<String>> = HashMap::new();
        for i in 0..30 {
            trust_map.insert(format!("top{i}"), 3.0);
            lists.insert(format!("top{i}"), vec!["target".to_string()]);
        }
        trust_map.insert("target".to_string(), 2.0);
        let neuron = TrustGraphNeuron::from_data(lists);

        let with_bonus = neuron.handle_highly_trusted_bonus(trust_map.clone(), 10, 15.0);
        assert!(with_bonus["target"] > 100.0);
        let result =
            TrustGraphNeuron::handle_highly_trusted_bonus_saturation(&trust_map, with_bonus);
        assert_f64_near!(result["target"], 2.0 + HIGHLY_TRUSTED_BONUS_SATURATION);
        assert_eq!(result["top0"], 3.0);
    }

    fn input(users: &[&str], lists: &[(&str, &[&str])]) -> Input {
        Input {
            users: users_vec(users),
            trusted_for_user: trust(lists),
        }
    }

    #[test]
    fn calculate_scores_only_users_and_keeps_round() {
        // "9" is outside `users`: it shapes the graph but gets no score.
        let output = calculate(
            33,
            input(
                &["0", "1", "2"],
                &[("0", &["1", "9"]), ("1", &["0"]), ("9", &["1"])],
            ),
        )
        .unwrap();
        assert_eq!(output.current_round, 33);
        assert_eq!(
            output.scores.keys().collect::<Vec<_>>(),
            vec!["0", "1", "2"]
        );
        assert_f64_near!(output.scores["2"], 0.0);
    }

    #[test]
    fn rejects_self_trust() {
        let err = calculate(33, input(&["0", "1"], &[("1", &["0", "1"])])).unwrap_err();
        assert!(
            err.to_string().contains("must not trust themselves"),
            "{err}"
        );
    }

    #[test]
    fn rejects_duplicate_users_and_non_numeric_ids() {
        assert!(input(&["0", "0"], &[("0", &["1"])]).validate().is_err());
        assert!(input(&["0", "a"], &[("0", &["1"])]).validate().is_err());
        assert!(input(&["0"], &[("0", &["b"])]).validate().is_err());
        assert!(input(&["0"], &[("", &["0"])]).validate().is_err());
    }

    #[test]
    fn example_inputs_are_valid() {
        for (json, users) in [(include_str!("../../../data/example_trust_graph.json"), 40)] {
            let input: Input = serde_json::from_str(json).unwrap();
            let output = calculate(33, input).unwrap();
            assert_eq!(output.scores.len(), users);
            assert!(output.scores.values().all(|score| score.is_finite()));
        }
    }
}
