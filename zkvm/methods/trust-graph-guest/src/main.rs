use risc0_zkvm::guest::env;
use trust_graph_core::{calculate, Input};

fn main() {
    let current_round: u32 = env::read();
    let input: Input = env::read();
    let output = calculate(current_round, input).expect("Invalid trust graph input");
    env::commit(&output);
}
