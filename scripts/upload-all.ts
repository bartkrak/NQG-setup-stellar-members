import { existsSync } from "node:fs";
import { parseArgs } from "node:util";
import { loadConfig, target } from "./lib/config.ts";
import { activeRound, layout, neuronName, setNeuronResult, type Slot } from "./lib/governance.ts";
import { NEURONS, type Neuron } from "./lib/neurons.ts";
import { checkCoverage, readProofOutput, type ProofOutput } from "./lib/proof.ts";
import { readSnapshot, runPaths, writeJson } from "./lib/run.ts";
import { read, send } from "./lib/stellar.ts";

const USAGE = `Usage:
  node scripts/upload-all.ts --run <run dir> [--config <flow.config.json>]

Sends every proof of a run to the governance contract, then calculates the
voting powers and prints them.`;

/** Voting powers have 6 decimals. */
const DECIMALS = 1_000_000;

function parseFlags() {
  try {
    const { values } = parseArgs({
      options: { run: { type: "string" }, config: { type: "string" } },
      strict: true,
    });
    if (!values.run) throw new Error("missing --run");
    return { run: values.run, config: values.config };
  } catch (error) {
    throw new Error(`${error instanceof Error ? error.message : error}\n${USAGE}`);
  }
}

/** `submitted.json`: what was sent, written after every step. */
type Submitted = {
  governance: string;
  round: number;
  uploads: Partial<Record<Neuron, Slot & { sentAt: string }>>;
  calculatedAt?: string;
  votingPowers?: Record<string, number>;
};

const sameSlot = (a: Slot, b: Slot) => a.layerId === b.layerId && a.neuronId === b.neuronId;
const describe = (slot: Slot) => `layer ${slot.layerId} neuron ${slot.neuronId}`;

function main() {
  const flags = parseFlags();
  const config = loadConfig(flags.config);
  const snapshot = readSnapshot(flags.run);
  const paths = runPaths(flags.run);
  if (snapshot.governance !== config.governance || snapshot.rpcUrl !== config.rpcUrl) {
    throw new Error(
      `the run was collected for ${snapshot.governance} at ${snapshot.rpcUrl}, ` +
        `the config points at ${config.governance} at ${config.rpcUrl}`,
    );
  }

  const outputs = {} as Record<Neuron, ProofOutput>;
  for (const neuron of NEURONS) {
    const path = paths.proof(neuron);
    if (!existsSync(path)) {
      throw new Error(`no proof for ${neuron}; run node scripts/prove-all.ts --run ${flags.run}`);
    }
    outputs[neuron] = readProofOutput(path);
    checkCoverage(path, outputs[neuron], snapshot.round, snapshot.users);
  }

  // Simulations only: nothing is sent unless all of these pass.
  const governance = target(config, config.governance);
  const round = activeRound(governance);
  if (round !== String(snapshot.round)) {
    throw new Error(
      `the run is for round ${snapshot.round}, the contract's active round is ${round}; ` +
        "collect and prove again",
    );
  }
  const membership = String(read(governance, "get_membership_contract"));
  if (membership !== snapshot.membership) {
    throw new Error(
      `the contract now validates against ${membership}, the run read ${snapshot.membership}; ` +
        "collect and prove again",
    );
  }
  // calculate_voting_powers needs a result for every neuron of every layer.
  const configured = NEURONS.map((neuron) => config.neurons[neuron]);
  const unfilled = layout(governance).filter((slot) => !configured.some((c) => sameSlot(c, slot)));
  if (unfilled.length) {
    throw new Error(
      `the contract has neurons the config fills with nothing: ${unfilled.map(describe).join(", ")}`,
    );
  }
  for (const neuron of NEURONS) {
    const slot = config.neurons[neuron];
    const name = neuronName(governance, slot);
    if (name !== neuron) {
      throw new Error(`${describe(slot)} is named "${name}", the config puts ${neuron} there`);
    }
  }

  const submitted: Submitted = {
    governance: config.governance,
    round: snapshot.round,
    uploads: {},
  };
  for (const neuron of NEURONS) {
    const slot = config.neurons[neuron];
    console.error(
      `\n== Sending ${neuron}: ${outputs[neuron].ids.length} scores to ${describe(slot)}...`,
    );
    setNeuronResult(governance, slot, neuron, outputs[neuron]);
    submitted.uploads[neuron] = { ...slot, sentAt: new Date().toISOString() };
    writeJson(paths.submitted, submitted);
  }

  console.error("\n== Calculating voting powers...");
  send(governance, "calculate_voting_powers");
  const powers = read(governance, "get_voting_powers") as Record<string, number>;
  submitted.calculatedAt = new Date().toISOString();
  submitted.votingPowers = powers;
  writeJson(paths.submitted, submitted);

  console.log(`\nVoting powers, round ${snapshot.round}:`);
  for (const [id, power] of Object.entries(powers).sort(([a], [b]) => Number(a) - Number(b))) {
    console.log(`  ${id.padStart(6)}  ${(power / DECIMALS).toFixed(6)}`);
  }
  console.error(`\nSaved to ${paths.submitted}`);
}

try {
  main();
} catch (error) {
  console.error(error instanceof Error ? error.message : error);
  process.exit(1);
}
