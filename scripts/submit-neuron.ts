import { parseArgs } from "node:util";
import { activeRound, neuronName, setNeuronResult } from "./lib/governance.ts";
import { GUESTS, isNeuron } from "./lib/neurons.ts";
import { readProofOutput } from "./lib/proof.ts";
import type { Target } from "./lib/stellar.ts";

const USAGE = `Usage:
  node scripts/submit-neuron.ts \\
    --neuron prior-voting-history|assigned-reputation|trust-graph \\
    --output <output.json> \\
    --layer-id <id> \\
    --neuron-id <id> \\
    --contract <id> \\
    --source <account> \\
    --network <network>`;

const FLAGS = [
  "neuron", "output", "layer-id", "neuron-id", "contract", "source", "network",
] as const;

type Flags = Record<(typeof FLAGS)[number], string>;

function parseFlags(): Flags {
  const fail = (message: string) => new Error(`${message}\n${USAGE}`);
  let values: Record<string, unknown>;
  try {
    ({ values } = parseArgs({
      options: Object.fromEntries(FLAGS.map((name) => [name, { type: "string" as const }])),
      strict: true,
    }));
  } catch (error) {
    throw fail(error instanceof Error ? error.message : String(error));
  }
  for (const name of FLAGS) {
    if (typeof values[name] !== "string") throw fail(`missing --${name}`);
  }
  return values as Flags;
}

function main() {
  const flags = parseFlags();
  const neuron = flags.neuron;
  if (!isNeuron(neuron)) throw new Error(`unknown neuron ${neuron}`);
  const output = readProofOutput(flags.output);
  const governance: Target = {
    contract: flags.contract,
    source: flags.source,
    network: ["--network", flags.network],
  };
  const slot = { layerId: flags["layer-id"], neuronId: flags["neuron-id"] };

  // The contract stores the result under its active round, which is not an
  // argument; refuse an output proven for another round.
  const round = activeRound(governance);
  if (round !== output.currentRound) {
    throw new Error(
      `the output is for round ${output.currentRound}, the contract's active round is ${round}`,
    );
  }
  // The contract accepts any layer and neuron id; check this one exists.
  const name = neuronName(governance, slot);

  console.log(
    `Sending ${GUESTS[neuron]} round ${output.currentRound} scores to layer ${slot.layerId} ` +
      `neuron ${slot.neuronId} (${name})...`,
  );
  setNeuronResult(governance, slot, neuron, output);
}

try {
  main();
} catch (error) {
  console.error(error instanceof Error ? error.message : error);
  process.exit(1);
}
