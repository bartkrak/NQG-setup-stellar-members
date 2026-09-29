use std::{collections::HashMap, fs::File, io::BufReader};

use crate::neurons::Neuron;

pub struct Neuron3 {
    data: HashMap<u32, f64>,
}
impl Neuron3 {
    pub fn from_json(path: &str) -> Neuron3 {
        let file = File::open(path).unwrap();
        let reader = BufReader::new(file);
        let data: HashMap<u32, f64> = serde_json::from_reader(reader).unwrap();

        Neuron3 { data }
    }

    fn bonus(input_value: f64) -> f64 {
        input_value * 3.0
    }
}
impl Neuron for Neuron3 {
    fn name(&self) -> String {
        String::from("Neuron3")
    }

    fn calculate_result(&self, users: &[u32]) -> HashMap<u32, f64> {
        let mut result = HashMap::new();

        for user in users {
            let bonus: f64 = Neuron3::bonus(*self.data.get(user).unwrap());
            result.insert(*user, bonus);
        }

        result
    }
}
