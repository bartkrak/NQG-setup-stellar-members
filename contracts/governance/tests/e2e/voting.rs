use soroban_sdk::{
    Address, Env, I256, Map, String, Vec,
    testutils::{Address as _, MockAuth, MockAuthInvoke},
    vec,
};

use governance::types::VotingSystemError;
use governance::{DECIMALS, LayerAggregator};

use crate::e2e::common::contract_utils::deploy_contract;

#[allow(clippy::identity_op)]
#[test]
fn voting_powers_from_weighted_neurons() {
    let env = Env::default();
    let (contract_client, _admin) = deploy_contract(&env);
    env.cost_estimate().budget().reset_unlimited();
    env.mock_all_auths();

    let mut raw_neurons: Vec<(String, I256)> = Vec::new(&env);
    raw_neurons.push_back((
        String::from_str(&env, "Dummy"),
        I256::from_i128(&env, 2 * DECIMALS),
    ));
    raw_neurons.push_back((
        String::from_str(&env, "TrustGraph"),
        I256::from_i128(&env, 1 * DECIMALS),
    ));
    contract_client.add_layer(&raw_neurons, &LayerAggregator::Sum);

    let user1: u32 = 1;
    let user2: u32 = 2;
    let user3: u32 = 3;

    let mut neuron_result = Map::new(&env);
    neuron_result.set(user1, I256::from_i128(&env, 100 * DECIMALS));
    neuron_result.set(user2, I256::from_i128(&env, 200 * DECIMALS));
    neuron_result.set(user3, I256::from_i128(&env, 300 * DECIMALS));

    let mut neuron_result2 = Map::new(&env);
    neuron_result2.set(user1, I256::from_i128(&env, 1000 * DECIMALS));
    neuron_result2.set(user2, I256::from_i128(&env, 2000 * DECIMALS));
    neuron_result2.set(user3, I256::from_i128(&env, 3000 * DECIMALS));

    contract_client.set_neuron_result(
        &String::from_str(&env, "0"),
        &String::from_str(&env, "0"),
        &neuron_result,
    );
    contract_client.set_neuron_result(
        &String::from_str(&env, "0"),
        &String::from_str(&env, "1"),
        &neuron_result2,
    );

    env.cost_estimate().budget().reset_default();
    contract_client.calculate_voting_powers();
    println!("{}", env.cost_estimate().budget());

    // Neuron 0 weighs 2, neuron 1 weighs 1, the layer sums them
    let mut expected = Map::new(&env);
    expected.set(user1, I256::from_i128(&env, (100 * 2 + 1000) * DECIMALS));
    expected.set(user2, I256::from_i128(&env, (200 * 2 + 2000) * DECIMALS));
    expected.set(user3, I256::from_i128(&env, (300 * 2 + 3000) * DECIMALS));
    assert_eq!(contract_client.get_voting_powers(), expected);
    assert_eq!(
        contract_client.get_voting_power_for_id(&user2),
        I256::from_i128(&env, (200 * 2 + 2000) * DECIMALS)
    );
}

#[test]
fn calculate_voting_powers_requires_admin() {
    let env = Env::default();
    env.cost_estimate().budget().reset_unlimited();

    let (contract_client, admin) = deploy_contract(&env);
    env.mock_all_auths();

    let user: u32 = 1;
    contract_client.add_layer(
        &vec![
            &env,
            (
                String::from_str(&env, "Dummy"),
                I256::from_i128(&env, DECIMALS),
            ),
        ],
        &LayerAggregator::Sum,
    );
    let mut neuron_result = Map::new(&env);
    neuron_result.set(user, I256::from_i128(&env, 100 * DECIMALS));
    contract_client.set_neuron_result(
        &String::from_str(&env, "0"),
        &String::from_str(&env, "0"),
        &neuron_result,
    );

    // Anyone else is refused and no powers are written
    env.mock_auths(&[MockAuth {
        address: &Address::generate(&env),
        invoke: &MockAuthInvoke {
            contract: &contract_client.address,
            fn_name: "calculate_voting_powers",
            args: vec![&env],
            sub_invokes: &[],
        },
    }]);
    assert!(contract_client.try_calculate_voting_powers().is_err());
    assert_eq!(
        contract_client
            .try_get_voting_powers()
            .unwrap_err()
            .unwrap(),
        VotingSystemError::VotingPowersNotSet
    );

    // The admin calculates them
    env.mock_auths(&[MockAuth {
        address: &admin,
        invoke: &MockAuthInvoke {
            contract: &contract_client.address,
            fn_name: "calculate_voting_powers",
            args: vec![&env],
            sub_invokes: &[],
        },
    }]);
    contract_client.calculate_voting_powers();
    assert_eq!(
        contract_client.get_voting_power_for_id(&user),
        I256::from_i128(&env, 100 * DECIMALS)
    );
}

