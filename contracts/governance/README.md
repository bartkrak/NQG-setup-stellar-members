# NQG

This contract is a part of implementation of
the [Neural Quorum Governance](https://stellarcommunityfund.gitbook.io/module-library) mechanism.

![architecture](image.png)

Neuron scores are computed off-chain and submitted to this contract. For the prover, proof format and scripts,
see the [main README](../../README.md).

The contract adds up results of each layer and computes the final voting power (NQG score) for each voter. This voting
power is stored on-chain per round for future reference.

Voters are identified by their Stellar Membership token id
(`u32`), which stays the same across key rotations and recoveries. Every uploaded neuron result is checked against the
membership contract, and ids that are not active members are rejected.

The contract only computes NQG. It does not count votes: other contracts and apps read the scores through
`get_voting_power_for_user(member_id)` or `get_voting_powers()`.

## Interface

Voter ids are plain `u32` Stellar Membership token ids. Neuron results, neuron weights and voting powers are `i64`
fixed point numbers with 6 decimals: `1_000_000` is 1.0. Weighting and `Product` layers multiply in `i128` before dividing, so the full range is
usable. A result that does not fit an `i64` fails with `ArithmeticOverflow` instead of wrapping. The stellar CLI takes
and prints `i64` values as JSON numbers, for example `1000000`.

| Function | Who | What it does |
|---|---|---|
| `__constructor(admin, current_round, membership_contract, verifier)` | deployer | Sets the admin, the active round, the Stellar Membership contract and the Groth16 verifier contract. |
| `add_layer(raw_neurons, layer_aggregator)` | admin | Adds a layer of neurons (`[(name, weight: i64)]`, aggregated by `Sum` or `Product`). |
| `remove_layer(layer_id)` | admin | Removes a layer and its neurons. |
| `set_neuron_result(layer_id, neuron_id, result: Map<u32, i64>, guest, journal, seal)` | admin | Stores one neuron's results for the active round after admin, membership and verifier checks. Replaces an existing result for the same layer, neuron and round. |
| `calculate_voting_powers()` | admin | Runs all layers over the active round's neuron results and stores the voting powers for that round. |
| `get_voting_power_for_user(member_id: u32) -> i64` | anyone | Voting power (NQG score) of one member for the active round. |
| `get_voting_powers() -> Map<u32, i64>` | anyone | Voting powers of all members for the active round. |
| `set_current_round(round)` / `get_current_round()` | admin / anyone | Switches the active round. Data of other rounds is kept. |
| `set_membership_contract(address)` / `get_membership_contract()` | admin / anyone | Points the contract at another Stellar Membership contract. |
| `set_verifier(address)` / `get_verifier()` | admin / anyone | Points the contract at another Groth16 verifier contract. |
| `set_trust_key(key: BytesN<32>)` / `get_trust_key()` | admin / anyone | The operator's X25519 public key, which every trust list is also encrypted to. |
| `set_trust_list(member_id: u32, trust_list: TrustList)` | the member | Stores the member's encrypted trust list, replacing the previous one. See [Trust lists](#trust-lists). |
| `get_trust_list(member_id: u32) -> Option<TrustList>` | anyone | The member's encrypted trust list, if one was stored. |
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
| 18 | `NotAMember` | `set_neuron_result` with an id that is not an active member; `set_trust_list` for a token never minted or revoked. |
| 19 | `ArithmeticOverflow` | `calculate_voting_powers` or `get_layer_result` when a value does not fit an `i64`. |
| 20 | `InvalidProof` | `set_neuron_result` when the verifier rejects the proof or cannot answer. |
| 21 | `NeuronProofNotSet` | `get_neuron_proof[_round]` when no proof was stored for that neuron and round. |
| 22 | `TrustKeyNotSet` | `get_trust_key` or `set_trust_list` before the admin set the trust key. |
| 23 | `TrustListTooLarge` | `set_trust_list` with a `list` over `MAX_TRUST_LIST_SIZE` (512) bytes. |
| 24 | `TrustKeyMismatch` | `set_trust_list` with an operator copy not sealed for the current trust key. |

## Trust lists

Each member can store one trust list, under their token id in persistent storage, written with the maximum lifetime
the network allows. It is encrypted off-chain with a data key, sealed for the member and for the operator key set with
`set_trust_key`:

```rust
TrustList { list: Bytes, member: KeyWrap, operator: KeyWrap }
KeyWrap { key: BytesN<32>, ephemeral: BytesN<32>, sealed: BytesN<48> }
```

`list` (version, nonce, ciphertext) is opaque to the contract. A `KeyWrap` is one recipient's copy of the data key:
the recipient's X25519 public key, the one-time key it was sealed with, and the sealed key. `set_trust_list` requires
`operator.key` to equal the current trust key (`TrustKeyMismatch` otherwise). It cannot check the sealed copies
themselves: a list that does not open is stored, and `collect.ts` reports it. After a new `set_trust_key`, every
stored list is still sealed for the old key and stays unreadable for `collect.ts` until its member saves again;
replacing the operator copies is not implemented. The two copies are separate so that one can later be replaced
without touching the list. The encryption, the member's key derived
from a wallet signature and the operator side are described in the [root README](../../README.md#trust-lists).

`set_trust_list` asks the membership contract for `member(member_id)`, which must not be revoked, and for
`owner_of(member_id)`, whose signature it requires. A token never minted or revoked fails with `NotAMember`. Because
the list is keyed by token id, it stays after a key move; the new key replaces it, the old one can no longer write
it.

## Connecting to Stellar Membership

The two contracts point at each other:

- **NQG reads the membership contract** to validate voters. `set_neuron_result` calls `member(token_id)` for every
  key of the uploaded map and requires `revoked` to be `false`. A revoked token, a token never minted
  (`NonExistentToken`), a record without a `revoked` flag, or a contract that cannot answer rejects the whole upload
  with `NotAMember`, and nothing is written. Only `revoked` is read from the record, so changes to its other fields do
  not affect NQG.
  `owner_of` is not used for that: it succeeds for revoked tokens. `set_trust_list` reads it after `member`, to require
  the signature of the token's current address.
- **The membership contract reads NQG** to show a member's score (`governance(token_id)`, `trait_value(token_id,
  "nqg")`). Its `get_nqg` calls `get_voting_power_for_user(token_id: u32) -> i64`.

### Deploying the pair

The Groth16 verifier has to be deployed first (or an existing deployment chosen), since NQG takes its address in
`--verifier`. Then, between NQG and the membership contract, each constructor takes the other contract's address, so
one has to go first. The membership contract tolerates a wrong
NQG address (the score reads 0 and `set_nqg_contract` fixes it), while NQG must know the real membership contract to
validate anything.
