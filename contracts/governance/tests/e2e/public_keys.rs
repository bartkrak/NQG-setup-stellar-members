use governance::{DECIMALS, LayerAggregator};
use soroban_sdk::{Env, Map, String, vec};

use crate::e2e::common::contract_utils::{deploy_contract, public_key, upload_neuron_result};

#[test]
fn public_keys_work_without_membership() {
    let env = Env::default();
    let (client, _) = deploy_contract(&env);
    env.mock_all_auths();
    let layer = String::from_str(&env, "0");
    let neuron = String::from_str(&env, "0");
    let first = public_key(&env, 1);
    let second = public_key(&env, 42);
    let mut result = Map::new(&env);
    result.set(first.clone(), 100);
    result.set(second.clone(), 200);

    client.add_layer(
        &vec![&env, (String::from_str(&env, "Public keys"), DECIMALS)],
        &LayerAggregator::Sum,
    );
    upload_neuron_result(&client, &layer, &neuron, &result);
    assert_eq!(client.get_neuron_result(&layer, &neuron), result);
    client.calculate_voting_powers();
    assert_eq!(client.get_voting_powers(), result);
    assert_eq!(client.get_voting_power_for_user(&first), 100);
    assert_eq!(client.get_voting_power_for_user(&second), 200);
}
