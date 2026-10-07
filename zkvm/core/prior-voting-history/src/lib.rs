mod math;
mod neuron;
mod output;

pub use math::generalised_logistic_function;
pub use neuron::PriorVotingHistoryInput as Input;
pub use neuron::{validate_current_round, PriorVotingHistoryInput, PriorVotingHistoryNeuron, Vote};
pub use output::{calculate, Output};
