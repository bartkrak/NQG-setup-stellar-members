# Neuron proofs with RISC Zero

Generate a zero-knowledge proof of a neuron's calculation and verify its public
result without sharing the private input. Supports **prior voting history** and
**assigned reputation**, each with its own core crate, guest, and Image ID.

There are two command-line programs:

- **`host`** reads a private JSON input and generates a receipt using a local prover.
- **`verifier`** checks a receipt against an independently approved Image ID and
  prints the verified neuron output as JSON.

A **guest** is the program executed inside the RISC Zero zkVM. Its **Image ID**
identifies the compiled program. It depends only on the source code and the
build environment, never on the input. A **receipt** contains a cryptographic
proof and its public output (the **journal**).


## Local build and proving

The workspace contains `core/prior-voting-history/` and
`core/assigned-reputation/` for each neuron's types and scoring, two guests in
`methods/`, one `host/`, and the standalone `verifier/`. 

The verifier keeps argument parsing in `cli.rs`, journal decoding in `journal.rs`,
and cryptographic verification in `lib.rs`. Its `main.rs` only coordinates those
steps and prints the result.

Use Rust via rustup, the RISC Zero Rust guest toolchain installed through `rzup`,
and `r0vm` 3.0.6. With those tools installed, run from the repository root:

```sh
cargo build --release --locked --manifest-path zkvm/Cargo.toml -p host -p verifier

mkdir -p zkvm/artifacts

zkvm/target/release/host image-id --neuron prior-voting-history

zkvm/target/release/host prove --neuron prior-voting-history \
  --input zkvm/data/example_prior_voting_history.json \
  --receipt zkvm/artifacts/prior.bin

zkvm/target/release/verifier verify --neuron prior-voting-history \
  --receipt zkvm/artifacts/prior.bin --image-id "<APPROVED_PRIOR_IMAGE_ID>"
```

The host always uses local `r0vm`. Development receipts are disabled. The verifier
rejects fake receipts explicitly, verifies the supplied Image ID independently of
neuron selection, then decodes the journal as the selected output type. It has no
guest build dependency and can be built by itself with `-p verifier`.

## Generate a proof

Build the host as described in [Local build and proving](#local-build-and-proving).
Compare the selected guest's `host image-id --neuron ...` output with its
independently approved Image ID before generating proofs.

### 1. Install the prover

Install [`rzup`](https://github.com/risc0/risc0/blob/main/risc0/cargo-risczero/README.md)
and the prover binary used by the host:

```sh
curl -L https://risczero.com/install | bash
rzup install r0vm 3.0.6
```

Follow the installer's PATH instructions or start a new terminal. If multiple
versions are installed, select this one with `rzup use r0vm 3.0.6`.

### 2. Prepare the private input

Start with [data/example_prior_voting_history.json](data/example_prior_voting_history.json). It contains synthetic example data.

The input has five required fields:

| Field | Meaning |
| --- | --- |
| `currentRound` | Last included round, inclusive; must be at least 8 |
| `users` | Person IDs whose scores should be calculated |
| `usersRoundHistory` | Person ID to a list of participation rounds |
| `votesPerRound` | Round to submission to person ID to vote |
| `submittersPerRound` | Round to a list of submission authors' IDs |

Votes are `Yes`, `No`, `Delegate`, or `Abstain`, with that capitalization. Missing
or unknown fields, nonnumeric person IDs, duplicate list entries, and rounds with
no submissions in `votesPerRound` are rejected. An individual submission may have
an empty votes object.

For each round, submission authors receive full activity. Other users must appear
in that round's participation history. Before round 32, participation receives
full activity; from round 32 onward, activity is `max(active votes / submissions,
0.5)` when round vote data is present. Missing detailed round data contributes
zero for non-authors. Only `Yes` and `No` count as active votes.

The original logistic weighting and final bonus curve remain in
[neuron.rs](core/prior-voting-history/src/neuron.rs). Rounds are processed from 1
through `currentRound`; generation time increases with the computation workload.

### 3. Generate the receipt locally

```sh
mkdir -p zkvm/artifacts
zkvm/target/release/host prove \
  --input zkvm/data/example_prior_voting_history.json \
  --receipt zkvm/artifacts/receipt.bin
```

Replace `--input` with your private file path when using real data. The host
explicitly uses local `r0vm`; it does not select a remote proving service. The
input changes the journal and the proof, not the Image ID.

The command prints `Proof generated in ...s` and saves the binary receipt.
**It does not run a separate verification step.** It also does not overwrite an
existing receipt: choose a new filename for each run. Parent directories must exist.

Proving can take several minutes. This workflow produces a local RISC Zero
receipt for verification with the standalone verifier.


## Verify a received proof

### 1. Build only the verifier

```sh
cargo build --release --locked --manifest-path zkvm/Cargo.toml -p verifier
```

Keep `-p verifier`: the default workspace build selects `host` and builds both guests.
The verifier package does not depend on the guest build package, so neither the
RISC Zero guest toolchain nor `r0vm` is needed for verification.

### 2. Obtain the receipt and approved Image ID

Download the receipt as a binary file, for example to `zkvm/artifacts/receipt.bin`
(create the directory with `mkdir -p zkvm/artifacts`). Generated receipts are
ignored by Git, so cloning the repository does not provide one.

Take the expected Image ID from a source revision and local build you have
independently accepted, never solely from whoever sent the receipt. It must contain
**64 hexadecimal characters**.

### 3. Verify

```sh
zkvm/target/release/verifier verify \
  --receipt zkvm/artifacts/receipt.bin \
  --image-id "<APPROVED_IMAGE_ID>"
```

For the bundled example, successful verification returns:

```json
{
  "currentRound": 33,
  "scores": {
    "52345125252": 0.20848214882407576,
    "52345125253": 0.06825242602146757,
    "52345125254": 0.03410388656983916,
    "52345125255": 0.0
  }
}
```

It also prints `Proof verified in ...s` to stderr. JSON goes to stdout. To save it,
append `> verified-result.json` to the verification command. Use the process exit
status to determine success: failed verification exits with a nonzero status and
does not publish result JSON.




## What the proof guarantees

A valid receipt establishes correct execution of the program identified by the
supplied Image ID and binds its public journal to that execution. It does not
establish that the private history is authentic or complete, or identify its
provider. This project trusts the history provider for those properties.

The prior-voting-history journal exposes only `currentRound` and `scores`; the
assigned-reputation journal exposes only its person-ID/score pairs in `scores`.
Neither contains individual votes, participation history, author lists, roles,
tiers, or an input-data commitment. Public
scores still reveal information about activity. The binary receipt format itself
is not encryption.

Accepting an arbitrary receipt and Image ID from the same sender only proves an
execution of the sender's chosen program. Approval of the program identity is a
separate responsibility.
