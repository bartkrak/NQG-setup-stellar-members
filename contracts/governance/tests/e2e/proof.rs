use soroban_sdk::testutils::{Address as _, MockAuth, MockAuthInvoke};
use soroban_sdk::{Address, Bytes, BytesN, Env, IntoVal, Map, String, vec};

use governance::types::VotingSystemError;
use governance::{NeuronGuest, NeuronProof, VotingSystem, VotingSystemClient};

use crate::e2e::common::contract_utils::deploy_contract;
use crate::e2e::common::membership::MockMembership;
use crate::e2e::common::verifier::{MockVerifier, MockVerifierClient};

fn digest(env: &Env, journal: &Bytes) -> BytesN<32> {
    env.crypto().sha256(journal).into()
}

fn scores(env: &Env, member_id: u32, score: i64) -> Map<u32, i64> {
    let mut result = Map::new(env);
    result.set(member_id, score);
    result
}

/// The verifier mock behind `contract_client`.
fn verifier<'a>(env: &Env, contract_client: &VotingSystemClient) -> MockVerifierClient<'a> {
    MockVerifierClient::new(env, &contract_client.get_verifier())
}

#[test]
fn verifier_is_set_by_constructor() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let membership = env.register(MockMembership, ());
    let verifier = env.register(MockVerifier, ());
    let contract_id = env.register(VotingSystem, (admin, 25u32, membership, verifier.clone()));
    let contract_client = VotingSystemClient::new(&env, &contract_id);

    assert_eq!(contract_client.get_verifier(), verifier);
}

#[test]
fn set_verifier_requires_admin() {
    let env = Env::default();
    let (contract_client, admin) = deploy_contract(&env);
    let before = contract_client.get_verifier();
    let replacement = Address::generate(&env);

    // Anyone else is refused and nothing changes
    env.mock_auths(&[MockAuth {
        address: &Address::generate(&env),
        invoke: &MockAuthInvoke {
            contract: &contract_client.address,
            fn_name: "set_verifier",
            args: vec![&env, replacement.into_val(&env)],
            sub_invokes: &[],
        },
    }]);
    assert!(contract_client.try_set_verifier(&replacement).is_err());
    assert_eq!(contract_client.get_verifier(), before);

    // The admin replaces it
    env.mock_auths(&[MockAuth {
        address: &admin,
        invoke: &MockAuthInvoke {
            contract: &contract_client.address,
            fn_name: "set_verifier",
            args: vec![&env, replacement.into_val(&env)],
            sub_invokes: &[],
        },
    }]);
    contract_client.set_verifier(&replacement);
    assert_eq!(contract_client.get_verifier(), replacement);
}

#[test]
fn stores_result_and_proof_after_verify_sees_the_digest() {
    let env = Env::default();
    let (contract_client, _admin) = deploy_contract(&env);
    env.mock_all_auths();
    let layer0 = String::from_str(&env, "0");
    let neuron0 = String::from_str(&env, "0");
    let result = scores(&env, 1, 1_500_000);
    let journal = Bytes::from_slice(&env, b"scores");
    let seal = Bytes::from_slice(&env, &[0x73, 0xc4, 0x57, 0xba, 0x01]);
    let guest = NeuronGuest::AssignedReputation;

    contract_client.set_neuron_result(&layer0, &neuron0, &result, &guest, &journal, &seal);

    let expected = NeuronProof {
        seal: seal.clone(),
        image_id: guest.image_id(&env),
        journal_digest: digest(&env, &journal),
    };
    assert_eq!(contract_client.get_neuron_result(&layer0, &neuron0), result);
    assert_eq!(
        contract_client.get_neuron_proof(&layer0, &neuron0),
        expected
    );
    assert_eq!(
        verifier(&env, &contract_client).seen(),
        Some((seal, expected.image_id, expected.journal_digest))
    );
}

#[test]
fn each_guest_is_verified_against_its_own_image_id() {
    let env = Env::default();
    let (contract_client, _admin) = deploy_contract(&env);
    env.mock_all_auths();
    let layer0 = String::from_str(&env, "0");
    let neuron0 = String::from_str(&env, "0");
    let result = scores(&env, 1, 1);
    let journal = Bytes::from_slice(&env, b"scores");
    let seal = Bytes::from_slice(&env, &[9]);
    let guests = [
        NeuronGuest::PriorVotingHistory,
        NeuronGuest::AssignedReputation,
        NeuronGuest::TrustGraph,
    ];

    for guest in guests {
        contract_client.set_neuron_result(&layer0, &neuron0, &result, &guest, &journal, &seal);
        let image_id = guest.image_id(&env);
        assert_eq!(verifier(&env, &contract_client).seen().unwrap().1, image_id);
        assert_eq!(
            contract_client.get_neuron_proof(&layer0, &neuron0).image_id,
            image_id
        );
    }
    for (i, a) in guests.iter().enumerate() {
        for b in &guests[i + 1..] {
            assert_ne!(a.image_id(&env), b.image_id(&env));
        }
    }
}

