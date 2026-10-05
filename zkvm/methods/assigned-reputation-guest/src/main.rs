use assigned_reputation_core::{calculate_result, Input, Output};
use risc0_zkvm::guest::env;

fn main() {
    let current_round: u32 = env::read();
    let input: Input = env::read();
    let output: Output = calculate_result(current_round, &input);
    env::commit(&output);
}
