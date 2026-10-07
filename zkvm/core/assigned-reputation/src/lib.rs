// Shared types and scoring logic for the assigned-reputation neuron.
// Used by both the host and the zkVM guest, so the proven computation
// is exactly the same code the host (or anyone else) can run natively.
//
// Ported from neurons/src/assigned_reputation.rs. The host reads Input directly
// from JSON and passes it to the guest (tier as i32, roles per user).

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReputationTier {
    Unknown,
    Verified,
    Pathfinder,
    Navigator,
    Pilot,
}

impl ReputationTier {
    pub fn from_i32(tier: i32) -> Self {
        match tier {
            0 => Self::Verified,
            1 => Self::Pathfinder,
            2 => Self::Navigator,
            3 => Self::Pilot,
            _ => Self::Unknown,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserRecord {
    pub id: String,
    pub tier: i32,
    pub discord_roles: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Input {
    pub users: Vec<UserRecord>,
}


#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Output {
    /// Not used in scoring; copied into the journal so it is proven with the scores.
    pub current_round: u32,
    /// (id, bonus) pairs, in the same order as the input users.
    pub scores: Vec<(String, f64)>,
}

fn reputation_bonus(tier: ReputationTier) -> f64 {
    match tier {
        ReputationTier::Unknown | ReputationTier::Verified => 0.0,
        ReputationTier::Pathfinder => 1.0,
        ReputationTier::Navigator => 2.0,
        ReputationTier::Pilot => 3.0,
    }
}

fn role_to_bonus(role: &str) -> f64 {
    match role {
        "Ambassador President" => 1.0,
        "SCF Project" => 1.0,
        "Public Good Contributor" => 1.0,
        "Moderator" => 1.0,
        "SDF" => 1.0,
        "Tier 1 Validator" => 1.0,

        "West Africa Ambassador" => 0.5,
        "Brazil Ambassador" => 0.5,
        "India Ambassador" => 0.5,
        "Southern African Ambassador" => 0.5,
        "East Africa Ambassador" => 0.5,
        "Mexico Ambassador" => 0.5,
        "Colombia Ambassador" => 0.5,
        "Chile Ambassador" => 0.5,
        "Argentina Ambassador" => 0.5,
        "Europe Ambassador" => 0.5,

        _ => 0.0,
    }
}

fn discord_roles_bonus(roles: &[String]) -> f64 {
    roles.iter().map(|role| role_to_bonus(role)).sum()
}

pub fn calculate_result(current_round: u32, input: &Input) -> Output {
    let scores = input
        .users
        .iter()
        .map(|user| {
            let bonus = reputation_bonus(ReputationTier::from_i32(user.tier))
                + discord_roles_bonus(&user.discord_roles);
            (user.id.clone(), bonus)
        })
        .collect();
    Output {
        current_round,
        scores,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(id: &str, tier: i32, roles: &[&str]) -> UserRecord {
        UserRecord {
            id: id.to_string(),
            tier,
            discord_roles: roles.iter().map(|r| r.to_string()).collect(),
        }
    }

    #[test]
    fn matches_original_neuron_run() {
        // Mirrors the neuron_run test from neurons/src/assigned_reputation.rs
        let input = Input {
            users: vec![
                user("52345125252", 2, &["SDF", "SCF Project", "Moderator"]),
                user("52345125253", 3, &[]),
                user("52345125254", 0, &["Public Good Contributor"]),
            ],
        };
        let output = calculate_result(33, &input);
        assert_eq!(output.current_round, 33);
        assert_eq!(output.scores[0], ("52345125252".to_string(), 5.0));
        assert_eq!(output.scores[1], ("52345125253".to_string(), 3.0));
        assert_eq!(output.scores[2], ("52345125254".to_string(), 1.0));
    }

    #[test]
    fn unknown_tier_and_roles_give_zero() {
        let input = Input {
            users: vec![user("52345125252", -1, &["unrecognized role"])],
        };
        assert_eq!(calculate_result(1, &input).scores[0].1, 0.0);
    }

    #[test]
    fn preserves_zkvm_ambassador_rules_even_where_neurons_differs() {
        for role in [
            "West Africa Ambassador",
            "India Ambassador",
            "Southern African Ambassador",
            "East Africa Ambassador",
            "Europe Ambassador",
        ] {
            assert_eq!(
                calculate_result(
                    1,
                    &Input {
                        users: vec![user("52345125252", 0, &[role])],
                    }
                )
                .scores[0]
                    .1,
                0.5,
                "{role}"
            );
        }
        for role in [
            "Bolivia Ambassador",
            "Costa Rica Ambassador",
            "Peru Ambassador",
            "Ghana Ambassador",
            "Nigeria Ambassador",
        ] {
            assert_eq!(
                calculate_result(
                    1,
                    &Input {
                        users: vec![user("52345125252", 0, &[role])],
                    }
                )
                .scores[0]
                    .1,
                0.0,
                "{role}"
            );
        }
    }

    #[test]
    fn preserves_order_duplicate_roles_and_unknown_tiers() {
        let output = calculate_result(
            1,
            &Input {
                users: vec![
                    user("52345125253", 99, &["SDF", "SDF"]),
                    user("52345125252", 1, &["Brazil Ambassador"]),
                ],
            },
        );
        assert_eq!(
            output.scores,
            vec![("52345125253".into(), 2.0), ("52345125252".into(), 1.5)]
        );
    }
}
