use soroban_sdk::testutils::{Address as _, MockAuth, MockAuthInvoke};
use soroban_sdk::{Address, Env, IntoVal, vec};

use governance::{VotingSystem, VotingSystemClient};

use crate::e2e::common::contract_utils::deploy_contract;
use crate::e2e::common::membership::MockMembership;

#[test]
fn membership_contract_is_set_by_constructor() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let membership = env.register(MockMembership, ());
    let contract_id = env.register(VotingSystem, (admin, 25u32, membership.clone()));
    let contract_client = VotingSystemClient::new(&env, &contract_id);

    assert_eq!(contract_client.get_membership_contract(), membership);
}

#[test]
fn set_membership_contract_requires_admin() {
    let env = Env::default();
    let (contract_client, admin) = deploy_contract(&env);
    let before = contract_client.get_membership_contract();
    let replacement = Address::generate(&env);

    // Anyone else is refused and nothing changes
    env.mock_auths(&[MockAuth {
        address: &Address::generate(&env),
        invoke: &MockAuthInvoke {
            contract: &contract_client.address,
            fn_name: "set_membership_contract",
            args: vec![&env, replacement.into_val(&env)],
            sub_invokes: &[],
        },
    }]);
    assert!(
        contract_client
            .try_set_membership_contract(&replacement)
            .is_err()
    );
    assert_eq!(contract_client.get_membership_contract(), before);

    // The admin replaces it
    env.mock_auths(&[MockAuth {
        address: &admin,
        invoke: &MockAuthInvoke {
            contract: &contract_client.address,
            fn_name: "set_membership_contract",
            args: vec![&env, replacement.into_val(&env)],
            sub_invokes: &[],
        },
    }]);
    contract_client.set_membership_contract(&replacement);
    assert_eq!(contract_client.get_membership_contract(), replacement);
}
