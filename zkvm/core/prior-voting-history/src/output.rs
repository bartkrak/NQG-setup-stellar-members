//! Public proof result. No scoring rules live here.
use crate::Input;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Output {
    pub current_round: u32,
    pub scores: BTreeMap<String, f64>,
}

/// Attach the public round to the existing neuron's scores.
pub fn calculate(input: Input) -> Result<Output> {
    let current_round = input.current_round;
    let scores = input.calculate()?;
    Ok(Output {
        current_round,
        scores,
    })
}
