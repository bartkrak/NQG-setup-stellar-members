use assigned_reputation_core::{calculate_result, Input, Output};
use risc0_zkvm::guest::env;

fn main() {
    let input: Input = env::read();
    let output: Output = calculate_result(&input);
    env::commit(&output);
}