#[test]
fn rejected_proof_stores_nothing() {
    let env = Env::default();
    let (contract_client, _admin) = deploy_contract(&env);
    env.mock_all_auths();
    let layer0 = String::from_str(&env, "0");
    let neuron0 = String::from_str(&env, "0");
    let neuron1 = String::from_str(&env, "1");
    let guest = NeuronGuest::PriorVotingHistory;
    let seal = Bytes::from_slice(&env, &[9]);
    let accepted = scores(&env, 1, 100);
    let accepted_journal = Bytes::from_slice(&env, b"accepted");
    contract_client.set_neuron_result(
        &layer0,
        &neuron0,
        &accepted,
        &guest,
        &accepted_journal,
        &seal,
    );

    verifier(&env, &contract_client).reject(&true);
    let rejected = scores(&env, 1, 200);
    let rejected_journal = Bytes::from_slice(&env, b"rejected");

    // A neuron with a result keeps it, and its proof
    assert_eq!(
        contract_client
            .try_set_neuron_result(
                &layer0,
                &neuron0,
                &rejected,
                &guest,
                &rejected_journal,
                &seal
            )
            .unwrap_err()
            .unwrap(),
        VotingSystemError::InvalidProof
    );
    assert_eq!(
        contract_client.get_neuron_result(&layer0, &neuron0),
        accepted
    );
    assert_eq!(
        contract_client
            .get_neuron_proof(&layer0, &neuron0)
            .journal_digest,
        digest(&env, &accepted_journal)
    );

    // A neuron without one stays without
    assert_eq!(
        contract_client
            .try_set_neuron_result(
                &layer0,
                &neuron1,
                &rejected,
                &guest,
                &rejected_journal,
                &seal
            )
            .unwrap_err()
            .unwrap(),
        VotingSystemError::InvalidProof
    );
    assert_eq!(
        contract_client
            .try_get_neuron_result(&layer0, &neuron1)
            .unwrap_err()
            .unwrap(),
        VotingSystemError::NeuronResultNotSet
    );
    assert_eq!(
        contract_client
            .try_get_neuron_proof(&layer0, &neuron1)
            .unwrap_err()
            .unwrap(),
        VotingSystemError::NeuronProofNotSet
    );
}

#[test]
fn verifier_that_cannot_answer_rejects_the_upload() {
    let env = Env::default();
    let (contract_client, _admin) = deploy_contract(&env);
    env.mock_all_auths();
    let layer0 = String::from_str(&env, "0");
    let neuron0 = String::from_str(&env, "0");
    contract_client.set_verifier(&Address::generate(&env));

    assert_eq!(
        contract_client
            .try_set_neuron_result(
                &layer0,
                &neuron0,
                &scores(&env, 1, 100),
                &NeuronGuest::TrustGraph,
                &Bytes::from_slice(&env, b"scores"),
                &Bytes::from_slice(&env, &[9])
            )
            .unwrap_err()
            .unwrap(),
        VotingSystemError::InvalidProof
    );
    assert_eq!(
        contract_client
            .try_get_neuron_result(&layer0, &neuron0)
            .unwrap_err()
            .unwrap(),
        VotingSystemError::NeuronResultNotSet
    );
}

#[test]
fn members_are_checked_before_the_proof() {
    let env = Env::default();
    let (contract_client, _admin) = deploy_contract(&env);
    env.mock_all_auths();

    assert_eq!(
        contract_client
            .try_set_neuron_result(
                &String::from_str(&env, "0"),
                &String::from_str(&env, "0"),
                &scores(&env, 42, 100),
                &NeuronGuest::PriorVotingHistory,
                &Bytes::from_slice(&env, b"scores"),
                &Bytes::from_slice(&env, &[9])
            )
            .unwrap_err()
            .unwrap(),
        VotingSystemError::NotAMember
    );
    assert_eq!(verifier(&env, &contract_client).seen(), None);
}

#[test]
fn proofs_follow_the_round() {
    let env = Env::default();
    let (contract_client, _admin) = deploy_contract(&env);
    env.mock_all_auths();
    let layer0 = String::from_str(&env, "0");
    let neuron0 = String::from_str(&env, "0");
    let guest = NeuronGuest::PriorVotingHistory;
    let seal = Bytes::from_slice(&env, &[9]);
    let journal_25 = Bytes::from_slice(&env, b"round 25");
    let journal_26 = Bytes::from_slice(&env, b"round 26");
    let journal_26_again = Bytes::from_slice(&env, b"round 26 again");

    contract_client.set_neuron_result(
        &layer0,
        &neuron0,
        &scores(&env, 1, 1),
        &guest,
        &journal_25,
        &seal,
    );

    // A new round has no proof until an upload
    contract_client.set_current_round(&26);
    assert_eq!(
        contract_client
            .try_get_neuron_proof(&layer0, &neuron0)
            .unwrap_err()
            .unwrap(),
        VotingSystemError::NeuronProofNotSet
    );

    contract_client.set_neuron_result(
        &layer0,
        &neuron0,
        &scores(&env, 1, 2),
        &guest,
        &journal_26,
        &seal,
    );
    assert_eq!(
        contract_client
            .get_neuron_proof(&layer0, &neuron0)
            .journal_digest,
        digest(&env, &journal_26)
    );

    // A later upload for the same round replaces the proof
    contract_client.set_neuron_result(
        &layer0,
        &neuron0,
        &scores(&env, 1, 3),
        &guest,
        &journal_26_again,
        &seal,
    );
    assert_eq!(
        contract_client
            .get_neuron_proof(&layer0, &neuron0)
            .journal_digest,
        digest(&env, &journal_26_again)
    );

    // The earlier round keeps its own
    assert_eq!(
        contract_client
            .get_neuron_proof_round(&layer0, &neuron0, &25)
            .journal_digest,
        digest(&env, &journal_25)
    );
}
