import { mkdirSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, resolve } from "node:path";
import { parseArgs } from "node:util";
import type { TrustKeyFile } from "./lib/trust-lists.ts";
import { keyPair, newPrivateKey } from "./lib/trust.ts";

const USAGE = `Usage:
  node scripts/trust-key.ts --out <file>

Generates the operator key trust lists are encrypted to and writes it to
<file>, readable by you only, never over an existing file. Keep it outside
the repository: whoever holds it reads every member's trust list. Prints the
public key, which the NQG admin gives to the contract with set_trust_key.`;

function parseFlags() {
  try {
    const { values } = parseArgs({ options: { out: { type: "string" } }, strict: true });
    if (!values.out) throw new Error("missing --out");
    return { out: resolve(values.out.replace(/^~(?=\/)/, homedir())) };
  } catch (error) {
    throw new Error(`${error instanceof Error ? error.message : error}\n${USAGE}`);
  }
}

async function main() {
  const { out } = parseFlags();
  const privateKey = newPrivateKey();
  const { publicKey } = await keyPair(privateKey);
  const file: TrustKeyFile = {
    publicKey: Buffer.from(publicKey).toString("hex"),
    privateKey: Buffer.from(privateKey).toString("hex"),
  };
  mkdirSync(dirname(out), { recursive: true, mode: 0o700 });
  writeFileSync(out, `${JSON.stringify(file, null, 2)}\n`, { flag: "wx", mode: 0o600 });
  console.error(
    `Wrote the operator key to ${out}. Give its public key to the NQG contract with set_trust_key (README step 3).`,
  );
  console.log(file.publicKey);
}

try {
  await main();
} catch (error) {
  console.error(error instanceof Error ? error.message : error);
  process.exit(1);
}
