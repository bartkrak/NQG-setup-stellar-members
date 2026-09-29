mod neuron1;
mod neuron2;
mod neuron3;

mod neurons;
use std::{
    collections::BTreeMap,
    fs::File,
    io::{BufReader, BufWriter},
};

use neuron1::Neuron1;
use neuron2::Neuron2;
use neuron3::Neuron3;

use crate::neurons::Neuron;

pub const DECIMALS: i64 = 1_000_000_000_000_000_000;

fn main() {
    println!("Calculating neurons results...");

    // 1. create neurons
    let neuron1 = Neuron1::from_json("../data/neuron1_input.json");
    let neuron2 = Neuron2::from_json("../data/neuron2_input.json");
    let neuron3 = Neuron3::from_json("../data/neuron3_input.json");

    // 2. read voters list file: Stellar Membership token ids
    let file = File::open("../data/voters.json").unwrap();
    let reader = BufReader::new(file);
    let users: Vec<u32> = serde_json::from_reader(reader).unwrap();

    // 3. run neurons
    let results = calculate_neuron_results(
        &users,
        vec![Box::new(neuron1), Box::new(neuron2), Box::new(neuron3)],
    );

    // 4. save results to files
    let file = File::create("../data/neurons_output.json").unwrap();
    let writer = BufWriter::new(file);
    serde_json::to_writer_pretty(writer, &results).unwrap();

    println!("Done.");
}

/// Neuron name to its results, keyed by token id. JSON object keys are
/// strings, so ids are written as `"0"`, `"1"`, ..., which the stellar CLI
/// parses back into the contract's `u32` keys.
fn calculate_neuron_results(
    users: &[u32],
    neurons: Vec<Box<dyn Neuron>>,
) -> BTreeMap<String, BTreeMap<u32, String>> {
    let mut results: BTreeMap<String, BTreeMap<u32, String>> = BTreeMap::new();
    for neuron in neurons {
        println!("running {}", neuron.name());
        let result = neuron.calculate_result(users);
        let result: BTreeMap<u32, String> = result
            .into_iter()
            .map(|(key, value)| (key, to_fixed_point_decimal(value).to_string()))
            .collect();
        results.insert(neuron.name(), result);
    }
    results
}

#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
fn to_fixed_point_decimal(val: f64) -> i128 {
    (val * DECIMALS as f64) as i128
}
