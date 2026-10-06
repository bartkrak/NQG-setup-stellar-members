use governance::{NeuronGuest, VotingSystem, VotingSystemClient};
use soroban_sdk::testutils::Address as AddressTrait;
use soroban_sdk::{Address, Env, Map, String};

use crate::e2e::common::membership::{MockMembership, MockMembershipClient};
use crate::e2e::common::verifier::{MockVerifier, proof};

/// Members minted on the membership mock before the governance contract is
/// deployed, as token ids 0 to `MEMBERS - 1`. Tests upload data for ids in
/// that range unless they mean to be rejected.
pub const MEMBERS: u32 = 4;

/// Deploy the governance contract for round 25, with a fresh membership mock
/// holding `MEMBERS` members as its Stellar Membership contract and a fresh
/// verifier mock that accepts every proof.
pub fn deploy_contract(env: &Env) -> (VotingSystemClient<'_>, Address) {
    let admin = Address::generate(env);
    let membership = env.register(MockMembership, ());
    let members = MockMembershipClient::new(env, &membership);
    for _ in 0..MEMBERS {
        members.mint(&Address::generate(env));
    }
    let verifier = env.register(MockVerifier, ());
    let contract_id = env.register(VotingSystem, (admin.clone(), 25u32, membership, verifier));
    let contract_client = VotingSystemClient::new(env, &contract_id);

    (contract_client, admin)
}

/// Upload `result` for the active round with a proof the verifier mock
/// accepts, for tests that are not about the proof.
pub fn upload_neuron_result(
    contract_client: &VotingSystemClient,
    layer_id: &String,
    neuron_id: &String,
    result: &Map<u32, i64>,
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
