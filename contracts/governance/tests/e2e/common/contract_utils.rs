use governance::{NeuronGuest, VotingSystem, VotingSystemClient};
use soroban_sdk::testutils::Address as AddressTrait;
use soroban_sdk::{Address, Env, Map, String, TryFromVal};

use crate::e2e::common::verifier::{MockVerifier, proof};

/// Deploy governance for round 25 with a verifier mock accepting every proof.
pub fn deploy_contract(env: &Env) -> (VotingSystemClient<'_>, Address) {
    let admin = Address::generate(env);
    let verifier = env.register(MockVerifier, ());
    let contract_id = env.register(VotingSystem, (admin.clone(), 25u32, verifier));
    let contract_client = VotingSystemClient::new(env, &contract_id);

    (contract_client, admin)
}

/// Upload `result` for the active round with a proof the verifier mock
/// accepts, for tests that are not about the proof.
pub fn upload_neuron_result(
    contract_client: &VotingSystemClient,
    layer_id: &String,
    neuron_id: &String,
    result: &Map<Address, i64>,
) {
    let (journal, seal) = proof(&contract_client.env);
    contract_client.set_neuron_result(
        layer_id,
        neuron_id,
        result,
        &NeuronGuest::PriorVotingHistory,
        &journal,
        &seal,
    );
}

/// Deterministic Stellar account address, with no membership registration.
pub fn public_key(env: &Env, index: u8) -> Address {
    use soroban_sdk::xdr::{AccountId, PublicKey, ScAddress, Uint256};
    Address::try_from_val(
        env,
        &ScAddress::Account(AccountId(PublicKey::PublicKeyTypeEd25519(Uint256(
            [index; 32],
        )))),
    )
    .unwrap()
}
