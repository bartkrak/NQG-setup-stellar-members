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

/** Run the stellar CLI and return its stdout. Its stderr is shown as is. */
export function stellar(args: string[]): string {
  const result = spawnSync("stellar", args, {
    stdio: ["ignore", "pipe", "inherit"],
    encoding: "utf8",
  });
  if (result.error) {
    const notFound = (result.error as NodeJS.ErrnoException).code === "ENOENT";
    throw notFound ? new Error("stellar CLI not found") : result.error;
  }
  if (result.status !== 0) {
    throw new Error(`stellar failed: ${result.signal ?? `exit code ${result.status}`}`);
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
