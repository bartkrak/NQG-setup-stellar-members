#![no_std]

use soroban_sdk::{
    contract, contractclient, contracterror, contractimpl, contracttype, Address, Bytes, BytesN,
    Env, Map, I256,
};

/// `verify` on the deployed Groth16 verifier.
///
/// The third argument is the SHA-256 of the journal bytes, despite the name
/// `journal` on that contract.
#[contractclient(name = "Groth16VerifierClient")]
#[allow(dead_code)]
trait Groth16Verifier {
    fn verify(env: Env, seal: Bytes, image_id: BytesN<32>, journal: BytesN<32>);
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    NotFound = 1,
}

/// Guest selected by `set_neuron_result`. The Image ID is fixed in this contract.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Neuron {
    PriorVotingHistory,
    AssignedReputation,
}

/// Journal bytes that passed `verify`, plus the proof checked against them.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Proof {
    pub journal: Bytes,
    pub seal: Bytes,
    pub image_id: BytesN<32>,
    pub journal_digest: BytesN<32>,
}

#[contracttype]
enum DataKey {
    Admin,
    Verifier,
    Result(Neuron, u32),
    Proof(Neuron, u32),
}

/// Image ID of the prior-voting-history guest (`host image-id`).
const PRIOR_VOTING_HISTORY_IMAGE_ID: [u8; 32] = [
    0xef, 0x57, 0x50, 0x76, 0xad, 0xca, 0x8e, 0xa1, 0x81, 0x18, 0xe8, 0xf5, 0x61, 0xd2, 0xc3, 0x9c,
    0x32, 0xaf, 0xb3, 0x28, 0x8d, 0x53, 0x21, 0xb9, 0x61, 0xca, 0xf5, 0xda, 0xdb, 0x28, 0x66, 0xdc,
];

/// Image ID of the assigned-reputation guest (`host image-id`).
const ASSIGNED_REPUTATION_IMAGE_ID: [u8; 32] = [
    0xd7, 0x5d, 0x18, 0x9d, 0xcd, 0x4b, 0x3b, 0xd0, 0x52, 0x8f, 0x8f, 0x47, 0xbe, 0x9a, 0x2f, 0x78,
    0xfe, 0x3f, 0xb1, 0x3a, 0x21, 0xef, 0x2f, 0x7f, 0x88, 0x65, 0x2f, 0xaf, 0x35, 0x21, 0xf2, 0x01,
];

impl Neuron {
    fn image_id(self) -> &'static [u8; 32] {
        match self {
            Neuron::PriorVotingHistory => &PRIOR_VOTING_HISTORY_IMAGE_ID,
            Neuron::AssignedReputation => &ASSIGNED_REPUTATION_IMAGE_ID,
        }
    }
}

/// Readable neuron scores, stored only after the Groth16 verifier accepts the journal.
/// Membership token id to a score scaled by 10^18.
#[contract]
pub struct NeuronResult;

