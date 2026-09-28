use std::collections::HashMap;

pub trait Neuron {
    fn name(&self) -> String;

    /// Result for each voter, keyed by Stellar Membership token id.
    fn calculate_result(&self, users: &[u32]) -> HashMap<u32, f64>;
}
