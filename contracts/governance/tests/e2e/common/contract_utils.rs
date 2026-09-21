use governance::{VotingSystem, VotingSystemClient};
use soroban_sdk::testutils::Address as AddressTrait;
use soroban_sdk::{Address, Env};

use crate::e2e::common::membership::MockMembership;

/// Deploy the governance contract for round 25, with a fresh membership mock
/// as its Stellar Membership contract.
pub fn deploy_contract(env: &Env) -> (VotingSystemClient<'_>, Address) {
    let admin = Address::generate(env);
    let membership = env.register(MockMembership, ());
    let contract_id = env.register(VotingSystem, (admin.clone(), 25u32, membership));
    let contract_client = VotingSystemClient::new(env, &contract_id);

    (contract_client, admin)
}
