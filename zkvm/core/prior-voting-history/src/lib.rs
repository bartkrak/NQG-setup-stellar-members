mod math;
mod neuron;
mod output;

pub use math::generalised_logistic_function;
pub use neuron::PriorVotingHistoryInput as Input;
pub use neuron::{PriorVotingHistoryInput, PriorVotingHistoryNeuron, Vote};
pub use output::{calculate, Output};
