# Neuron proofs with RISC Zero

Generate a zero-knowledge proof of a neuron's calculation and verify its public
result without sharing the private voting history. The current implementation
supports the **prior voting history** neuron.

There are two command-line programs:

- **`host`** reads a private JSON input and generates a receipt using a local prover.
- **`verifier`** checks a receipt against an independently approved Image ID and
  prints the verified round and scores.

A **guest** is the program executed inside the RISC Zero zkVM. Its **Image ID**
identifies the compiled program. It depends only on the source code and the
build environment, never on the input. A **receipt** contains a cryptographic
proof and its public output (the **journal**).


## Three separate checks

| Check | What it establishes | Needs | Status |
| --- | --- | --- | --- |
| Reproduce the Image ID from source | The published source compiles to this Image ID | Git, Rust, Docker with Buildx | Available |
| Verify a proof | A receipt was produced by the program with a given Image ID and carries this journal | Rust (to build `verifier`), the receipt, the approved Image ID | Available for local receipts |
| Read the approved Image ID on-chain | Which Image ID the application contract accepts | Stellar tooling, the contract address | Not yet available: no contract |

The checks are independent. Reproducing the Image ID needs no proof and no private
input; verifying a proof needs no Docker or guest compiler. Only the
combination links a published source to an accepted result.

### Release process

1. A source revision is published (a Git tag) and its Image ID is approved in the
   application contract.
2. The administrator builds that revision with
   [scripts/build-reproducible.sh](scripts/build-reproducible.sh), checks the
   printed ID against the approved one, and generates proofs from the private
   voting history.
3. The contract accepts a result only if the proof is valid for the approved
   Image ID and its journal matches the submitted result.
4. Anyone can rebuild the published revision and compare the Image ID with the
   one read from the contract.


## Reproduce the Image ID

This is the same step for independent reviewers and for the administrator.

### 1. Install the tools

You need **Git**, **Rust** through [rustup](https://rustup.rs) (the host toolchain
is selected automatically from [rust-toolchain.toml](rust-toolchain.toml)), and a
running **Docker with the Buildx plugin**. RISC Zero tools (`rzup`, `r0vm`, the
RISC Zero Rust toolchain) and the private input are **not** needed: the guest is
compiled inside the pinned builder image.

Buildx is required because RISC Zero exports the compiled guest with `docker
build --output`, which the legacy builder does not support. Check it with
`docker buildx version`.

**macOS**: [Docker Desktop](https://docs.docker.com/desktop/) or
[OrbStack](https://orbstack.dev) include Buildx. With Colima, add the plugin
once:

```sh
brew install git colima docker docker-buildx
mkdir -p ~/.docker/cli-plugins
ln -sfn "$(brew --prefix)/opt/docker-buildx/bin/docker-buildx" ~/.docker/cli-plugins/docker-buildx
```

Run `colima start` before building.

**Linux**: install [Docker Engine](https://docs.docker.com/engine/install/) from
docker.com; its packages include Buildx. 

Also install a C toolchain for the host build, for example
`sudo apt install -y git build-essential` on Debian/Ubuntu.

**Windows**: use WSL 2, not native Windows. The script needs Bash, and RISC Zero
tools do not support native Windows. In PowerShell as administrator:

```powershell
wsl --install -d Ubuntu
```

Install Docker Desktop and enable WSL integration for the Ubuntu distribution,
or install Docker Engine inside Ubuntu as on Linux. Then follow the Linux steps
in the Ubuntu terminal. Clone the repository inside the Linux file system (for
example under `~`), not under `/mnt/c`, which is much slower.

### 2. Build and compare

```sh
zkvm/scripts/build-reproducible.sh "<APPROVED_IMAGE_ID>"
```

The script prints the Image ID, then `Image ID matches the approved value.` Omit
the argument to print the ID only. A mismatch exits with an error. The first build
downloads the builder image (over 1 GB) and can take several minutes.

The guest is recompiled in the container on every run, so the printed ID always
comes from the current source. The script also produces the native host at
`zkvm/target/reproducible/release/host`, which contains exactly that guest.

### What is pinned

- the source revision, including [Cargo.lock](Cargo.lock) and the guest's
  [Cargo.lock](methods/prior-voting-history-guest/Cargo.lock);
- RISC Zero SDK `3.0.6` (`risc0-build`, `risc0-zkvm`);
- the guest builder image `risczero/risc0-guest-builder`, pinned by digest in
  [methods/build.rs](methods/build.rs). The digest selects a single
  `linux/amd64` image.

The host toolchain and the Docker installation do not enter the guest binary.
An earlier Podman flow produced the same Image ID for two builds from different
source paths on one Apple Silicon Mac. This script's Docker flow, Linux, and WSL
have not been confirmed yet. Publish the source tag, builder image digest, and
Image ID for each approved release.

Keep real private history outside `zkvm/`: the whole directory is sent to
Docker as the build context, and Git ignores do not exclude files from it.


## Generate a proof

For the administrator. Build the host as described in
[Reproduce the Image ID](#reproduce-the-image-id), passing the approved ID so
that proofs are generated only by the approved program.

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

Start with [data/example_input.json](data/example_input.json). It contains synthetic example data.

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
[neuron.rs](prior-voting-history-core/src/neuron.rs). Rounds are processed from 1
through `currentRound`; generation time increases with the computation workload.

### 3. Generate the receipt locally

```sh
mkdir -p zkvm/artifacts
zkvm/target/reproducible/release/host prove \
  --input zkvm/data/example_input.json \
  --receipt zkvm/artifacts/receipt.bin
```

Replace `--input` with your private file path when using real data. The host
explicitly uses local `r0vm`; it does not select a remote proving service. The
input changes the journal and the proof, not the Image ID.

The command prints `Proof generated in ...s` and saves the binary receipt.
**It does not run a separate verification step.** It also does not overwrite an
existing receipt: choose a new filename for each run. Parent directories must exist.

Proving can take several minutes. The receipt is a local RISC Zero receipt, not
yet a payload accepted by a Stellar contract; see
[Planned Stellar / Soroban integration](#planned-stellar--soroban-integration).


## Verify a received proof

### 1. Build only the verifier

```sh
cargo build --release --locked --manifest-path zkvm/Cargo.toml -p verifier
```

Keep `-p verifier`: the default workspace build selects `host` and builds its guest.
The verifier package does not depend on the guest build package, so no container
engine or RISC Zero tools are needed.

### 2. Obtain the receipt and approved Image ID

Download the receipt as a binary file, for example to `zkvm/artifacts/receipt.bin`
(create the directory with `mkdir -p zkvm/artifacts`). Generated receipts are
ignored by Git, so cloning the repository does not provide one.

Take the expected Image ID from the application contract or a release you have
independently accepted, never from whoever sent the receipt. It must contain
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

The journal exposes only `currentRound` and `scores`. It contains no individual
votes, participation history, author lists, or input-history commitment. Public
scores still reveal information about activity. The binary receipt format itself
is not encryption.

Accepting an arbitrary receipt and Image ID from the same sender only proves an
execution of the sender's chosen program. Approval of the program identity is a
separate responsibility.
