# NQG

This contract is a part of implementation of
the [Neural Quorum Governance](https://stellarcommunityfund.gitbook.io/module-library) mechanism.

![architecture](image.png)

Currently, because
of [resource constraints](https://developers.stellar.org/docs/reference/resource-limits-fees#resource-limits) and to
preserve voter privacy, neurons are computed off-chain and uploaded to the contract. Each neuron is computed by a RISC
Zero guest program (`zkvm/` in this repository), and every upload carries its Groth16 proof, which the contract checks
with a verifier contract before storing anything. See [Proofs](#proofs).

The contract adds up results of each layer and computes the final voting power (NQG score) for each voter. This voting
power is stored on-chain per round for future reference.

Voters are identified by their Stellar Membership token id
(`u32`), which stays the same across key rotations and recoveries. Every uploaded neuron result is checked against the
membership contract, and ids that are not active members are rejected.

The contract only computes NQG. It does not count votes: other contracts and apps read the scores through
`get_voting_power_for_user(member_id)` or `get_voting_powers()`.

## Interface

Voter ids are plain `u32` Stellar Membership token ids. Neuron results, neuron weights and voting powers are `i64`
fixed point numbers with 6 decimals: `1_000_000` is 1.0, and the largest value is about 9.2 trillion
(9,223,372,036,854.775807). Weighting and `Product` layers multiply in `i128` before dividing, so the full range is
usable. A result that does not fit an `i64` fails with `ArithmeticOverflow` instead of wrapping. The stellar CLI takes
and prints `i64` values as JSON numbers, for example `1000000`.

| Function | Who | What it does |
|---|---|---|
| `__constructor(admin, current_round, membership_contract, verifier)` | deployer | Sets the admin, the active round, the Stellar Membership contract and the Groth16 verifier contract. |
| `add_layer(raw_neurons, layer_aggregator)` | admin | Adds a layer of neurons (`[(name, weight: i64)]`, aggregated by `Sum` or `Product`). |
| `remove_layer(layer_id)` | admin | Removes a layer and its neurons. |
| `set_neuron_result(layer_id, neuron_id, result: Map<u32, i64>, guest, journal, seal)` | admin | Stores one neuron's results and their proof for the active round. Every key must be an active member, see below, and the verifier must accept the proof, see [Proofs](#proofs). |
| `calculate_voting_powers()` | admin | Runs all layers over the active round's neuron results and stores the voting powers for that round. |
| `get_voting_power_for_user(member_id: u32) -> i64` | anyone | Voting power (NQG score) of one member for the active round. |
| `get_voting_powers() -> Map<u32, i64>` | anyone | Voting powers of all members for the active round. |
| `set_current_round(round)` / `get_current_round()` | admin / anyone | Switches the active round. Data of other rounds is kept. |
| `set_membership_contract(address)` / `get_membership_contract()` | admin / anyone | Points the contract at another Stellar Membership contract. |
| `set_verifier(address)` / `get_verifier()` | admin / anyone | Points the contract at another Groth16 verifier contract. |
| `transfer_admin(new_admin)`, `upgrade(wasm_hash)` | admin | Administration. |
| `get_layer`, `get_neuron`, `get_neuron_result[_round]`, `get_neuron_proof[_round]`, `get_layer_result`, `get_neural_governance` | anyone | Inspect the setup, intermediate results and the proofs they were uploaded with. |

### Errors

| Code | Name | When |
|---|---|---|
| 0 | `UnknownError` | `get_voting_power_for_user` before voting powers were calculated for the active round. |
| 7 | `NeuronResultNotSet` | `calculate_voting_powers` when a neuron of a layer has no result for the active round. |
| 9, 10 | `LayerMissing`, `NeuronMissing` | Unknown layer or neuron id. |
| 11 | `NGQResultForVoterMissing` | `get_voting_power_for_user` for a member without a score this round. |
| 15 | `VotingPowersNotSet` | `get_voting_powers` before voting powers were calculated for the active round. |
| 17 | `LayerResultsUsersMismatch` | `calculate_voting_powers` when layers do not cover the same members. |
| 18 | `NotAMember` | `set_neuron_result` with an id that is not an active member. |
| 19 | `ArithmeticOverflow` | `calculate_voting_powers` or `get_layer_result` when a value does not fit an `i64`. |
| 20 | `InvalidProof` | `set_neuron_result` when the verifier rejects the proof or cannot answer. |
| 21 | `NeuronProofNotSet` | `get_neuron_proof[_round]` when no proof was stored for that neuron and round. |

## Proofs

`set_neuron_result` takes, next to the result, the proof from `host prove` (`zkvm/`):

- `guest`: the guest program that computed the result, `PriorVotingHistory`, `AssignedReputation` or `TrustGraph`.
  Each has its Image ID fixed as a constant in [`src/verifier.rs`](src/verifier.rs).
- `journal`: the guest's public output (`journal` in the prover output JSON, hex on the CLI).
- `seal`: the 4-byte verifier selector followed by the Groth16 seal (`seal` in the output JSON).

The contract computes `sha256(journal)` itself and calls `verify(seal, image_id, journal_digest)` on the
[RISC Zero Groth16 verifier](https://github.com/NethermindEth/stellar-risc0-verifier) set by the constructor (its
third argument is the digest, despite being named `journal`). The order is: admin signature, membership of every key,
proof. If any of them fails nothing is written. Otherwise the result is stored as before and
`NeuronProof { seal, image_id, journal_digest }` is stored next to it in persistent storage, under the same layer,
neuron and round. A later upload for the same neuron and round replaces both.

The journal itself is not stored, only its digest; it stays readable in the `set_neuron_result` transaction.

A changed guest, core crate, dependency or builder image can change an Image ID. Compare the constants with
`host image-id --neuron <n>` before deploying, and `upgrade` the contract when they change: with a stale Image ID every
upload for that guest fails with `InvalidProof`.

## Connecting to Stellar Membership

The two contracts point at each other:

- **NQG reads the membership contract** to validate voters. `set_neuron_result` calls `member(token_id)` for every
  key of the uploaded map and requires `status` to be `0` (Active). A revoked token (`status` `1`), a token never
  minted (`NonExistentToken`), or a contract that cannot answer rejects the whole upload with `NotAMember`, and
  nothing is written. Only `status` is read from the record, so changes to its other fields do not affect NQG.
  `owner_of` is not used: it succeeds for revoked tokens.
- **The membership contract reads NQG** to show a member's score (`governance(token_id)`, `trait_value(token_id,
  "nqg")`). Its `get_nqg` calls `get_voting_power_for_user(token_id: u32) -> i64`. Both contracts use 6 decimals, so the
  value only needs widening to the membership contract's `i128`. Any failure, including a member without a score this round, reads as 0.

Neither call re-enters the other contract, so there is no reentrancy at runtime.

Because the score is keyed by token id, it survives `rotate_key` and `recover`. The membership contract may still
show 0 for a while after a key moves (its voting power hold, `held_until` in `governance`); that is its own rule, NQG
keeps returning the score.

### Change needed in the membership contract

Until September 2026 NQG returned an `I256` with 18 decimals, and `get_nqg` still decodes that and divides by
`NQG_SCALE` (10^12). Against the `i64` NQG the decode fails and every score reads 0. It has to decode an `i64`
instead, with no scaling:

```rust
fn get_nqg(e: &Env, token_id: u32) -> i128 {
    let r = e.try_invoke_contract::<i64, InvokeError>(
        &StellarMembership::nqg_contract(e),
        &Symbol::new(e, "get_voting_power_for_user"),
        vec![e, token_id.into_val(e)],
    );
    match r {
        Ok(Ok(nqg)) => i128::from(nqg).max(0),
        _ => 0,
    }
}
```

`NQG_SCALE` and the `I256` import in `governance.rs` become unused. The test NQG contract (`contracts/test-nqg`) and
the `nqg` test mock should return `i64` as well.

### Deploying the pair

The Groth16 verifier has to be deployed first (or an existing deployment chosen), since NQG takes its address in
`--verifier`. Then, between NQG and the membership contract, each constructor takes the other contract's address, so
one has to go first. The membership contract tolerates a wrong
NQG address (the score reads 0 and `set_nqg_contract` fixes it), while NQG must know the real membership contract to
validate anything. So:

1. Deploy the membership contract with any placeholder `--nqg_contract` (for example the admin's address).
2. Deploy NQG with the membership contract address (`--membership_contract`) and the verifier (`--verifier`).
3. Call `set_nqg_contract` on the membership contract with the NQG address.

If the membership contract is later redeployed to a new address, point NQG at it with `set_membership_contract`
instead of redeploying NQG.

### Tutorial: both contracts on a local network

This runs the whole flow on a local network in Docker: members are minted on the membership contract, neuron results
are uploaded for them, and the membership contract returns their NQG scores. It uses the scripts from
[`examples`](../../examples/README.md). You need the stellar CLI, Docker and `jq`.

The stellar CLI loads a `.env` from the current folder or any parent, and a `STELLAR_RPC_URL` there overrides
`--network local`. If you keep one (for example for testnet), check with `stellar env` before starting, or the
commands below may go to that network instead.

**1. Start the network and create identities.**

```bash
stellar container start local
for k in nqg-admin mem-admin m0 m1 m2 m3 m4; do stellar keys generate $k --network local --fund; done
```

If a later step fails with `Account not found`, friendbot was not ready yet when the keys were generated: fund them
again with `stellar keys fund <name> --network local`.

**2. Deploy the membership contract with a placeholder NQG address**, from the membership repository after
`stellar contract build`. `mint` needs the signature of both the new member and an operator, and `stellar contract
invoke` only signs for the source account, so for this test the member keys are also the operators and each one mints
its own token:

```bash
MEMBERSHIP=$(stellar contract deploy --network local --source-account mem-admin \
  --wasm target/wasm32v1-none/release/stellar_membership.wasm \
  -- --admin mem-admin \
  --operators "[\"$(stellar keys address m0)\",\"$(stellar keys address m1)\",\"$(stellar keys address m2)\",\"$(stellar keys address m3)\",\"$(stellar keys address m4)\"]" \
  --name "Stellar Members" --symbol SMBR --uri https://example.com --uri_trait https://example.com \
  --nqg_contract mem-admin)

for i in 0 1 2 3 4; do
  stellar contract invoke --network local --source-account m$i --id $MEMBERSHIP -- mint \
    --to m$i --operator m$i --role 0 \
    --external_accounts '{"accounts":[],"email_hash":null}' --bio="test$i" --projects '[]'
done
```

The mints return token ids `0` to `4`, the ids used in `examples/data/voters.json`.

**3. Configure the examples.** In `examples/`, copy `env.example` to `.env` and set:

```bash
STELLAR_PUBLIC_KEY=<output of: stellar keys address nqg-admin>
STELLAR_SECRET_KEY=<output of: stellar keys secret nqg-admin>
STELLAR_RPC_URL="http://localhost:8000/rpc"
STELLAR_NETWORK_PASSPHRASE="Standalone Network ; February 2017"
STELLAR_NETWORK="local"
MEMBERSHIP_CONTRACT_ADDRESS=<the $MEMBERSHIP address from step 2>
```

**4. Deploy NQG and compute scores**, from `examples/`:

```bash
./scripts/membership_check_voters.sh              # every id 0..4 must print "active"
./scripts/governance_deploy.sh                    # deploys NQG, sets up layers, saves the address in .env
./scripts/governance_upload_neurons_results.sh
./scripts/governance_calculate_voting_powers.sh
./scripts/governance_get_voting_powers.sh
```

The last script should print `113000000` (113.0) for member `0`.

**5. Connect the membership contract to NQG and read a score through it.** This needs the `get_nqg` change above;
without it `nqg` reads `"0"`.

```bash
source .env
stellar contract invoke --network local --source-account mem-admin --id $MEMBERSHIP_CONTRACT_ADDRESS \
  -- set_nqg_contract --nqg_contract $NEURAL_GOVERNANCE_ADDRESS
stellar contract invoke --network local --source-account mem-admin --id $MEMBERSHIP_CONTRACT_ADDRESS \
  --send=no -- governance --token_id 0
```

The result should be `{"held_until":0,"nqg":"113000000","role":0}`: 113.0 at 6 decimals.

**6. Check that validation follows membership.** Revoke a member and upload again:

```bash
stellar contract invoke --network local --source-account m2 --id $MEMBERSHIP_CONTRACT_ADDRESS \
  -- revoke --operator m2 --token_id 2 --revoked
./scripts/membership_check_voters.sh               # 2 is reported as revoked
./scripts/governance_upload_neurons_results.sh     # each upload fails with Error(Contract, #18), NotAMember
```

`--revoked` is a bool flag: without it the call reinstates the token, which for an active member changes nothing.

Clean up with `stellar container stop local`.

`examples/scripts/e2e_testnet.sh` runs this whole flow on testnet, or locally, with a pass or fail for each check.

### Things to know

- **Only the active round is served.** `get_voting_power_for_user` and `get_voting_powers` read the round set by
  `set_current_round`. Moving to a new round makes scores read as missing until `calculate_voting_powers` runs for it.
- **Validation happens at upload time.** A member revoked after their results were uploaded keeps their score for that
  round in NQG. The membership contract shows 0 for a revoked member regardless.
- **The proof does not cover `result`.** The verifier checks that the guest committed the journal, but the contract
  does not decode the journal (risc0 serde of string ids and `f64` scores) and does not compare it, or its round, with
  `result` and the active round. Anyone can check them off-chain against the journal in the transaction. Nothing ties
  a guest to a neuron either: `guest` is chosen per upload.
- **One membership read per voter.** Each uploaded id costs one `member` call, and a transaction has a ledger read
  limit, so a single `set_neuron_result` covers a limited number of members. Fine for small rounds; large rounds will
  need a different approach.
- **Storage lifetimes are not extended.** Neuron results live in temporary storage and expire if
  `calculate_voting_powers` runs too late after the upload. Voting powers and the contract instance are not extended
  either, so on a long running network they eventually need their TTL extended or restored.

## Release 

Governance releases are built by the
[`governance-release.yml`](../../.github/workflows/governance-release.yml) GitHub Actions workflow. Pushing a Git tag
starting with `v` triggers the workflow. It builds and optimizes the WASM, publishes it in a GitHub Release, and
submits the WASM hash and source commit for StellarExpert validation.

The version in [`Cargo.toml`](Cargo.toml) should match the Git tag without the `v` prefix. For example:

```text
Cargo.toml: version = "1.0.3"
Git tag:    v1.0.3
WASM:       governance_v1.0.3.wasm
```

If the versions do not match, the build still works, but the generated release tag includes both versions, for
example `v1.0.3_contracts_governance_pkg0.1.0_cli27.0.0`.

### Release flow

1. Update the contract version in `contracts/governance/Cargo.toml` and commit `Cargo.lock` if it changes.
2. Make sure all contract changes are committed and merged into `main`.
3. Create a tag matching the version from `Cargo.toml`:

   ```bash
   git tag v1.0.3
   ```

4. Push the tag:

   ```bash
   git push origin v1.0.3
   ```

5. GitHub Actions builds the optimized WASM, creates the GitHub Release, and validates the source and WASM hash.
   After a successful release, the workflow removes the short trigger tag and keeps the generated release tag.
