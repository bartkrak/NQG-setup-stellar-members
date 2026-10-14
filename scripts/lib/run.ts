import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import type { Neuron } from "./neurons.ts";

/** A minted membership token as read from chain. */
export type Member = { id: number; status: number; role: number };

/** `snapshot.json`: what `collect.ts` read, and for which contracts. */
export type Snapshot = {
  createdAt: string;
  rpcUrl: string;
  networkPassphrase: string;
  governance: string;
  membership: string;
  /** The governance contract's active round when collected. */
  round: number;
  /** Ledger the members were read at. */
  ledger: number;
  /** Every minted token, revoked ones too. */
  members: Member[];
  /** Active members: the ids every neuron scores. */
  users: string[];
  /** Input fields that came from `scripts/mocks/`, not from chain. */
  mocked: string[];
};

/**
 * A run folder, the hand-off between the scripts:
 * `collect.ts` writes the snapshot and the inputs, `prove-all.ts` the proofs,
 * `upload-all.ts` the record of what it sent.
 */
export function runPaths(dir: string) {
  return {
    dir,
    snapshot: join(dir, "snapshot.json"),
    inputs: join(dir, "inputs"),
    proofs: join(dir, "proofs"),
    input: (neuron: Neuron) => join(dir, "inputs", `${neuron}.json`),
    proof: (neuron: Neuron) => join(dir, "proofs", `${neuron}.json`),
    submitted: join(dir, "submitted.json"),
  };
}

export function readSnapshot(dir: string): Snapshot {
  const path = runPaths(dir).snapshot;
  try {
    return JSON.parse(readFileSync(path, "utf8"));
  } catch (error) {
    throw new Error(`cannot read ${path}: ${error instanceof Error ? error.message : error}`);
  }
}

export function writeJson(path: string, value: unknown) {
  writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`, "utf8");
}
