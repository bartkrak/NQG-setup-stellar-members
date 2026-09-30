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

/// Results are i64 fixed point numbers with 6 decimals, as the NQG contract
/// stores them: 1.0 is `1_000_000`.
pub const DECIMALS: i64 = 1_000_000;

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
/// parses back into the contract's `u32` keys. Values are JSON numbers: the
/// CLI accepts an i64 only as a number.
fn calculate_neuron_results(
    users: &[u32],
    neurons: Vec<Box<dyn Neuron>>,
) -> BTreeMap<String, BTreeMap<u32, i64>> {
    let mut results: BTreeMap<String, BTreeMap<u32, i64>> = BTreeMap::new();
    for neuron in neurons {
        println!("running {}", neuron.name());
        let result = neuron.calculate_result(users);
        let result: BTreeMap<u32, i64> = result
            .into_iter()
            .map(|(key, value)| (key, to_fixed_point_decimal(value)))
            .collect();
        results.insert(neuron.name(), result);
    }
    results
}

/// Rounded to the nearest 0.000001, so 0.8 * 44 gives 35.2 and not 35.199999.
///
/// # Panics
///
/// If the value is not finite or does not fit an i64 at 6 decimals (about
/// 9.2 trillion).
#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn to_fixed_point_decimal(val: f64) -> i64 {
    let scaled = (val * DECIMALS as f64).round();
    assert!(
        scaled.is_finite() && scaled.abs() < i64::MAX as f64,
        "{val} does not fit an i64 with 6 decimals"
    );
    scaled as i64
}
