pub use prior_voting_history_core::{calculate, Input};
use risc0_zkvm::guest::env;

fn main() {
    let input: Input = env::read();
    let output = calculate(input).expect("Invalid voting-history input");
    env::commit(&output);
}