#[test]
fn setting_round() {
    let env = Env::default();
    env.cost_estimate().budget().reset_unlimited();

    let (contract_client, _admin) = deploy_contract(&env);
    env.mock_all_auths();

    contract_client.set_current_round(&20);
    assert_eq!(contract_client.get_current_round(), 20);

    contract_client.set_current_round(&30);
    assert_eq!(contract_client.get_current_round(), 30);
}

#[test]
#[allow(clippy::too_many_lines)]
fn set_bump_round_flow() {
    let env = Env::default();
    env.cost_estimate().budget().reset_unlimited();

    let (contract_client, _admin) = deploy_contract(&env);
    env.mock_all_auths();

    contract_client.set_current_round(&25);

    let user1: u32 = 1;
    let user2: u32 = 2;
    let neuron0 = String::from_str(&env, "0");
    let layer0 = String::from_str(&env, "0");

    // Setup contract
    contract_client.add_layer(
        &soroban_sdk::vec![
            &env,
            (
                neuron0.clone(),
                I256::from_i128(&env, 1_000_000_000_000_000_000)
            )
        ],
        &LayerAggregator::Sum,
    );

    // Set results for round 25
    let mut result25 = Map::new(&env);
    result25.set(user1, I256::from_i128(&env, 100));
    result25.set(user2, I256::from_i128(&env, 200));
    contract_client.set_neuron_result(&layer0, &neuron0, &result25);

    // Verify results are set
    assert_eq!(
        contract_client.get_neuron_result(&layer0, &neuron0),
        result25
    );
    contract_client.calculate_voting_powers();
    assert_eq!(
        contract_client.get_voting_power_for_id(&user1),
        I256::from_i128(&env, 100)
    );

    // Bump the round
    contract_client.set_current_round(&26);

    // Verify results and powers are unset for the new round
    assert_eq!(
        contract_client
            .try_get_voting_powers()
            .unwrap_err()
            .unwrap(),
        VotingSystemError::VotingPowersNotSet
    );
    assert_eq!(
        contract_client
            .try_get_neuron_result(&layer0, &neuron0)
            .unwrap_err()
            .unwrap(),
        VotingSystemError::NeuronResultNotSet
    );

    // Set results for round 26
    let mut result26 = Map::new(&env);
    result26.set(user1, I256::from_i128(&env, 5000));
    result26.set(user2, I256::from_i128(&env, 6000));
    contract_client.set_neuron_result(&layer0, &neuron0, &result26);

    // Verify results are set
    assert_eq!(
        contract_client.get_neuron_result(&layer0, &neuron0),
        result26
    );
    contract_client.calculate_voting_powers();
    assert_eq!(
        contract_client.get_voting_power_for_id(&user1),
        I256::from_i128(&env, 5000)
    );

    // Verify historical results are still accessible
    assert_eq!(
        contract_client.get_neuron_result_round(&layer0, &neuron0, &25),
        result25
    );
}

#[test]
fn get_voting_power_for_id() {
    let env = Env::default();
    env.cost_estimate().budget().reset_unlimited();

    let (contract_client, _admin) = deploy_contract(&env);
    env.mock_all_auths();

    contract_client.set_current_round(&25);

    let user1: u32 = 1;
    let user2: u32 = 2;
    let neuron0 = String::from_str(&env, "0");
    let neuron1 = String::from_str(&env, "1");
    let layer0 = String::from_str(&env, "0");

    // Setup contract
    contract_client.add_layer(
        &soroban_sdk::vec![
            &env,
            (
                neuron0.clone(),
                I256::from_i128(&env, 1_000_000_000_000_000_000)
            ),
            (
                neuron1.clone(),
                I256::from_i128(&env, 1_000_000_000_000_000_000)
            )
        ],
        &LayerAggregator::Sum,
    );

    let mut result0 = Map::new(&env);
    result0.set(user1, I256::from_i128(&env, 100));
    result0.set(user2, I256::from_i128(&env, 200));
    contract_client.set_neuron_result(&layer0, &neuron0, &result0);

    let mut result1 = Map::new(&env);
    result1.set(user1, I256::from_i128(&env, 222));
    result1.set(user2, I256::from_i128(&env, 333));
    contract_client.set_neuron_result(&layer0, &neuron1, &result1);

    // Verify results are set
    assert_eq!(
        contract_client.get_neuron_result(&layer0, &neuron0),
        result0
    );
    assert_eq!(
        contract_client.get_neuron_result(&layer0, &neuron1),
        result1
    );
    contract_client.calculate_voting_powers();
    // Verify correct voting powers are returned for each user
    assert_eq!(
        contract_client.get_voting_power_for_id(&user1),
        I256::from_i32(&env, 322)
    );
    assert_eq!(
        contract_client.get_voting_power_for_id(&user2),
        I256::from_i32(&env, 533)
    );
    // Verify error is returned for invalid user
    assert_eq!(
        contract_client
            .try_get_voting_power_for_id(&99)
            .unwrap_err()
            .unwrap(),
        VotingSystemError::NGQResultForVoterMissing
    );
}

