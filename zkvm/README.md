# Neuron proofs with RISC Zero

Generate a Groth16 proof of a neuron's scores without publishing the private
input. The workspace has two neurons, **prior voting history** and **assigned
reputation**. Each has its own core crate, guest program, and Image ID.

You will:

1. Install the local prover.
2. Build `host`.
3. Generate a receipt and read the public scores, the raw journal, and the proof.
4. Deploy the Groth16 verifier on Stellar testnet.
5. Ask that contract to check the proof.

A **guest** is the program that runs inside the RISC Zero zkVM. Its **Image ID**
identifies that compiled program. It depends on the source and the build, and
stays the same when the input changes. A **receipt** is the Groth16 proof plus
its public output, the **journal**. The journal holds the scores. `journal_digest`
is the SHA-256 hash of those bytes. 

`host prove` writes one JSON file, `--output`, with everything needed on chain:

| Field | What it is |
| --- | --- |
| `currentRound`, `scores` | The decoded journal, for reading the scores |
| `journal` | Raw journal bytes, hex |
| `seal` | Groth16 proof bytes, prefixed with a 4-byte selector, hex |
| `imageId` | Identity of the guest that produced the journal |

Run every command below from the repository root unless a step says otherwise.


## 1. Install the prover

Install Rust with [rustup](https://rustup.rs/). This workspace uses the stable
toolchain in [rust-toolchain.toml](rust-toolchain.toml).

Install [`rzup`](https://github.com/risc0/risc0/blob/main/risc0/cargo-risczero/README.md)
and `r0vm` 3.0.6. The host calls this binary. It does not use a remote prover.

```sh
curl -L https://risczero.com/install | bash
rzup install r0vm 3.0.6
```

Follow the installer's `PATH` instructions, or open a new terminal. If several
`r0vm` versions are installed, select this one:

```sh
rzup use r0vm 3.0.6
```

`r0vm` proves a STARK on this machine, then starts the Docker image
`risczero/risc0-groth16-prover:v2025-04-03.1` to wrap that STARK as Groth16.
The image includes `linux/arm64`, so Colima on Apple Silicon can run it. Leave
`DOCKER_DEFAULT_PLATFORM` unset.

Docker must be running before `host prove`. On macOS with Colima, the virtual
machine needs more than the default 2 GiB of memory. At 2 GiB the Groth16
container is killed and `host prove` fails. Give Colima 8 GiB:

```sh
colima stop
colima start --memory 8 --cpu 4
```

Later `colima start` keeps that memory and CPU count.

On macOS, RISC Zero's default work directory is under `/var/folders`. Colima
does not mount that path, so the container would see an empty `/mnt`. Put the
work directory in your home folder, which Colima does mount. Run this in the
same terminal that will run `host prove`. A new terminal needs the `export`
again:

```sh
mkdir -p "$HOME/.risc0/groth16-work"
export RISC0_WORK_DIR="$HOME/.risc0/groth16-work"
```

`r0vm` reads `RISC0_WORK_DIR` from the environment it inherits from `host`. It
writes the STARK seal into that directory and mounts the directory into the
container as `/mnt`. The container reads `input.json` and writes `proof.json`.
`r0vm` turns `proof.json` into the Groth16 receipt, and `host` saves the
receipt as the `.bin` file. 


## 2. Build the host

```sh
cargo build --release --locked --manifest-path zkvm/Cargo.toml -p host
```

The binary is `zkvm/target/release/host`.


## 3. Read the guest Image ID

Print the Image ID of the guest compiled into this binary. Compare it with the
Image ID you have approved before you accept scores from a proof.

```sh
zkvm/target/release/host image-id --neuron prior-voting-history
zkvm/target/release/host image-id --neuron assigned-reputation
```

`host prove` writes the same kind of value as `imageId` in `--output`, taken
from the receipt. Those two strings match when the receipt was produced by this binary.


## 4. Generate a proof

Create the output directory once:

```sh
mkdir -p zkvm/artifacts
```

`host prove` saves a new receipt and writes the decoded scores together with
`journal`, `seal`, and `imageId` to `--output`. It leaves an existing receipt
or output file in
place, so each run needs a new
`--receipt` path and a new `--output` path. The parent directory must already
exist. `Proof generated in ...s` goes to stderr. Proving
takes several minutes. Groth16 wraps the STARK after the guest finishes, so it
takes longer than a composite receipt. The host asks `r0vm` for Groth16 only.

Both neurons need the round being scored. `host prove` reads `CURRENT_ROUND`
only from a `.env` file in the working directory or one of its parents. A shell
variable with the same name is ignored. Create the file in the repository root:

```sh
echo "CURRENT_ROUND=33" > .env
```

The round is not part of the input file. The guest commits it as the first
field of the journal, so it is proven together with the scores, and the
`--output` file of both neurons has `currentRound` next to `scores`. The proof
shows which round the scores were calculated for. It does not show that this
round is the real current round.

### Prior voting history

```sh
zkvm/target/release/host prove --neuron prior-voting-history \
  --input zkvm/data/example_prior_voting_history.json \
  --receipt zkvm/artifacts/prior-voting-history.bin \
  --output zkvm/artifacts/prior-voting-history.json
```

[data/example_prior_voting_history.json](data/example_prior_voting_history.json)
is synthetic. Replace `--input` with your private file for real data. The input
changes the journal and the proof. It does not change the Image ID.

The JSON has four required fields:

| Field | Meaning |
| --- | --- |
| `users` | Person IDs whose scores should be calculated |
| `usersRoundHistory` | Person ID to a list of participation rounds |
| `votesPerRound` | Round to submission to person ID to vote |
| `submittersPerRound` | Round to a list of submission authors' IDs |

Votes are `Yes`, `No`, `Delegate`, or `Abstain`, with that capitalization.
Missing or unknown fields, nonnumeric person IDs, duplicate list entries, and
rounds with no submissions in `votesPerRound` are rejected. One submission may
have an empty votes object.

For each round, submission authors receive full activity. Other users must
appear in that round's participation history. Before round 32, participation
receives full activity. From round 32 onward, activity is `max(active votes /
submissions, 0.5)` when round vote data is present. Missing detailed round data
contributes zero for non-authors. Only `Yes` and `No` count as active votes.
The logistic weighting and final bonus curve are in
[neuron.rs](core/prior-voting-history/src/neuron.rs). Rounds run from 1 through
`CURRENT_ROUND`, inclusive, so a larger round takes longer. This neuron needs
`CURRENT_ROUND` to be at least 8.

The `--output` file is JSON with `currentRound` and `scores`. `scores` maps a
person ID to that person's score.

### Assigned reputation

This example input is smaller than prior voting history, so it is the faster
proof to try first.

```sh
zkvm/target/release/host prove --neuron assigned-reputation \
  --input zkvm/data/example_assigned_reputation.json \
  --receipt zkvm/artifacts/assigned-reputation.bin \
  --output zkvm/artifacts/assigned-reputation.json
```

[data/example_assigned_reputation.json](data/example_assigned_reputation.json)
is a `users` list. Each user has `id`, `tier`, and `discord_roles`. Tier `0` is
Verified, `1` Pathfinder, `2` Navigator, `3` Pilot. Any other tier is Unknown.
The bonus for each user is the tier bonus plus the bonus for each listed role.
The role names and amounts are in
[lib.rs](core/assigned-reputation/src/lib.rs).

The round does not change the scores. It is only copied into the journal.

The `--output` file is JSON with `currentRound` and `scores`, which maps a
person ID to that person's score, the same as prior voting history. The proven
journal itself holds the scores as `(id, score)` pairs in the same order as
`users`.


## 5. Check the proof on Stellar testnet

Submit `seal` and `imageId` from the `--output` file to `verify` on the
Groth16 verifier from
[stellar-risc0-verifier](https://github.com/NethermindEth/stellar-risc0-verifier).
The contract argument named `journal` is the SHA-256 of the raw `journal` bytes.
`host prove` does not write that hash.

The contract checks that the seal matches the Image ID and the journal digest.
It does not decode scores. 

Install the [Stellar CLI](https://developers.stellar.org/docs/tools/developer-tools/cli/install-cli).
Clone `stellar-risc0-verifier` and run the following
inside that clone:

```sh
stellar keys generate deployer --network testnet
stellar keys fund deployer --network testnet

rustup target add wasm32v1-none
stellar contract build --package groth16-verifier

stellar contract deploy \
  --wasm target/wasm32v1-none/release/groth16_verifier.wasm \
  --source-account deployer \
  --network testnet
```


Copy the contract id, and `seal` and `imageId` from the `--output` file. Pass
the SHA-256 of the `journal` bytes as `--journal`. This call asks
testnet to run `verify` and print the result. `--send=no` means the call is
simulated and is not recorded as a transaction:

```sh
stellar contract invoke \
  --send=no \
  --network testnet \
  --source deployer \
  --id "<GROTH16_VERIFIER_CONTRACT_ID>" \
  -- \
  verify \
  --seal "<SEAL>" \
  --image_id "<IMAGE_ID>" \
  --journal "<JOURNAL_DIGEST>"
```

A successful simulation prints `null`. `verify` returns an empty success value,
and the CLI prints that empty value as `null`.

The first four bytes of `seal` are the selector baked into this WASM. A receipt
from `r0vm` 3.0.6 matches the parameters shipped with the verifier when those
four bytes are the same.


## 6. Store the scores after verification

[`neuron-result`](../zkvm-contracts/neuron-result) `set_neuron_result` takes the
score map, the round, and `journal` and `seal` from the `--output` file. It hashes
those journal bytes, uses the Image ID of the selected neuron, and calls
`verify` on the Groth16 verifier address saved at deployment. When that call
succeeds, it stores the scores and the journal, seal, image id, and digest.
Nothing is stored when `verify` fails. `journal_digest` is not an argument.

`round` is an argument, and the result is stored under that neuron and round.
The contract does not check it against the `currentRound` in the journal, so
the admin is trusted to pass the right round, the same as for the score map.
Results for earlier rounds stay readable. A later call that also verifies for
the same neuron and round replaces that result.

Build this contract from the repository root. Reuse the verifier deployed in
section 5; do not deploy another copy of `groth16_verifier.wasm`.

```sh
stellar contract build --manifest-path zkvm-contracts/Cargo.toml --package neuron-result

stellar contract deploy \
  --wasm zkvm-contracts/target/wasm32v1-none/release/neuron_result.wasm \
  --source-account deployer \
  --network testnet \
  -- \
  --admin "<ADMIN_ADDRESS>" \
  --verifier "<GROTH16_VERIFIER_CONTRACT_ID>"
```

`get_neuron_result --neuron AssignedReputation --round 33` reads the score map
for that round. `get_proof` takes the same arguments and reads the stored
journal and seal. `PriorVotingHistory` is the other `--neuron` value.

The contract has the Image IDs of both guests built in. After any change to a
guest or its core crate, run `host image-id` again, update
`PRIOR_VOTING_HISTORY_IMAGE_ID` and `ASSIGNED_REPUTATION_IMAGE_ID` in
[lib.rs](../zkvm-contracts/neuron-result/src/lib.rs), and deploy the contract
again.


## What the proof guarantees

A valid receipt shows that the program identified by the Image ID ran correctly
and that this journal is the public output of that run. It does not show that
the private history is authentic or complete, and it does not name who supplied
that history. This project trusts the history provider for those properties.

Both journals start with `currentRound`. The prior-voting-history journal then
contains `scores` as a map, and the assigned-reputation journal contains
person-ID and score pairs in `scores`.
Neither journal contains individual votes, participation history, author lists,
roles, tiers, or a commitment to the input. Public scores still reveal
information about activity. The `.bin` receipt is a serialization of the proof
and the journal. It is not an encrypted copy of the private input.
