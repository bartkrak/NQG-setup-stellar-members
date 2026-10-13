// Shared types and scoring logic for the assigned-reputation neuron.
// Used by both the host and the zkVM guest, so the proven computation
// is exactly the same code the host (or anyone else) can run natively.
//
// Ported from neurons/src/assigned_reputation.rs. The host reads Input directly
// from JSON and passes it to the guest (tier as i32, roles per user).

use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

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
    pub public_key: String,
    pub tier: i32,
    pub discord_roles: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Input {
    pub users: Vec<UserRecord>,
}

impl Input {
    pub fn validate(&self) -> Result<()> {
        for user in &self.users {
            ensure!(
                stellar_strkey::ed25519::PublicKey::from_string(&user.public_key).is_ok(),
                "Invalid Stellar public key: {}",
                user.public_key
            );
        }
        ensure!(
            self.users
                .iter()
                .map(|user| &user.public_key)
                .collect::<HashSet<_>>()
                .len()
                == self.users.len(),
            "Users must not contain duplicate public keys"
        );
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Output {
    /// Not used in scoring; copied into the journal so it is proven with the scores.
    pub current_round: u32,
    /// (public_key, bonus) pairs, in the same order as the input users.
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
            (user.public_key.clone(), bonus)
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

    fn user(public_key: &str, tier: i32, roles: &[&str]) -> UserRecord {
        UserRecord {
            public_key: public_key.to_string(),
            tier,
            discord_roles: roles.iter().map(|r| r.to_string()).collect(),
        }
    }

    #[test]
    fn matches_original_neuron_run() {
        // Mirrors the neuron_run test from neurons/src/assigned_reputation.rs
        let input = Input {
            users: vec![
                user(
                    "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
                    2,
                    &["SDF", "SCF Project", "Moderator"],
                ),
                user(
                    "GAAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEAQDZ7H",
                    3,
                    &[],
                ),
                user(
                    "GABAEAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEJXA",
                    0,
                    &["Public Good Contributor"],
                ),
            ],
        };
        let output = calculate_result(33, &input);
        assert_eq!(output.current_round, 33);
        assert_eq!(
            output.scores[0],
            (
                "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF".to_string(),
                5.0
            )
        );
        assert_eq!(
            output.scores[1],
            (
                "GAAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEAQDZ7H".to_string(),
                3.0
            )
        );
        assert_eq!(
            output.scores[2],
            (
                "GABAEAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEJXA".to_string(),
                1.0
            )
        );
    }

    #[test]
    fn unknown_tier_and_roles_give_zero() {
        let input = Input {
            users: vec![user(
                "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
                -1,
                &["unrecognized role"],
            )],
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
                        users: vec![user(
                            "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
                            0,
                            &[role]
                        )],
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
                        users: vec![user(
                            "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
                            0,
                            &[role]
                        )],
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
                    user(
                        "GAAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEAQDZ7H",
                        99,
                        &["SDF", "SDF"],
                    ),
                    user(
                        "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
                        1,
                        &["Brazil Ambassador"],
                    ),
                ],
            },
        );
        assert_eq!(
            output.scores,
            vec![
                (
                    "GAAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEAQDZ7H".into(),
                    2.0
                ),
                (
                    "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF".into(),
                    1.5
                )
            ]
        );
    }
    #[test]
    fn example_input_uses_public_keys_and_preserves_scores() {
        let input: Input = serde_json::from_str(include_str!(
            "../../../data/example_assigned_reputation.json"
        ))
        .unwrap();
        input.validate().unwrap();
        let output = calculate_result(33, &input);
        assert_eq!(
            output.scores,
            vec![
                (input.users[0].public_key.clone(), 3.0),
                (input.users[1].public_key.clone(), 3.5),
            ]
        );
        assert_eq!(output.current_round, 33);
    }

    #[test]
    fn rejects_ids_invalid_keys_and_duplicate_public_keys() {
        let key = stellar_strkey::ed25519::PublicKey([0; 32]).to_string();
        let record = user(&key, 0, &[]);
        assert!(Input {
            users: vec![record.clone(), record]
        }
        .validate()
        .is_err());
        for invalid in [
            "",
            "0",
            "not-a-key",
            "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        ] {
            assert!(Input {
                users: vec![user(invalid, 0, &[])]
            }
            .validate()
            .is_err());
        }
        assert!(serde_json::from_str::<Input>(
            r#"{"users":[{"id":"0","tier":0,"discord_roles":[]}]}"#
        )
        .is_err());
    }
}
