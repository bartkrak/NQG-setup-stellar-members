#![allow(non_upper_case_globals)]
use crate::ContractResult;
use crate::fixed_mul_floor::fixed_mul_floor;
use crate::types::VotingSystemError;

use soroban_sdk::{Env, Map, String, Vec, contracttype};

pub mod traits;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LayerAggregator {
    Sum,
    Product,
}

#[contracttype]
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Neuron {
    pub name: String,
    /// Fixed point with `DECIMALS`: 1.0 is `1_000_000`.
    pub weight: i64,
}

impl Neuron {
    pub fn create(name: String, weight: i64) -> Self {
        Self { name, weight }
    }
}

#[contracttype]
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Layer {
    /// Vec of `neuron_id`s
    pub neurons: Vec<String>,
    pub aggregator: LayerAggregator,
}

impl Layer {
    pub fn create(neurons: Vec<String>, aggregator: LayerAggregator) -> Self {
        Self {
            neurons,
            aggregator,
        }
    }
}

#[allow(clippy::upper_case_acronyms)]
#[contracttype]
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NGQ {
    /// Vec of `layer_id`s
    pub layers: Vec<String>,
}

impl NGQ {
    pub fn new(env: &Env) -> Self {
        Self {
            layers: Vec::new(env),
        }
    }
}

/// Aggregate each member's weighted neuron results into the layer result.
///
/// `ArithmeticOverflow` if a result does not fit an `i64`.
pub(crate) fn aggregate_result(
    env: &Env,
    result: Map<u32, Vec<i64>>,
    layer_aggregator: LayerAggregator,
    decimals: i64,
) -> ContractResult<Map<u32, i64>> {
    let mut aggregated_result = Map::new(env);
    for (user, res) in result {
        let mut values = res.iter();
        let aggregated = match values.next() {
            None => Some(0),
            Some(first) => values.try_fold(first, |acc, e| match layer_aggregator {
                LayerAggregator::Sum => acc.checked_add(e),
                LayerAggregator::Product => fixed_mul_floor(acc, e, decimals),
            }),
        }
        .ok_or(VotingSystemError::ArithmeticOverflow)?;
        aggregated_result.set(user, aggregated);
    }
    Ok(aggregated_result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::vec;

    #[test]
    fn creating_neuron() {
        let env = Env::default();

        let name = String::from_str(&env, "abc");
        let weight = 100;
        let neuron = Neuron::create(name.clone(), weight);
        assert_eq!(neuron, Neuron { name, weight });
    }

    #[test]
    fn creating_layer() {
        let env = Env::default();

        let layer = Layer::create(
            vec![&env, String::from_str(&env, "neuron1")],
            LayerAggregator::Sum,
        );
        assert_eq!(
            layer,
            Layer {
                neurons: vec![&env, String::from_str(&env, "neuron1")],
                aggregator: LayerAggregator::Sum
            }
        );
    }

    #[test]
    fn creating_ngq() {
        let env = Env::default();

        let ngq = NGQ::new(&env);
        assert_eq!(ngq, NGQ { layers: vec![&env] });
    }

    #[test]
    fn aggregate_empty() {
        let env = Env::default();

        let user1: u32 = 1;

        let mut result: Map<u32, Vec<i64>> = Map::new(&env);
        result.set(user1, vec![&env]);

        let aggregated = aggregate_result(&env, result, LayerAggregator::Product, 1).unwrap();
        assert_eq!(aggregated.get(user1).unwrap(), 0);
    }

    #[test]
    fn aggregate_sum() {
        let env = Env::default();

        let user1: u32 = 1;
        let user2: u32 = 2;

        let mut result: Map<u32, Vec<i64>> = Map::new(&env);
        result.set(user1, vec![&env, 1, 2]);
        result.set(user2, vec![&env, 3, 4]);

        let aggregated = aggregate_result(&env, result, LayerAggregator::Sum, 1).unwrap();
        assert_eq!(aggregated.get(user1).unwrap(), 3);
        assert_eq!(aggregated.get(user2).unwrap(), 7);
    }

    #[test]
    fn aggregate_product() {
        let env = Env::default();

        let user1: u32 = 1;
        let user2: u32 = 2;

        let mut result: Map<u32, Vec<i64>> = Map::new(&env);
        result.set(user1, vec![&env, 1, 2]);
        result.set(user2, vec![&env, 3, 4]);

        let aggregated = aggregate_result(&env, result, LayerAggregator::Product, 1).unwrap();
        assert_eq!(aggregated.get(user1).unwrap(), 2);
        assert_eq!(aggregated.get(user2).unwrap(), 12);
    }

    #[test]
    fn aggregate_overflow() {
        let env = Env::default();

        let mut result: Map<u32, Vec<i64>> = Map::new(&env);
        result.set(1, vec![&env, i64::MAX, 1]);

        assert_eq!(
            aggregate_result(&env, result.clone(), LayerAggregator::Sum, 1),
            Err(VotingSystemError::ArithmeticOverflow)
        );
        assert_eq!(
            aggregate_result(&env, result, LayerAggregator::Product, 1).map(|r| r.get(1)),
            Ok(Some(i64::MAX))
        );
    }
}
