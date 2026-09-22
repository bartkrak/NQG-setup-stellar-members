use governance::{VotingSystem, VotingSystemClient};
use soroban_sdk::testutils::Address as AddressTrait;
use soroban_sdk::{Address, Env};

use crate::e2e::common::membership::{MockMembership, MockMembershipClient};

/// Members minted on the membership mock before the governance contract is
/// deployed, as token ids 0 to `MEMBERS - 1`. Tests upload data for ids in
/// that range unless they mean to be rejected.
pub const MEMBERS: u32 = 4;

/// Deploy the governance contract for round 25, with a fresh membership mock
/// holding `MEMBERS` members as its Stellar Membership contract.
pub fn deploy_contract(env: &Env) -> (VotingSystemClient<'_>, Address) {
    let admin = Address::generate(env);
    let membership = env.register(MockMembership, ());
    let members = MockMembershipClient::new(env, &membership);
    for _ in 0..MEMBERS {
        members.mint(&Address::generate(env));
    }
    let contract_id = env.register(VotingSystem, (admin.clone(), 25u32, membership));
    let contract_client = VotingSystemClient::new(env, &contract_id);

    (contract_client, admin)
}
