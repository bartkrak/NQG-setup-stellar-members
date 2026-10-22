import { spawnSync } from "node:child_process";

/**
 * A contract call through the stellar CLI: the contract, the account that
 * signs (or, for reads, only pays for the simulation) and the flags that
 * select the network.
 */
export type Target = {
  contract: string;
  source: string;
  network: string[];
};

export function isContractError(error: unknown, code: number): boolean {
  const message = error instanceof Error ? error.message : String(error);
  const lines = message.split("\n");
  const errorLine = lines.find(line => line.includes("error:")) ?? lines[0];
  return errorLine.includes(`Error(Contract, #${code})`);
}

/** Run the Stellar CLI, returning stdout or throwing its error message. */
export function stellar(args: string[]): string {
  const result = spawnSync("stellar", args, {
    stdio: ["ignore", "pipe", "pipe"],
    encoding: "utf8",
    timeout: 60_000,
    maxBuffer: 16 * 1024 * 1024,
  });
  if (result.error) {
    const notFound = (result.error as NodeJS.ErrnoException).code === "ENOENT";
    throw notFound ? new Error("stellar CLI not found") : result.error;
  }
  if (result.status !== 0) {
    throw new Error(result.stderr.trim() || `stellar failed: ${result.signal ?? `exit code ${result.status}`}`);
  }
  return result.stdout.trim();
}

function invokeArgs(target: Target, send: boolean, fn: string, args: string[]): string[] {
  return [
    "contract", "invoke",
    send ? "--send=yes" : "--send=no",
    "--id", target.contract,
    "--source", target.source,
    ...target.network,
    "--",
    fn,
    ...args,
  ];
}

/** Simulate a call and return its result as parsed JSON. No transaction, no fee. */
export function read(target: Target, fn: string, args: string[] = []): unknown {
  return JSON.parse(stellar(invokeArgs(target, false, fn, args)));
}

/** Send a call as a transaction signed by the source. */
export function send(target: Target, fn: string, args: string[] = []): void {
  stellar(invokeArgs(target, true, fn, args));
}
