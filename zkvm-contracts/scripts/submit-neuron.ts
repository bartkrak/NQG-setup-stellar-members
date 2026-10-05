import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { parseArgs } from "node:util";

type Output = {
  currentRound: string;
  scores: [string, string][];
  journal: string;
  seal: string;
}

const NEURONS: Record<string, string> = {
  "prior-voting-history": "PriorVotingHistory",
  "assigned-reputation": "AssignedReputation",
};

const USAGE = `Usage:
  node zkvm-contracts/scripts/submit-neuron.ts \\
    --neuron assigned-reputation|prior-voting-history \\
    --output <output.json> \\
    --contract <id> \\
    --source <account> \\
    --network <network>`;

const FLAGS = ["neuron", "output", "contract", "source", "network"] as const;

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

function scaleTo1e18(token: string): string {
  const match = /^(-?)(\d+)(?:\.(\d+))?$/.exec(token);
  if (!match) throw new Error(`bad score ${token}`);
  const [, sign, whole, fraction = ""] = match;
  const value = BigInt(whole + fraction.padEnd(18, "0").slice(0, 18));
  return value === 0n ? "0" : `${sign}${value}`;
}

function readOutput(json: string): Output {
  const data = JSON.parse(json);

  if(data?.currentRound === undefined || data?.currentRound === null) {
    throw new Error("output JSON has no currentRound");
  }
  if (!data.scores || typeof data.scores !== "object" || Array.isArray(data.scores)) {
    throw new Error("output JSON has no scores map");
  }
  if (typeof data.journal !== "string" || !data.journal) {
    throw new Error("output JSON has no journal");
  }
  if (typeof data.seal !== "string" || !data.seal) {
    throw new Error("output JSON has no seal");
  }

  return {
    currentRound: String(data.currentRound),
    scores: Object.entries(data.scores).map(([id, score]): [string, string] => [id, String(score)]),
    journal: data.journal,
    seal: data.seal,
  };
}

function scaledScoresJson(entries: Array<[string, string]>): string {
  const scores: Record<string, string> = {};
  for (const [id, score] of entries) {
    if (!/^(0|[1-9]\d*)$/.test(id) || BigInt(id) > 0xffff_ffffn) {
      throw new Error(`membership token ${id} must be a u32`);
    }
    scores[id] = scaleTo1e18(score);
  }
  return JSON.stringify(scores);
}

function stellar(args: string[]) {
  const result = spawnSync("stellar", args, { stdio: "inherit" });
  if (result.error) {
    const notFound = (result.error as NodeJS.ErrnoException).code === "ENOENT";
    throw notFound ? new Error("stellar CLI not found") : result.error;
  }
  if (result.status !== 0) {
    throw new Error(`stellar failed: ${result.signal ?? `exit code ${result.status}`}`);
  }
}

function main() {
  const flags = parseFlags();
  const neuron = NEURONS[flags.neuron];
  if (!neuron) throw new Error(`unknown neuron ${flags.neuron}`);

  const { currentRound, scores: entries, journal, seal } = readOutput(
    readFileSync(flags.output, "utf8"),
  );
  const scores = scaledScoresJson(entries);

  console.log(`Sending ${neuron} round ${currentRound} scores...`);
  stellar([
    "contract", "invoke",
    "--id", flags.contract,
    "--source", flags.source,
    "--network", flags.network,
    "--",
    "set_neuron_result",
    "--neuron", neuron,
    "--round", currentRound,
    "--result", scores,
    "--journal", journal,
    "--seal", seal,
  ]);
}

try {
  main();
} catch (error) {
  console.error(error instanceof Error ? error.message : error);
  process.exit(1);
}