#[contractimpl]
impl NeuronResult {
    /// `verifier` is the already deployed Groth16 verifier contract.
    pub fn __constructor(env: Env, admin: Address, verifier: Address) {
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Verifier, &verifier);
    }

    /// Check `journal` and `seal`, then store the scores and the proof under
    /// `neuron` and `round`.
    ///
    /// Only `admin` can call it.
    /// The digest is not an argument: it is SHA-256 of `journal`. A later call
    /// that also verifies replaces this neuron's result for the same round.
    pub fn set_neuron_result(
        env: Env,
        neuron: Neuron,
        round: u32,
        result: Map<u32, I256>,
        journal: Bytes,
        seal: Bytes,
    ) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();

        let image_id = BytesN::from_array(&env, neuron.image_id());
        let journal_digest: BytesN<32> = env.crypto().sha256(&journal).into();
        let verifier: Address = env.storage().instance().get(&DataKey::Verifier).unwrap();
        Groth16VerifierClient::new(&env, &verifier).verify(&seal, &image_id, &journal_digest);

        env.storage()
            .persistent()
            .set(&DataKey::Result(neuron, round), &result);
        env.storage().persistent().set(
            &DataKey::Proof(neuron, round),
            &Proof {
                journal,
                seal,
                image_id,
                journal_digest,
            },
        );
    }

    pub fn get_neuron_result(
        env: Env,
        neuron: Neuron,
        round: u32,
    ) -> Result<Map<u32, I256>, Error> {
        env.storage()
            .persistent()
            .get(&DataKey::Result(neuron, round))
            .ok_or(Error::NotFound)
    }

    /// Journal, seal, image id, and digest stored by a successful `set_neuron_result`.
    pub fn get_proof(env: Env, neuron: Neuron, round: u32) -> Result<Proof, Error> {
        env.storage()
            .persistent()
            .get(&DataKey::Proof(neuron, round))
            .ok_or(Error::NotFound)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{
        contract, contractimpl, symbol_short, testutils::Address as _, Address, Env,
    };

    #[contract]
    struct MockVerifier;

    #[contractimpl]
    impl MockVerifier {
        pub fn __constructor(env: Env, reject: bool) {
            env.storage()
                .instance()
                .set(&symbol_short!("reject"), &reject);
        }

        pub fn verify(env: Env, seal: Bytes, image_id: BytesN<32>, journal: BytesN<32>) {
            let reject: bool = env
                .storage()
                .instance()
                .get(&symbol_short!("reject"))
                .unwrap();
            if reject {
                panic!("verifier rejected the proof");
            }
            env.storage().instance().set(&symbol_short!("seal"), &seal);
            env.storage()
                .instance()
                .set(&symbol_short!("image"), &image_id);
            env.storage()
                .instance()
                .set(&symbol_short!("digest"), &journal);
        }

        pub fn seen(env: Env) -> (Bytes, BytesN<32>, BytesN<32>) {
            (
                env.storage()
                    .instance()
                    .get(&symbol_short!("seal"))
                    .unwrap(),
                env.storage()
                    .instance()
                    .get(&symbol_short!("image"))
                    .unwrap(),
                env.storage()
                    .instance()
                    .get(&symbol_short!("digest"))
                    .unwrap(),
            )
        }
    }

    fn setup(reject: bool) -> (Env, Address, NeuronResultClient<'static>) {
        let env = Env::default();
        let admin = Address::generate(&env);
        let verifier_id = env.register(MockVerifier, (reject,));
        let contract_id = env.register(NeuronResult, (admin, verifier_id.clone()));
        let client = NeuronResultClient::new(&env, &contract_id);
        (env, verifier_id, client)
    }

    fn scores(env: &Env, token: u32, value: i128) -> Map<u32, I256> {
        let mut result = Map::new(env);
        result.set(token, I256::from_i128(env, value));
        result
    }

    #[test]
    fn stores_scores_and_proof_only_after_verify_sees_the_hash() {
        let (env, verifier_id, client) = setup(false);
        env.mock_all_auths();
        let neuron = Neuron::AssignedReputation;
        let result = scores(&env, 12, 1_500_000_000_000_000_000);
        let journal = Bytes::from_slice(&env, b"scores");
        let seal = Bytes::from_slice(&env, &[0x73, 0xc4, 0x57, 0xba, 0x01]);
        let expected_digest: BytesN<32> = env.crypto().sha256(&journal).into();

        client.set_neuron_result(&neuron, &33, &result, &journal, &seal);

        assert_eq!(client.get_neuron_result(&neuron, &33), result);
        let stored = client.get_proof(&neuron, &33);
        assert_eq!(stored.journal, journal);
        assert_eq!(stored.seal, seal);
        assert_eq!(
            stored.image_id,
            BytesN::from_array(&env, &ASSIGNED_REPUTATION_IMAGE_ID)
        );
        assert_eq!(stored.journal_digest, expected_digest);
        assert_eq!(
            MockVerifierClient::new(&env, &verifier_id).seen(),
            (seal, stored.image_id, expected_digest)
        );
    }

    #[test]
    fn each_neuron_keeps_its_own_scores_and_proof() {
        let (env, verifier_id, client) = setup(false);
        env.mock_all_auths();
        let prior_scores = scores(&env, 7, 2_500_000_000_000_000_000);
        let assigned_scores = scores(&env, 12, 1_500_000_000_000_000_000);
        let prior = Bytes::from_slice(&env, b"prior");
        let assigned = Bytes::from_slice(&env, b"assigned");
        let seal = Bytes::from_slice(&env, &[9]);

        client.set_neuron_result(
            &Neuron::PriorVotingHistory,
            &33,
            &prior_scores,
            &prior,
            &seal,
        );
        assert_eq!(
            MockVerifierClient::new(&env, &verifier_id).seen().1,
            BytesN::from_array(&env, &PRIOR_VOTING_HISTORY_IMAGE_ID)
        );
        client.set_neuron_result(
            &Neuron::AssignedReputation,
            &33,
            &assigned_scores,
            &assigned,
            &seal,
        );

        assert_eq!(
            client.get_neuron_result(&Neuron::PriorVotingHistory, &33),
            prior_scores
        );
        assert_eq!(
            client.get_neuron_result(&Neuron::AssignedReputation, &33),
            assigned_scores
        );
        assert_eq!(
            client.get_proof(&Neuron::PriorVotingHistory, &33).journal,
            prior
        );
        assert_eq!(
            client.get_proof(&Neuron::AssignedReputation, &33).journal,
            assigned
        );
    }

    #[test]
    fn each_round_keeps_its_own_scores_and_proof() {
        let (env, _, client) = setup(false);
        env.mock_all_auths();
        let neuron = Neuron::PriorVotingHistory;
        let round_33 = scores(&env, 7, 1);
        let round_34 = scores(&env, 7, 2);
        let journal_33 = Bytes::from_slice(&env, b"a");
        let journal_34 = Bytes::from_slice(&env, b"b");
        let seal = Bytes::from_slice(&env, &[9]);

        client.set_neuron_result(&neuron, &33, &round_33, &journal_33, &seal);
        client.set_neuron_result(&neuron, &34, &round_34, &journal_34, &seal);

        assert_eq!(client.get_neuron_result(&neuron, &33), round_33);
        assert_eq!(client.get_neuron_result(&neuron, &34), round_34);
        assert_eq!(client.get_proof(&neuron, &33).journal, journal_33);
        assert_eq!(client.get_proof(&neuron, &34).journal, journal_34);
        let Err(Ok(Error::NotFound)) = client.try_get_neuron_result(&neuron, &35) else {
            panic!("expected NotFound");
        };
    }

    #[test]
    fn same_round_is_replaced_by_a_later_verified_call() {
        let (env, _, client) = setup(false);
        env.mock_all_auths();
        let neuron = Neuron::PriorVotingHistory;
        let first = scores(&env, 7, 1);
        let second = scores(&env, 7, 2);
        let seal = Bytes::from_slice(&env, &[9]);

        client.set_neuron_result(&neuron, &33, &first, &Bytes::from_slice(&env, b"a"), &seal);
        client.set_neuron_result(&neuron, &33, &second, &Bytes::from_slice(&env, b"b"), &seal);

        assert_eq!(client.get_neuron_result(&neuron, &33), second);
    }

    #[test]
    fn failed_verify_stores_nothing() {
        let (env, _, client) = setup(true);
        env.mock_all_auths();
        let neuron = Neuron::PriorVotingHistory;
        let result = scores(&env, 7, 1);
        let journal = Bytes::from_slice(&env, b"scores");
        let seal = Bytes::from_slice(&env, &[1, 2, 3]);

        assert!(client
            .try_set_neuron_result(&neuron, &33, &result, &journal, &seal)
            .is_err());
        let Err(Ok(Error::NotFound)) = client.try_get_neuron_result(&neuron, &33) else {
            panic!("expected NotFound");
        };
        let Err(Ok(Error::NotFound)) = client.try_get_proof(&neuron, &33) else {
            panic!("expected NotFound");
        };
    }

    #[test]
    fn set_without_admin_auth_stores_nothing() {
        let (env, _, client) = setup(false);
        let neuron = Neuron::PriorVotingHistory;
        let result = scores(&env, 7, 1);
        let journal = Bytes::from_slice(&env, b"scores");
        let seal = Bytes::from_slice(&env, &[1]);

        assert!(client
            .try_set_neuron_result(&neuron, &33, &result, &journal, &seal)
            .is_err());
        let Err(Ok(Error::NotFound)) = client.try_get_neuron_result(&neuron, &33) else {
            panic!("expected NotFound");
        };
    }
}
