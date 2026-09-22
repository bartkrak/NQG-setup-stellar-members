use soroban_sdk::testutils::{Address as _, MockAuth, MockAuthInvoke};
use soroban_sdk::{Address, Env, I256, IntoVal, Map, String, vec};

use governance::types::{Vote, VotingSystemError};
use governance::{VotingSystem, VotingSystemClient};

use crate::e2e::common::contract_utils::{MEMBERS, deploy_contract};
use crate::e2e::common::membership::{MockMembership, MockMembershipClient};

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

/// The membership mock behind `contract_client`, with member 2 revoked and
/// one more member minted. Returns the newcomer's id.
fn revoke_and_mint(env: &Env, contract_client: &VotingSystemClient) -> u32 {
    let members = MockMembershipClient::new(env, &contract_client.get_membership_contract());
    members.revoke(&2);
    let newcomer = members.mint(&Address::generate(env));
    assert_eq!(newcomer, MEMBERS);
    newcomer
}

#[test]
fn neuron_results_take_only_active_members() {
    let env = Env::default();
    let (contract_client, _admin) = deploy_contract(&env);
    env.mock_all_auths();
    let layer0 = String::from_str(&env, "0");
    let neuron0 = String::from_str(&env, "0");
    let newcomer = revoke_and_mint(&env, &contract_client);

    let mut accepted = Map::new(&env);
    accepted.set(1, I256::from_i128(&env, 100));
    accepted.set(newcomer, I256::from_i128(&env, 200));
    contract_client.set_neuron_result(&layer0, &neuron0, &accepted);
    assert_eq!(
        contract_client.get_neuron_result(&layer0, &neuron0),
        accepted
    );

    // A revoked member, then an id never minted: the whole map is refused
    // and the stored result stays
    for rejected_id in [2, 42] {
        let mut rejected = Map::new(&env);
        rejected.set(1, I256::from_i128(&env, 300));
        rejected.set(rejected_id, I256::from_i128(&env, 400));
        assert_eq!(
            contract_client
                .try_set_neuron_result(&layer0, &neuron0, &rejected)
                .unwrap_err()
                .unwrap(),
            VotingSystemError::NotAMember
        );
        assert_eq!(
            contract_client.get_neuron_result(&layer0, &neuron0),
            accepted
        );
    }
}

#[test]
fn votes_take_only_active_members() {
    let env = Env::default();
    let (contract_client, _admin) = deploy_contract(&env);
    env.mock_all_auths();
    let submission = String::from_str(&env, "sub1");
    contract_client.set_submissions(&vec![
        &env,
        (submission.clone(), String::from_str(&env, "Applications")),
    ]);
    let newcomer = revoke_and_mint(&env, &contract_client);

    let mut accepted = Map::new(&env);
    accepted.set(1, Vote::Yes);
    accepted.set(newcomer, Vote::No);
    contract_client.set_votes_for_submission(&submission, &accepted);
    assert_eq!(
        contract_client.get_votes_for_submission(&submission),
        accepted
    );

    for rejected_id in [2, 42] {
        let mut rejected = Map::new(&env);
        rejected.set(1, Vote::No);
        rejected.set(rejected_id, Vote::Yes);
        assert_eq!(
            contract_client
                .try_set_votes_for_submission(&submission, &rejected)
                .unwrap_err()
                .unwrap(),
            VotingSystemError::NotAMember
        );
        assert_eq!(
            contract_client.get_votes_for_submission(&submission),
            accepted
        );
    }
}
