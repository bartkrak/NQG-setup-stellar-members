use soroban_sdk::testutils::{Address as _, MockAuth, MockAuthInvoke};
use soroban_sdk::{Address, Bytes, BytesN, Env, IntoVal, vec};

use governance::types::VotingSystemError;
use governance::{KeyWrap, MAX_TRUST_LIST_SIZE, TrustList, VotingSystemClient};

use crate::e2e::common::contract_utils::deploy_contract;
use crate::e2e::common::membership::MockMembershipClient;

const OPERATOR_KEY: [u8; 32] = [7; 32];

fn wrap(env: &Env, key: [u8; 32]) -> KeyWrap {
    KeyWrap {
        key: BytesN::from_array(env, &key),
        ephemeral: BytesN::from_array(env, &[1; 32]),
        sealed: BytesN::from_array(env, &[2; 48]),
    }
}

/// A list of `len` bytes, filled with `fill`, sealed for `operator_key`.
fn trust_list(env: &Env, fill: u8, len: u32, operator_key: [u8; 32]) -> TrustList {
    let mut list = Bytes::new(env);
    for _ in 0..len {
        list.push_back(fill);
    }
    TrustList {
        list,
        member: wrap(env, [fill; 32]),
        operator: wrap(env, operator_key),
    }
}

fn set_trust_key_as(
    env: &Env,
    contract_client: &VotingSystemClient,
    signer: &Address,
    key: [u8; 32],
) {
    let key = BytesN::from_array(env, &key);
    env.mock_auths(&[MockAuth {
        address: signer,
        invoke: &MockAuthInvoke {
            contract: &contract_client.address,
            fn_name: "set_trust_key",
            args: vec![env, key.into_val(env)],
            sub_invokes: &[],
        },
    }]);
    contract_client.set_trust_key(&key);
}

/// The contract with the operator key set.
fn deploy_with_trust_key(env: &Env) -> VotingSystemClient<'_> {
    let (contract_client, admin) = deploy_contract(env);
    set_trust_key_as(env, &contract_client, &admin, OPERATOR_KEY);
    contract_client
}

/// Call `set_trust_list` with only `signer`'s authorization.
fn set_trust_list_as(
    env: &Env,
    contract_client: &VotingSystemClient,
    signer: &Address,
    member_id: u32,
    trust_list: &TrustList,
) -> Result<(), VotingSystemError> {
    env.mock_auths(&[MockAuth {
        address: signer,
        invoke: &MockAuthInvoke {
            contract: &contract_client.address,
            fn_name: "set_trust_list",
            args: vec![env, member_id.into_val(env), trust_list.into_val(env)],
            sub_invokes: &[],
        },
    }]);
    match contract_client.try_set_trust_list(&member_id, trust_list) {
        Ok(_) => Ok(()),
        Err(Ok(error)) => Err(error),
        Err(Err(_)) => Err(VotingSystemError::UnknownError),
    }
}

#[test]
fn trust_key_is_set_by_the_admin() {
    let env = Env::default();
    let (contract_client, admin) = deploy_contract(&env);

    assert_eq!(
        contract_client.try_get_trust_key().unwrap_err().unwrap(),
        VotingSystemError::TrustKeyNotSet
    );

    let key = BytesN::from_array(&env, &OPERATOR_KEY);
    env.mock_auths(&[MockAuth {
        address: &Address::generate(&env),
        invoke: &MockAuthInvoke {
            contract: &contract_client.address,
            fn_name: "set_trust_key",
            args: vec![&env, key.into_val(&env)],
            sub_invokes: &[],
        },
    }]);
    assert!(contract_client.try_set_trust_key(&key).is_err());

    set_trust_key_as(&env, &contract_client, &admin, OPERATOR_KEY);
    assert_eq!(contract_client.get_trust_key(), key);
}

#[test]
fn a_member_writes_only_their_own_list() {
    let env = Env::default();
    let contract_client = deploy_with_trust_key(&env);
    let members = MockMembershipClient::new(&env, &contract_client.get_membership_contract());
    let owner = members.owner_of(&1);
    let first = trust_list(&env, 1, 286, OPERATOR_KEY);
    let second = trust_list(&env, 2, 286, OPERATOR_KEY);

    assert_eq!(contract_client.get_trust_list(&1), None);

    set_trust_list_as(&env, &contract_client, &owner, 1, &first).unwrap();
    assert_eq!(contract_client.get_trust_list(&1), Some(first.clone()));

    // Not another member, whose own token does not count here
    assert!(set_trust_list_as(&env, &contract_client, &members.owner_of(&2), 1, &second).is_err());
    assert_eq!(contract_client.get_trust_list(&1), Some(first));

    // The owner replaces it
    set_trust_list_as(&env, &contract_client, &owner, 1, &second).unwrap();
    assert_eq!(contract_client.get_trust_list(&1), Some(second));
}

