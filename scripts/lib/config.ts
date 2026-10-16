import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import type { Slot } from "./governance.ts";
import { NEURONS, type Neuron } from "./neurons.ts";
import type { Target } from "./stellar.ts";

export type VotingHistoryConfig = {
  tansuContract: string;
  parentProjectKey: string;
  explorerUrl: string;
};

/** `scripts/flow.config.json`, see `flow.config.example.json`. */
export type FlowConfig = {
  rpcUrl: string;
  networkPassphrase: string;
  /** The NQG governance contract. */
  governance: string;
  /** stellar CLI identity of the governance admin. */
  source: string;
  votingHistory: VotingHistoryConfig;
  /** Where each neuron's scores go in the governance contract. */
  neurons: Record<Neuron, Slot>;
};

export const DEFAULT_CONFIG = fileURLToPath(new URL("../flow.config.json", import.meta.url));

export function loadConfig(path = DEFAULT_CONFIG): FlowConfig {
  let data;
  try {
    data = JSON.parse(readFileSync(path, "utf8"));
  } catch (error) {
    throw new Error(
      `cannot read config ${path}: ${error instanceof Error ? error.message : error}\n` +
        "Copy scripts/flow.config.example.json to scripts/flow.config.json and fill it in.",
    );
  }
  const fail = (message: string) => new Error(`config ${path}: ${message}`);
  for (const field of ["rpcUrl", "networkPassphrase", "governance", "source"]) {
    if (typeof data?.[field] !== "string" || !data[field]) throw fail(`${field} must be set`);
  }
  for (const field of ["tansuContract", "parentProjectKey", "explorerUrl"]) {
    if (typeof data.votingHistory?.[field] !== "string" || !data.votingHistory[field]) {
      throw fail(`votingHistory.${field} must be set`);
    }
  }
  for (const neuron of NEURONS) {
    const slot = data.neurons?.[neuron];
    if (typeof slot?.layerId !== "string" || typeof slot?.neuronId !== "string") {
      throw fail(`neurons.${neuron} must have a layerId and a neuronId (strings)`);
    }
  }
  return data as FlowConfig;
}

/**
 * Network flags for the stellar CLI. Explicit, so that a `.env` with
 * `STELLAR_RPC_URL` or `STELLAR_NETWORK_PASSPHRASE` cannot redirect the calls.
 */
export function networkFlags(config: FlowConfig): string[] {
  return ["--rpc-url", config.rpcUrl, "--network-passphrase", config.networkPassphrase];
}

export function target(config: FlowConfig, contract: string): Target {
  return { contract, source: config.source, network: networkFlags(config) };
}
