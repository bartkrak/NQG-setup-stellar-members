pub use prior_voting_history_core::{calculate, Input};
use risc0_zkvm::guest::env;

fn main() {
    let current_round: u32 = env::read();
    let input: Input = env::read();
    let output = calculate(current_round, input).expect("Invalid voting-history input");
    env::commit(&output);
}