#[test]
fn the_list_follows_the_token_across_a_key_move() {
    let env = Env::default();
    let contract_client = deploy_with_trust_key(&env);
    let members = MockMembershipClient::new(&env, &contract_client.get_membership_contract());
    let old_key = members.owner_of(&1);
    let new_key = Address::generate(&env);
    let saved = trust_list(&env, 1, 286, OPERATOR_KEY);
    set_trust_list_as(&env, &contract_client, &old_key, 1, &saved).unwrap();

    members.move_key(&1, &new_key);

    assert_eq!(contract_client.get_trust_list(&1), Some(saved));
    let replaced = trust_list(&env, 2, 286, OPERATOR_KEY);
    assert!(set_trust_list_as(&env, &contract_client, &old_key, 1, &replaced).is_err());
    set_trust_list_as(&env, &contract_client, &new_key, 1, &replaced).unwrap();
    assert_eq!(contract_client.get_trust_list(&1), Some(replaced));
}

#[test]
fn revoked_and_unknown_members_cannot_write() {
    let env = Env::default();
    let contract_client = deploy_with_trust_key(&env);
    let members = MockMembershipClient::new(&env, &contract_client.get_membership_contract());
    let saved = trust_list(&env, 1, 286, OPERATOR_KEY);
    let owner = members.owner_of(&2);
    members.revoke(&2, &true);

    assert_eq!(
        set_trust_list_as(&env, &contract_client, &owner, 2, &saved),
        Err(VotingSystemError::NotAMember)
    );
    assert_eq!(
        set_trust_list_as(&env, &contract_client, &owner, 42, &saved),
        Err(VotingSystemError::NotAMember)
    );
    assert_eq!(contract_client.get_trust_list(&2), None);

    members.revoke(&2, &false);
    set_trust_list_as(&env, &contract_client, &owner, 2, &saved).unwrap();
}

#[test]
fn the_operator_copy_must_be_for_the_trust_key() {
    let env = Env::default();
    let (contract_client, admin) = deploy_contract(&env);
    let owner =
        MockMembershipClient::new(&env, &contract_client.get_membership_contract()).owner_of(&1);
    let sealed = trust_list(&env, 1, 286, OPERATOR_KEY);

    assert_eq!(
        set_trust_list_as(&env, &contract_client, &owner, 1, &sealed),
        Err(VotingSystemError::TrustKeyNotSet)
    );

    set_trust_key_as(&env, &contract_client, &admin, OPERATOR_KEY);
    assert_eq!(
        set_trust_list_as(
            &env,
            &contract_client,
            &owner,
            1,
            &trust_list(&env, 1, 286, [8; 32])
        ),
        Err(VotingSystemError::TrustKeyMismatch)
    );
    set_trust_list_as(&env, &contract_client, &owner, 1, &sealed).unwrap();

    // A new operator key refuses lists sealed for the previous one
    set_trust_key_as(&env, &contract_client, &admin, [8; 32]);
    assert_eq!(
        set_trust_list_as(&env, &contract_client, &owner, 1, &sealed),
        Err(VotingSystemError::TrustKeyMismatch)
    );
}

#[test]
fn lists_are_limited_in_size() {
    let env = Env::default();
    let contract_client = deploy_with_trust_key(&env);
    let owner =
        MockMembershipClient::new(&env, &contract_client.get_membership_contract()).owner_of(&1);

    let largest = trust_list(&env, 1, MAX_TRUST_LIST_SIZE, OPERATOR_KEY);
    set_trust_list_as(&env, &contract_client, &owner, 1, &largest).unwrap();
    assert_eq!(
        set_trust_list_as(
            &env,
            &contract_client,
            &owner,
            1,
            &trust_list(&env, 2, MAX_TRUST_LIST_SIZE + 1, OPERATOR_KEY)
        ),
        Err(VotingSystemError::TrustListTooLarge)
    );
    assert_eq!(contract_client.get_trust_list(&1), Some(largest));
}
