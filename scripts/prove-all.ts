import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";
import { NEURONS, type Neuron } from "./lib/neurons.ts";
import { checkCoverage, readProofOutput } from "./lib/proof.ts";
import { readSnapshot, runPaths } from "./lib/run.ts";

const USAGE = `Usage:
  node scripts/prove-all.ts --run <run dir> [--host <host binary>]

Proves every neuron of a run made by collect.ts, one after another, into
<run dir>/proofs/. Neurons that already have a proof are skipped.`;

const DEFAULT_HOST = fileURLToPath(new URL("../zkvm/target/release/host", import.meta.url));
const VERIFIER_RS = fileURLToPath(
  new URL("../contracts/governance/src/verifier.rs", import.meta.url),
);

/** The constant in `verifier.rs` holding each guest's Image ID. */
const IMAGE_ID_CONSTS: Record<Neuron, string> = {
  "prior-voting-history": "PRIOR_VOTING_HISTORY_IMAGE_ID",
  "assigned-reputation": "ASSIGNED_REPUTATION_IMAGE_ID",
  "trust-graph": "TRUST_GRAPH_IMAGE_ID",
};

function parseFlags() {
  try {
    const { values } = parseArgs({
      options: { run: { type: "string" }, host: { type: "string" } },
      strict: true,
    });
    if (!values.run) throw new Error("missing --run");
    return { run: values.run, host: values.host ?? DEFAULT_HOST };
  } catch (error) {
    throw new Error(`${error instanceof Error ? error.message : error}\n${USAGE}`);
  }
}

/** The Image IDs the governance contract in this checkout accepts, as hex. */
function contractImageIds(): Record<Neuron, string> {
  const source = readFileSync(VERIFIER_RS, "utf8");
  return Object.fromEntries(
    NEURONS.map((neuron) => {
      const name = IMAGE_ID_CONSTS[neuron];
      const body = new RegExp(`const ${name}: \\[u8; 32\\] = \\[([^\\]]*)\\]`).exec(source)?.[1];
      const bytes = body?.match(/0x[0-9a-fA-F]{2}/g);
      if (bytes?.length !== 32) throw new Error(`cannot read ${name} from ${VERIFIER_RS}`);
      return [neuron, bytes.map((byte) => byte.slice(2).toLowerCase()).join("")];
    }),
  ) as Record<Neuron, string>;
}

function hostImageId(host: string, neuron: Neuron): string {
  const result = spawnSync(host, ["image-id", "--neuron", neuron], { encoding: "utf8" });
  if (result.status !== 0) {
    throw new Error(`${host} image-id failed: ${result.error?.message ?? result.stderr}`);
  }
  return result.stdout.trim();
}

function dockerRunning(): boolean {
  return spawnSync("docker", ["info"], { stdio: "ignore" }).status === 0;
}

/** Check a proof file against the run and the contract's Image ID. */
function checkProof(path: string, round: number, users: string[], imageId: string) {
  const output = readProofOutput(path);
  checkCoverage(path, output, round, users);
  if (output.imageId !== imageId) {
    throw new Error(`${path}: Image ID ${output.imageId}, the contract expects ${imageId}`);
  }
}

function main() {
  const flags = parseFlags();
  const { round, users } = readSnapshot(flags.run);
  const paths = runPaths(flags.run);

  if (!existsSync(flags.host)) {
    throw new Error(
      `no prover at ${flags.host}. Build it (Docker running):\n` +
        "  cargo build --release --locked --manifest-path zkvm/Cargo.toml -p host",
    );
  }
  // A proof from a guest the contract does not know is rejected with
  // InvalidProof; find out before spending minutes on it.
  const expected = contractImageIds();
  const mismatches = NEURONS.flatMap((neuron) => {
    const actual = hostImageId(flags.host, neuron);
    return actual === expected[neuron]
      ? []
      : [`  ${neuron}: host ${actual}, contract ${expected[neuron]}`];
  });
  if (mismatches.length) {
    throw new Error(
      `the prover's Image IDs differ from contracts/governance/src/verifier.rs:\n` +
        `${mismatches.join("\n")}\n` +
        "Rebuild the prover from the commit the contract was deployed from, or update the " +
        "constants and upgrade the contract.",
    );
  }

  const todo = NEURONS.filter((neuron) => !existsSync(paths.proof(neuron)));
  if (todo.length && !dockerRunning()) {
    throw new Error("Docker is not running; the Groth16 step needs it (colima start)");
  }
  // Colima does not mount the default work directory under /var/folders.
  const env = { ...process.env };
  env.RISC0_WORK_DIR ??= join(homedir(), ".risc0", "groth16-work");
  mkdirSync(env.RISC0_WORK_DIR, { recursive: true });
  mkdirSync(paths.proofs, { recursive: true });

  for (const neuron of NEURONS) {
    const proof = paths.proof(neuron);
    if (todo.includes(neuron)) {
      console.error(`\n== Proving ${neuron} for round ${round} (${users.length} users)...`);
      const start = Date.now();
      const result = spawnSync(
        flags.host,
        [
          "prove", "--neuron", neuron,
          "--round", String(round),
          "--input", paths.input(neuron),
          "--output", proof,
        ],
        { stdio: "inherit", env },
      );
      if (result.status !== 0) {
        throw new Error(
          `proving ${neuron} failed: ${result.error?.message ?? `exit code ${result.status}`}\n` +
            "If docker returned exit code 137, the Groth16 container ran out of memory: give " +
            "Docker 8 GiB and stop other containers while proving.",
        );
      }
      checkProof(proof, round, users, expected[neuron]);
      console.error(`== ${neuron} done in ${((Date.now() - start) / 1000).toFixed(0)}s`);
    } else {
      checkProof(proof, round, users, expected[neuron]);
      console.error(`== ${neuron}: already proven, skipped`);
    }
  }
  console.error(`\nAll proofs are in ${paths.proofs}. Next: node scripts/upload-all.ts --run <dir>`);
}

try {
  main();
} catch (error) {
  console.error(error instanceof Error ? error.message : error);
  process.exit(1);
}
