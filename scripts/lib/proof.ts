import { readFileSync } from "node:fs";

/** A `host prove --output` file, checked and ready for `set_neuron_result`. */
export type ProofOutput = {
  currentRound: string;
  /** Scored membership token ids, as in the file. */
  ids: string[];
  /** The scores as the CLI `--result` argument: `{"<id>":<i64>,...}`. */
  result: string;
  journal: string;
  seal: string;
  imageId: string;
};

/** Neuron results in the governance contract are `i64` with 6 decimals. */
const DECIMALS = 6;
const I64_MIN = -(2n ** 63n);
const I64_MAX = 2n ** 63n - 1n;

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

/**
 * The scores as a JSON map for the CLI. Built by hand because the values go
 * out as JSON numbers, which the CLI reads as `i64`, and a JS number loses
 * digits above 2^53.
 */
function scaledScoresJson(entries: Array<[string, unknown]>): string {
  const fields = entries.map(([id, score]) => {
    if (!/^(0|[1-9]\d*)$/.test(id) || BigInt(id) > 0xffff_ffffn) {
      throw new Error(`membership token ${id} must be a u32`);
    }
    // serde_json writes a NaN or infinite f64 as null.
    if (typeof score !== "number") {
      throw new Error(`score of ${id} is ${JSON.stringify(score)}, not a finite number`);
    }
    if (Math.abs(score) < 10 ** -DECIMALS) score = 0;
    return `${JSON.stringify(id)}:${scaleToI64(String(score))}`;
  });
  return `{${fields.join(",")}}`;
}

export function readProofOutput(path: string): ProofOutput {
  let data;
  try {
    data = JSON.parse(readFileSync(path, "utf8"));
  } catch (error) {
    throw new Error(`cannot read ${path}: ${error instanceof Error ? error.message : error}`);
  }
  const fail = (message: string) => new Error(`${path}: ${message}`);

  if (data?.currentRound === undefined || data?.currentRound === null) {
    throw fail("no currentRound");
  }
  if (!data.scores || typeof data.scores !== "object" || Array.isArray(data.scores)) {
    throw fail("no scores map");
  }
  for (const field of ["journal", "seal", "imageId"]) {
    if (typeof data[field] !== "string" || !data[field]) throw fail(`no ${field}`);
  }

  const entries = Object.entries(data.scores);
  let result;
  try {
    result = scaledScoresJson(entries);
  } catch (error) {
    throw fail(error instanceof Error ? error.message : String(error));
  }
  return {
    currentRound: String(data.currentRound),
    ids: entries.map(([id]) => id),
    result,
    journal: data.journal,
    seal: data.seal,
    imageId: data.imageId,
  };
}

/** Require a proof for `round` that scores exactly `users`. */
export function checkCoverage(path: string, output: ProofOutput, round: number, users: string[]) {
  if (output.currentRound !== String(round)) {
    throw new Error(`${path}: proven for round ${output.currentRound}, the run is for ${round}`);
  }
  const scored = new Set(output.ids);
  const missing = users.filter((id) => !scored.has(id));
  const expected = new Set(users);
  const extra = output.ids.filter((id) => !expected.has(id));
  if (missing.length || extra.length) {
    throw new Error(
      `${path}: scores do not match the run's users` +
        (missing.length ? `; missing ${missing.join(", ")}` : "") +
        (extra.length ? `; not users ${extra.join(", ")}` : ""),
    );
  }
}