#[test]
fn calculate_voting_powers_clamps_negative_nqg_to_zero() {
    let env = Env::default();
    env.cost_estimate().budget().reset_unlimited();

    let (contract_client, _admin) = deploy_contract(&env);
    env.mock_all_auths();

    contract_client.set_current_round(&25);

    let user_negative: u32 = 1;
    let user_positive: u32 = 2;
    let user_recovers: u32 = 3;
    let neuron0 = String::from_str(&env, "0");
    let layer0 = String::from_str(&env, "0");
    let layer1 = String::from_str(&env, "1");

    contract_client.add_layer(
        &soroban_sdk::vec![
            &env,
            (
                String::from_str(&env, "L0"),
                I256::from_i128(&env, DECIMALS)
            )
        ],
        &LayerAggregator::Sum,
    );
    contract_client.add_layer(
        &soroban_sdk::vec![
            &env,
            (
                String::from_str(&env, "L1"),
                I256::from_i128(&env, DECIMALS)
            )
        ],
        &LayerAggregator::Sum,
    );

    let mut layer0_result = Map::new(&env);
    layer0_result.set(user_negative, I256::from_i128(&env, 100));
    layer0_result.set(user_positive, I256::from_i128(&env, 200));
    layer0_result.set(user_recovers, I256::from_i128(&env, -100));
    contract_client.set_neuron_result(&layer0, &neuron0, &layer0_result);

    let mut layer1_result = Map::new(&env);
    layer1_result.set(user_negative, I256::from_i128(&env, -500));
    layer1_result.set(user_positive, I256::from_i128(&env, 300));
    layer1_result.set(user_recovers, I256::from_i128(&env, 300));
    contract_client.set_neuron_result(&layer1, &neuron0, &layer1_result);

    contract_client.calculate_voting_powers();

    assert_eq!(
        contract_client.get_voting_power_for_id(&user_negative),
        I256::from_i32(&env, 0)
    );
    assert_eq!(
        contract_client.get_voting_power_for_id(&user_positive),
        I256::from_i32(&env, 500)
    );
    assert_eq!(
        contract_client.get_voting_power_for_id(&user_recovers),
        I256::from_i32(&env, 200)
    );
}

#[test]
fn calculate_voting_powers_rejects_layers_with_mismatched_users() {
    let env = Env::default();
    env.cost_estimate().budget().reset_unlimited();

    let (contract_client, _admin) = deploy_contract(&env);
    env.mock_all_auths();

    contract_client.set_current_round(&25);

    let user_negative: u32 = 1;
    let user_positive: u32 = 2;
    let neuron0 = String::from_str(&env, "0");
    let layer0 = String::from_str(&env, "0");
    let layer1 = String::from_str(&env, "1");

    contract_client.add_layer(
        &soroban_sdk::vec![
            &env,
            (
                String::from_str(&env, "L0"),
                I256::from_i128(&env, DECIMALS)
            )
        ],
        &LayerAggregator::Sum,
    );
    contract_client.add_layer(
        &soroban_sdk::vec![
            &env,
            (
                String::from_str(&env, "L1"),
                I256::from_i128(&env, DECIMALS)
            )
        ],
        &LayerAggregator::Sum,
    );

    let mut layer0_result = Map::new(&env);
    layer0_result.set(user_negative, I256::from_i128(&env, -100));
    layer0_result.set(user_positive, I256::from_i128(&env, 200));
    contract_client.set_neuron_result(&layer0, &neuron0, &layer0_result);

    // user_negative is absent from the last layer, so its -100 would never be clamped
    let mut layer1_result = Map::new(&env);
    layer1_result.set(user_positive, I256::from_i128(&env, 300));
    contract_client.set_neuron_result(&layer1, &neuron0, &layer1_result);

    assert_eq!(
        contract_client
            .try_calculate_voting_powers()
            .unwrap_err()
            .unwrap(),
        VotingSystemError::LayerResultsUsersMismatch
    );

    // Same user count per layer but different users must be rejected too
    let mut layer1_result = Map::new(&env);
    layer1_result.set(user_positive, I256::from_i128(&env, 300));
    layer1_result.set(3, I256::from_i128(&env, 300));
    contract_client.set_neuron_result(&layer1, &neuron0, &layer1_result);

    assert_eq!(
        contract_client
            .try_calculate_voting_powers()
            .unwrap_err()
            .unwrap(),
        VotingSystemError::LayerResultsUsersMismatch
    );

    // With user sets aligned the calculation succeeds and the clamp applies
    let mut layer1_result = Map::new(&env);
    layer1_result.set(user_negative, I256::from_i128(&env, 50));
    layer1_result.set(user_positive, I256::from_i128(&env, 300));
    contract_client.set_neuron_result(&layer1, &neuron0, &layer1_result);

    contract_client.calculate_voting_powers();

    assert_eq!(
        contract_client.get_voting_power_for_id(&user_negative),
        I256::from_i32(&env, 0)
    );
    assert_eq!(
        contract_client.get_voting_power_for_id(&user_positive),
        I256::from_i32(&env, 500)
    );
}
