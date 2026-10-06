# Neuron scores proven with RISC Zero and stored on Stellar

This repository computes neuron scores for Stellar Community Fund members,
proves with [RISC Zero](https://risczero.com/) that the scores were computed
correctly, and stores them on Stellar together with the proof, in the Neural
Quorum Governance (NQG) contract that turns them into voting powers. The
private input never leaves the machine that generates the proof. Only the
scores and the proof are published.

The system has three parts:

| Part | Path | What it does |
| --- | --- | --- |
| Prover | [`zkvm/`](zkvm) | Runs a neuron inside the RISC Zero zkVM and produces a Groth16 proof plus one output JSON file |
| Submit script | [`scripts/submit-neuron.ts`](scripts/submit-neuron.ts) | Reads the output JSON and sends the scores and the proof to the contract |
| Contract | [`contracts/governance/`](contracts/governance) | The NQG contract. Checks the proof with the Groth16 verifier, stores the scores per layer, neuron and round, and computes the voting powers |

The proof is checked on chain by the Groth16 verifier contract from
[NethermindEth/stellar-risc0-verifier](https://github.com/NethermindEth/stellar-risc0-verifier).
This repository does not contain that contract. It is deployed once and the
NQG contract calls it.

The NQG contract also requires every scored id to be an active member of the
Stellar Membership contract, which lives in a separate repository. The
contract's own README, [`contracts/governance/README.md`](contracts/governance/README.md),
covers its whole interface, the error codes and the link to the membership
contract.


## Contents

- [How it works](#how-it-works)
- [Requirements](#requirements)
- [Build the prover](#build-the-prover)
- [Deploy the contracts](#deploy-the-contracts)
- [Generate a proof](#generate-a-proof)
- [Submit the scores](#submit-the-scores)
- [Calculate the voting powers](#calculate-the-voting-powers)
- [Read the results](#read-the-results)
- [The contract](#the-contract)
- [What is proven and what is trusted](#what-is-proven-and-what-is-trusted)
- [After changing a neuron](#after-changing-a-neuron)


## How it works

```mermaid
flowchart LR
    env[".env<br/>CURRENT_ROUND"] --> host
    input["private input JSON"] --> host["host prove<br/>(zkvm/)"]
    host --> output["output JSON<br/>currentRound, scores,<br/>journal, seal, imageId"]
    output --> script["submit-neuron.ts"]
    script -->|set_neuron_result| contract["NQG contract<br/>(contracts/governance)"]
    contract -->|member| membership["Stellar Membership<br/>contract"]
    contract -->|verify| verifier["Groth16 verifier<br/>contract"]
```

1. You put the round being scored in `.env` in the repository root.
2. `host prove` runs the selected neuron as a guest program in the zkVM with
   the private input file. The guest calculates the scores and commits the
   round and the scores as its public output, the journal. The prover wraps the
   run in a Groth16 proof and writes one output JSON file.
3. `submit-neuron.ts` reads that file, checks that its round is the contract's
   active round, scales every score to an integer with 6 decimal places, and
   calls `set_neuron_result` on the NQG contract for the layer and neuron you
   name.
4. The contract checks that the caller is the admin and that every scored id
   is an active member, hashes the journal, and asks the Groth16 verifier
   whether the proof matches the journal and the neuron's Image ID. Only when
   all checks pass does it store the scores and the proof under that layer,
   neuron and round. A failed check stores nothing.
5. When every neuron has scores for the round, the admin calls
   `calculate_voting_powers`, and the contract weighs and adds up the scores
   into each member's voting power.


## Requirements

For proving:

- [Rust](https://rustup.rs/). `zkvm/` uses the stable toolchain from
  [`zkvm/rust-toolchain.toml`](zkvm/rust-toolchain.toml).
- [`rzup`](https://github.com/risc0/risc0/blob/main/risc0/cargo-risczero/README.md)
  and `r0vm` 3.0.6. The host calls this binary to prove locally. No remote
  prover is used.

  ```sh
  curl -L https://risczero.com/install | bash
  rzup install r0vm 3.0.6
  rzup use r0vm 3.0.6
  ```

- Docker with BuildKit support (Docker Desktop or Colima with Docker Buildx).
  Docker Desktop includes Buildx and BuildKit; no separate Buildx installation
  is needed. For an existing Colima setup on macOS, install Buildx with
  [Homebrew](https://formulae.brew.sh/formula/docker-buildx):

  ```sh
  brew install docker-buildx
  mkdir -p "$HOME/.docker/cli-plugins"
  ln -sfn "$(brew --prefix)/opt/docker-buildx/bin/docker-buildx" \
    "$HOME/.docker/cli-plugins/docker-buildx"
  docker buildx version
  ```

  The symlink makes the Homebrew plugin discoverable by Docker, as described
  in the [Colima setup instructions](https://github.com/abiosoft/colima/blob/main/docs/FAQ.md#docker-buildx-plugin-is-missing).
  If starting from scratch with Colima, first run `brew install colima docker`.

  Docker is used twice. Building the prover compiles the guests in
  `risczero/risc0-guest-builder:r0.1.91.1`, pinned by SHA-256 digest in
  [`zkvm/methods/build.rs`](zkvm/methods/build.rs). Proving runs
  `risczero/risc0-groth16-prover:v2025-04-03.1`, which `r0vm` starts to turn
  the proof into Groth16.

  On macOS with Colima, give the virtual machine 8 GiB of memory; with the
  default 2 GiB the Groth16 container is killed. On Apple Silicon also enable
  Rosetta: the guest builder exists only for `linux/amd64`, and without
  Rosetta Colima emulates it with QEMU, which is much slower:

  ```sh
  colima stop
  colima start --vz-rosetta --memory 8 --cpu 4
  ```

  Later `colima start` calls keep these settings. If Colima refuses
  `--vz-rosetta` because the existing virtual machine uses QEMU, recreate it
  with `colima delete` and run the start command again. This deletes the
  images and containers stored in Colima.

  After starting Docker Desktop or Colima, verify both the plugin and the
  connection to Docker:

  ```sh
  docker buildx version
  docker info
  ```

For deploying and submitting:

- [Stellar CLI](https://developers.stellar.org/docs/tools/developer-tools/cli/install-cli).
  Also add the WASM target:

  ```sh
  rustup target add wasm32v1-none
  ```

- Node.js 22.18 or newer. The submit script is TypeScript that Node runs
  directly. It has no runtime npm dependencies, so `npm install` is not needed.


## Build the prover

Start Docker first: the build compiles the guests inside the guest builder
image and fails if Docker is not running.

```sh
cargo build --release --locked --manifest-path zkvm/Cargo.toml -p host
```

The first build downloads the guest builder image and might take
several minutes. Later builds reuse Docker's cache.

The binary is `zkvm/target/release/host`. It has two commands:

| Command | What it does |
| --- | --- |
| `host image-id --neuron <neuron>` | Prints the Image ID of the guest compiled into this binary |
| `host prove --neuron <neuron> --input <json> --output <json>` | Generates a proof |

`<neuron>` is `prior-voting-history`, `assigned-reputation` or `trust-graph`.
If `--neuron` is left out, `prior-voting-history` is used, so always pass it.

Print the three Image IDs. You need them in
[step 3 of the deployment](#3-put-the-image-ids-into-the-contract):

```sh
zkvm/target/release/host image-id --neuron prior-voting-history
zkvm/target/release/host image-id --neuron assigned-reputation
zkvm/target/release/host image-id --neuron trust-graph
```

Because the guests are compiled in a pinned Docker image, the same commit gives
the same Image IDs on every machine and in every folder. Anyone can check the
Image IDs in the contract by building that commit and running these commands.


## Deploy the contracts

> **Contracts already deployed?** Skip to [Generate a proof](#generate-a-proof).
> You need the NQG contract id as `<CONTRACT_ID>`, the layer and neuron id of
> each neuron (see [step 5](#5-set-up-the-layers)), and the admin key added to
> the Stellar CLI with `stellar keys add admin --secret-key`. Build the prover
> from the commit the contract was deployed or last upgraded from; otherwise
> the Image IDs may differ and the contract rejects your proofs.

You deploy the Groth16 verifier, then the NQG contract, and then set up its
layers. The NQG contract also needs a Stellar Membership contract, deployed
from its own repository.

### 1. Create the accounts

The **deployer** pays for the deployments. The **admin** is the only account
that can configure the NQG contract and call `set_neuron_result`. They can be
the same account. On testnet:

```sh
stellar keys generate deployer --network testnet
stellar keys fund deployer --network testnet

stellar keys generate admin --network testnet
stellar keys fund admin --network testnet

stellar keys address admin
```

### 2. Deploy the Groth16 verifier

Skip this step if a Groth16 verifier from `stellar-risc0-verifier` already
exists on your network and you trust that deployment. Then use its contract id
below.

Clone the verifier repository outside this repository and run the following
inside that clone:

```sh
git clone https://github.com/NethermindEth/stellar-risc0-verifier.git
cd stellar-risc0-verifier

stellar contract build --package groth16-verifier

stellar contract deploy \
  --wasm target/wasm32v1-none/release/groth16_verifier.wasm \
  --source-account deployer \
  --network testnet
```

Save the printed contract id as `<VERIFIER_ID>`. Go back to this repository.

The verifier accepts seals whose first 4 bytes match the selector built into
its WASM. Proofs from `r0vm` 3.0.6 match the parameters shipped with the
verifier.

### 3. Put the Image IDs into the contract

The contract has the Image IDs as constants in
[`contracts/governance/src/verifier.rs`](contracts/governance/src/verifier.rs):

```rust
const PRIOR_VOTING_HISTORY_IMAGE_ID: [u8; 32] = [ ... ];
const ASSIGNED_REPUTATION_IMAGE_ID: [u8; 32] = [ ... ];
const TRUST_GRAPH_IMAGE_ID: [u8; 32] = [ ... ];
```

Compare all three with the output of `host image-id` from
[Build the prover](#build-the-prover). If they differ, replace the bytes. This
command prints an Image ID in the array format:

```sh
zkvm/target/release/host image-id --neuron prior-voting-history \
  | sed 's/../0x&, /g'
```

A contract with a wrong Image ID deploys without error, but every
`set_neuron_result` call for that neuron fails with `InvalidProof` (error 20).

### 4. Deploy the NQG contract

The NQG contract and the membership contract each store the other's address.
Deploy the membership contract first, as described in
[Deploying the pair](contracts/governance/README.md#deploying-the-pair), and
save its contract id as `<MEMBERSHIP_ID>`.

```sh
stellar contract build --manifest-path contracts/Cargo.toml --package governance

stellar contract deploy \
  --wasm contracts/target/wasm32v1-none/release/governance.wasm \
  --source-account deployer \
  --network testnet \
  -- \
  --admin "<ADMIN_ADDRESS>" \
  --current_round 33 \
  --membership_contract "<MEMBERSHIP_ID>" \
  --verifier "<VERIFIER_ID>"
```

`<ADMIN_ADDRESS>` is the `G...` address printed by `stellar keys address admin`.
`--current_round` is the round you are going to score. Save the printed
contract id as `<CONTRACT_ID>`, and point the membership contract at it with
`set_nqg_contract` (see the link above).

The admin can change all of this later: `set_current_round`,
`set_membership_contract`, `set_verifier`, `transfer_admin`, and `upgrade` for
new code (see [After changing a neuron](#after-changing-a-neuron)).

### 5. Set up the layers

The contract weighs each neuron's scores and combines them in layers. This
sets up one layer that adds up the three neurons, each with weight 1.0:

```sh
stellar contract invoke --network testnet --source admin --id "<CONTRACT_ID>" \
  -- add_layer \
  --raw_neurons '[["PriorVotingHistory",1000000],["AssignedReputation",1000000],["TrustGraph",1000000]]' \
  --layer_aggregator Sum
```

Each entry is a name and a weight with 6 decimals: `1000000` is 1.0. The name
is only a label. Layers get the ids `0`, `1`, ... in the order they are added,
and the neurons of a layer get the ids `0`, `1`, ... in the order they are
listed. Here prior voting history is layer `0` neuron `0`, assigned reputation
layer `0` neuron `1`, and trust graph layer `0` neuron `2`. You pass these ids
when you submit. Check the setup with:

```sh
stellar contract invoke --send=no --network testnet --source admin \
  --id "<CONTRACT_ID>" \
  -- \
  get_neuron --layer_id 0 --neuron_id 2
```

This returns `{"name":"TrustGraph","weight":1000000}`. The weights here are an
example. `--layer_aggregator` is `Sum` or `Product`, and a later layer is added
on top of the earlier ones; see
[the contract README](contracts/governance/README.md#interface).


## Generate a proof

### 1. Set the round

`host prove` reads `CURRENT_ROUND` only from a `.env` file in the working
directory or one of its parents. A shell variable with the same name is
ignored. The round is not part of the input file.

```sh
echo "CURRENT_ROUND=33" > .env
```

This replaces the whole file. If `.env` holds other settings, edit the
`CURRENT_ROUND` line instead. `.env` is in `.gitignore`.

### 2. Point RISC Zero at a work directory Docker can see

On macOS the default work directory is under `/var/folders`, which Colima does
not mount, so the Groth16 container would see no files. Run this in the same
terminal as `host prove`:

```sh
mkdir -p "$HOME/.risc0/groth16-work"
export RISC0_WORK_DIR="$HOME/.risc0/groth16-work"
```

### 3. Prove

Docker must be running. Create the output directory once:

```sh
mkdir -p zkvm/artifacts
```

Assigned reputation, the faster example:

```sh
zkvm/target/release/host prove --neuron assigned-reputation \
  --input zkvm/data/example_assigned_reputation.json \
  --output zkvm/artifacts/assigned-reputation-33.json
```

Prior voting history:

```sh
zkvm/target/release/host prove --neuron prior-voting-history \
  --input zkvm/data/example_prior_voting_history.json \
  --output zkvm/artifacts/prior-voting-history-33.json
```

Trust graph:

```sh
zkvm/target/release/host prove --neuron trust-graph \
  --input zkvm/data/example_trust_graph.json \
  --output zkvm/artifacts/trust-graph-33.json
```

The files in `zkvm/data/` are synthetic. Use your private input file for real
data. Proving takes several minutes.

`host prove` never overwrites files. Each run needs a new `--output` path, and
its parent directory must exist. The path is checked after proving, so pick a
new name before you start. Putting the round in the
file name is a simple way to keep them apart.

### The output file

`--output` holds everything needed on chain, from a single run:

```json
{
  "currentRound": 33,
  "scores": {
    "0": 1.5,
    "1": 2.25
  },
  "journal": "<hex>",
  "seal": "<hex>",
  "imageId": "<64 hex characters>"
}
```

| Field | What it is |
| --- | --- |
| `currentRound` | The round from `.env`, as proven in the journal |
| `scores` | Person (membership token) id to score, decoded from the journal |
| `journal` | Raw journal bytes, hex |
| `seal` | Groth16 proof with the 4-byte selector, hex |
| `imageId` | Image ID of the guest that produced the proof |

Keep the `--output` file. It is what you submit, and it is the only copy of
the proof.


## Submit the scores

```sh
node scripts/submit-neuron.ts \
  --neuron assigned-reputation \
  --output zkvm/artifacts/assigned-reputation-33.json \
  --layer-id 0 \
  --neuron-id 1 \
  --contract "<CONTRACT_ID>" \
  --source admin \
  --network testnet
```

All seven flags are required and unknown flags are rejected. `--neuron` must
match the neuron that produced the output file, as in `host prove`; with the
wrong neuron, the verifier rejects the proof because the Image ID differs.
`--layer-id` and `--neuron-id` choose where the contract stores the scores, as
set up in [step 5](#5-set-up-the-layers). `--source` is the admin identity name
in the Stellar CLI, or its secret key.

The script:

1. Reads `currentRound`, `scores`, `journal`, and `seal` from the output file.
2. Checks that every id is a decimal `u32`.
3. Scales every score to an integer with 6 decimal places, using exact
   decimal arithmetic. `1.5` becomes `1500000`. Digits after the 6th are
   dropped. Scores in exponent notation, such as `1e-7`, and scores that do
   not fit an `i64` are rejected.
4. Asks the contract for its active round and stops if it is not
   `currentRound`. The contract stores the scores under its active round, not
   under the round in the file.
5. Asks the contract for the neuron and stops if it does not exist. The
   contract would otherwise store the scores under any layer and neuron id.
6. Prints the neuron's name and runs:

   ```sh
   stellar contract invoke --id <CONTRACT_ID> --source <SOURCE> --network <NETWORK> \
     -- set_neuron_result \
     --layer_id 0 \
     --neuron_id 1 \
     --result '{"0":3000000,"1":3500000}' \
     --guest AssignedReputation \
     --journal <JOURNAL_HEX> \
     --seal <SEAL_HEX>
   ```

Steps 4 and 5 are simulations and cost no fee. The Stellar CLI output is shown
as is. A non-zero exit code means the script stopped or the call failed, and
nothing was stored. The contract refuses the call with `NotAMember` (error 18)
when an id is not an active member, and with `InvalidProof` (error 20) when the
verifier does not accept the proof.

Submitting again for the same layer, neuron and round replaces the stored
scores and proof. Results for other rounds stay unchanged.


## Calculate the voting powers

When every neuron of every layer has scores for the active round, the admin
calculates the voting powers:

```sh
stellar contract invoke --network testnet --source admin --id "<CONTRACT_ID>" \
  -- calculate_voting_powers
```

All layers must cover the same members, otherwise the call fails with
`LayerResultsUsersMismatch` (error 17). A neuron without scores for the round
fails it with `NeuronResultNotSet` (error 7). After replacing any scores,
calculate again.


## Read the results

Anyone can read the stored data. `--send=no` simulates the call and does not
create a transaction or cost a fee. The CLI still needs a `--source` account
that exists on the network, but it can be any account, not only `admin`.

```sh
stellar contract invoke --send=no --network testnet --source admin \
  --id "<CONTRACT_ID>" \
  -- \
  get_voting_powers
```

This returns the map from member id to voting power for the active round,
with 6 decimals, for example `{"0":3000000,"1":13002128}`.
`get_voting_power_for_user --member_id 1` returns one member's.

```sh
stellar contract invoke --send=no --network testnet --source admin \
  --id "<CONTRACT_ID>" \
  -- \
  get_neuron_result --layer_id 0 --neuron_id 1
```

This returns one neuron's scores for the active round, for example
`{"0":3000000,"1":3500000}`. Divide by 10^6 for the original value.

```sh
stellar contract invoke --send=no --network testnet --source admin \
  --id "<CONTRACT_ID>" \
  -- \
  get_neuron_proof --layer_id 0 --neuron_id 1
```

This returns the `seal`, `image_id`, and `journal_digest` that were checked.
For an earlier round use `get_neuron_result_round` and `get_neuron_proof_round`
with `--round`. They return `NeuronResultNotSet` (error 7) and
`NeuronProofNotSet` (error 21) when nothing is stored for that neuron and
round.


## The contract

The NQG contract is a Soroban contract in the [`contracts/`](contracts) Cargo
workspace, using `soroban-sdk` 28. Its
[README](contracts/governance/README.md) lists every function and error code.
The proof check is in
[`contracts/governance/src/verifier.rs`](contracts/governance/src/verifier.rs).

`set_neuron_result(layer_id, neuron_id, result, guest, journal, seal)` step by
step:

1. Requires the admin's signature.
2. Calls `member(id)` on the membership contract for every id in `result` and
   requires each one to be active.
3. Takes the Image ID constant of `guest`: `PriorVotingHistory`,
   `AssignedReputation` or `TrustGraph`.
4. Computes `journal_digest = sha256(journal)`.
5. Calls `verify(seal, image_id, journal_digest)` on the verifier. If the
   verifier rejects the proof or cannot answer, the call fails with
   `InvalidProof`.
6. Stores `result`, and `NeuronProof { seal, image_id, journal_digest }` next
   to it, under the layer, the neuron and the active round.

The contract stores only the journal's digest, which is what `verify` needs.
The full journal is still an argument, so the contract computes the digest
itself and the caller cannot pass a digest that does not belong to the journal.
The journal therefore stays readable in the `set_neuron_result` transaction in
Stellar's transaction history, but not in contract storage, so other contracts
cannot read it.

The tests use a mock verifier, so they check what the contract sends to it,
not a real Groth16 proof. Run them in `contracts/`:

```sh
cd contracts
stellar contract build
cargo test --all-features
```

`stellar contract build` comes first because one test upgrades the contract to
a mock WASM.


## What is proven and what is trusted

A valid proof shows that the guest with this Image ID ran correctly and that
the journal, the round and the scores, is its output. The contract stores
nothing unless such a proof exists for the submitted journal.

What is not proven:

- **The input.** The proof does not show that the voting history, roles, or
  tiers are real or complete, or who supplied them. The input provider is
  trusted.
- **The round.** The proof shows which round the scores were calculated for,
  not that it is the actual current round. The contract does not compare the
  round in the journal with its active round; the submit script does.
- **That the stored scores are the journal's.** The contract does not decode
  the journal. It stores `result` as sent, and the submit script sends the
  scores that `host prove` decoded from the journal. The admin is trusted to
  send matching scores, and anyone can check them: the journal is in the
  `set_neuron_result` transaction, and its digest is stored with the proof.
- **The layer and neuron.** The contract does not tie a guest to a layer or
  neuron. The admin chooses both.

Privacy: the journal contains only the round and the scores. It contains no
votes, participation history, author lists, roles, or tiers, and no commitment
to the input. The scores themselves still reveal something about each
member's activity.

## After changing a neuron

Changes to a guest in `zkvm/methods/` or to a core crate in `zkvm/core/`
can change the Image ID, even removing a comment line, because panic messages
in the binary can contain line numbers. A dependency update or a change to
the pinned Docker builder can change it too.

After such a change:

1. Rebuild the prover and run `host image-id` for the three neurons.
2. Update the constants in
   [`contracts/governance/src/verifier.rs`](contracts/governance/src/verifier.rs).
3. Run the contract tests, as in [The contract](#the-contract).
4. Build the contract, upload the new WASM, and upgrade the deployed contract
   with the hash that `upload` prints:

   ```sh
   stellar contract build --manifest-path contracts/Cargo.toml --package governance

   stellar contract upload \
     --wasm contracts/target/wasm32v1-none/release/governance.wasm \
     --source-account admin \
     --network testnet

   stellar contract invoke --network testnet --source admin --id "<CONTRACT_ID>" \
     -- upgrade --wasm_hash <WASM_HASH>
   ```

The contract keeps its address, its setup, and the scores, proofs and voting
powers stored so far. Proofs stored before the upgrade keep the Image ID they
were checked against.
