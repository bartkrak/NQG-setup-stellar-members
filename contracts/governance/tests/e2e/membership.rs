use soroban_sdk::testutils::{Address as _, MockAuth, MockAuthInvoke};
use soroban_sdk::{Address, Env, IntoVal, Map, String, vec};

use governance::types::VotingSystemError;
use governance::{NeuronGuest, VotingSystem, VotingSystemClient};

use crate::e2e::common::contract_utils::{MEMBERS, deploy_contract, upload_neuron_result};
use crate::e2e::common::membership::{MockMembership, MockMembershipClient};
use crate::e2e::common::verifier::proof;

#[test]
fn membership_contract_is_set_by_constructor() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let membership = env.register(MockMembership, ());
    let verifier = Address::generate(&env);
    let contract_id = env.register(VotingSystem, (admin, 25u32, membership.clone(), verifier));
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
    members.revoke(&2, &true);
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
    let (journal, seal) = proof(&env);

    let mut accepted = Map::new(&env);
    accepted.set(1, 100);
    accepted.set(newcomer, 200);
    upload_neuron_result(&contract_client, &layer0, &neuron0, &accepted);
    assert_eq!(
        contract_client.get_neuron_result(&layer0, &neuron0),
        accepted
    );

    // A revoked member (which still has an owner), then an id never minted:
    // the whole map is refused and the stored result stays
    for rejected_id in [2, 42] {
        let mut rejected = Map::new(&env);
        rejected.set(1, 300);
        rejected.set(rejected_id, 400);
        assert_eq!(
            contract_client
                .try_set_neuron_result(
                    &layer0,
                    &neuron0,
                    &rejected,
                    &NeuronGuest::PriorVotingHistory,
                    &journal,
                    &seal
                )
                .unwrap_err()
                .unwrap(),
            VotingSystemError::NotAMember
        );
        assert_eq!(
            contract_client.get_neuron_result(&layer0, &neuron0),
            accepted
        );
    }

    // Reinstated, member 2 is accepted again
    MockMembershipClient::new(&env, &contract_client.get_membership_contract()).revoke(&2, &false);
    let mut reinstated = Map::new(&env);
    reinstated.set(2, 500);
    upload_neuron_result(&contract_client, &layer0, &neuron0, &reinstated);
    assert_eq!(
        contract_client.get_neuron_result(&layer0, &neuron0),
        reinstated
    );
}
