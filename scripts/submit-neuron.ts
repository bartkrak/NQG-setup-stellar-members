import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { parseArgs } from "node:util";

type Output = {
  currentRound: string;
  scores: [string, string][];
  journal: string;
  seal: string;
}

/** `host prove --neuron` name to the `guest` the governance contract expects. */
const NEURONS: Record<string, string> = {
  "prior-voting-history": "PriorVotingHistory",
  "assigned-reputation": "AssignedReputation",
  "trust-graph": "TrustGraph",
};

/** Neuron results in the governance contract are `i64` with 6 decimals. */
const DECIMALS = 6;
const I64_MIN = -(2n ** 63n);
const I64_MAX = 2n ** 63n - 1n;

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

function scaleToI64(token: string): string {
  const match = /^(-?)(\d+)(?:\.(\d+))?$/.exec(token);
  if (!match) throw new Error(`bad score ${token}`);
  const [, sign, whole, fraction = ""] = match;
  const magnitude = BigInt(whole + fraction.padEnd(DECIMALS, "0").slice(0, DECIMALS));
  const value = sign ? -magnitude : magnitude;
  if (value < I64_MIN || value > I64_MAX) {
    throw new Error(`score ${token} does not fit an i64 with ${DECIMALS} decimals`);
  }
  return value.toString();
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

/**
 * The scores as a JSON map for the CLI. Built by hand because the values go
 * out as JSON numbers, which the CLI reads as `i64`, and a JS number loses
 * digits above 2^53.
 */
function scaledScoresJson(entries: Array<[string, string]>): string {
  const fields = entries.map(([id, score]) => {
    if (!/^(0|[1-9]\d*)$/.test(id) || BigInt(id) > 0xffff_ffffn) {
      throw new Error(`membership token ${id} must be a u32`);
    }
    return `${JSON.stringify(id)}:${scaleToI64(score)}`;
  });
  return `{${fields.join(",")}}`;
}

/** Run the stellar CLI. With `capture`, return its stdout instead of showing it. */
function stellar(args: string[], capture = false): string {
  const result = spawnSync("stellar", args, {
    stdio: capture ? ["ignore", "pipe", "inherit"] : "inherit",
    encoding: "utf8",
  });
  if (result.error) {
    const notFound = (result.error as NodeJS.ErrnoException).code === "ENOENT";
    throw notFound ? new Error("stellar CLI not found") : result.error;
  }
  if (result.status !== 0) {
    throw new Error(`stellar failed: ${result.signal ?? `exit code ${result.status}`}`);
  }
  return capture ? result.stdout.trim() : "";
}

function invokeArgs(flags: Flags, send: boolean, fn: string, args: string[]): string[] {
  return [
    "contract", "invoke",
    ...(send ? [] : ["--send=no"]),
    "--id", flags.contract,
    "--source", flags.source,
    "--network", flags.network,
    "--",
    fn,
    ...args,
  ];
}

function main() {
  const flags = parseFlags();
  const guest = NEURONS[flags.neuron];
  if (!guest) throw new Error(`unknown neuron ${flags.neuron}`);
  const slot = ["--layer_id", flags["layer-id"], "--neuron_id", flags["neuron-id"]];

  const { currentRound, scores: entries, journal, seal } = readOutput(
    readFileSync(flags.output, "utf8"),
  );
  const scores = scaledScoresJson(entries);

  // The contract stores the result under its active round, which is not an
  // argument; refuse an output proven for another round.
  const activeRound = stellar(invokeArgs(flags, false, "get_current_round", []), true);
  if (activeRound !== currentRound) {
    throw new Error(
      `the output is for round ${currentRound}, the contract's active round is ${activeRound}`,
    );
  }
  // The contract accepts any layer and neuron id; check this one exists.
  const { name } = JSON.parse(stellar(invokeArgs(flags, false, "get_neuron", slot), true));

  console.log(
    `Sending ${guest} round ${currentRound} scores to layer ${flags["layer-id"]} ` +
      `neuron ${flags["neuron-id"]} (${name})...`,
  );
  stellar(invokeArgs(flags, true, "set_neuron_result", [
    ...slot,
    "--result", scores,
    "--guest", guest,
    "--journal", journal,
    "--seal", seal,
  ]));
}

try {
  main();
} catch (error) {
  console.error(error instanceof Error ? error.message : error);
  process.exit(1);
}
