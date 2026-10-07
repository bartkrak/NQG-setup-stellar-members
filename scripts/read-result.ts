import { execFileSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { Address, Networks, rpc, scValToNative, xdr } from "@stellar/stellar-sdk";

const contract =
  "CBIQGYBF2LPQD4W7Z3DOUUY7RBPKP3OHUOLQ6MASI7GYTBOPQPV62T7X";
const rpcUrl = "https://soroban-testnet.stellar.org";
const dataDir = new URL("../zkvm/data/", import.meta.url);
const outputFile = new URL("assigned_reputation.json", dataDir);

type Member = { token_id: number; role: number };

const [source, ...extraArgs] = process.argv.slice(2);

if (!source || extraArgs.length > 0) {
  console.error(
    "Usage: node scripts/read-result.ts <account>"
  );
  process.exit(1);
}

function nextTokenId(): number {
  const output = execFileSync(
    "stellar",
    [
      "contract", "invoke",
      "--send=no",
      "--network", "testnet",
      "--rpc-url", rpcUrl,
      "--network-passphrase", Networks.TESTNET,
      "--source", source,
      "--id", contract,
      "--",
      "next_token_id",
    ],
    {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "inherit"],
    }
  );

  return JSON.parse(output);
}

function memberKey(tokenId: number): xdr.LedgerKey {
  return xdr.LedgerKey.contractData(new xdr.LedgerKeyContractData({
    contract: new Address(contract).toScAddress(),
    key: xdr.ScVal.scvVec([
      xdr.ScVal.scvSymbol("Member"),
      xdr.ScVal.scvU32(tokenId),
    ]),
    durability: xdr.ContractDataDurability.persistent,
  }));
}

try {
  const nextId = nextTokenId();

  const server = new rpc.Server(rpcUrl, { timeout: 30_000 });
  const members: Member[] = [];
  if (nextId > 0) {
    const keys: xdr.LedgerKey[] = [];
    for (let tokenId = 0; tokenId < nextId; tokenId++) {
      keys.push(memberKey(tokenId));
    }

    console.error(`Reading ${nextId} members...`);
    const response = await server.getLedgerEntries(...keys);
    const profiles = new Map<number, Member>();
    for (const entry of response.entries) {
      if (entry.val.type !== "contractData") {
        throw new Error("Unexpected ledger entry type");
      }
      const data = entry.val.contractData;
      const [, tokenId] = scValToNative(data.key) as [string, number];
      const member = scValToNative(data.val) as { role: number };
      profiles.set(tokenId, { ...member, token_id: tokenId });
    }
    for (let tokenId = 0; tokenId < nextId; tokenId++) {
      const member = profiles.get(tokenId);
      if (!member) {
        throw new Error(
          `Member ${tokenId} was not found in live persistent storage.`,
        );
      }
      members.push(member);
    }
  }

  const input = {
    users: members.map((member) => ({
      id: String(member.token_id),
      tier: member.role,
      discord_roles: [],
    })),
  };
  const json = JSON.stringify(input, null, 2);
  mkdirSync(dataDir, { recursive: true });
  writeFileSync(outputFile, `${json}\n`, "utf8");
  console.error(`Saved ${input.users.length} users to ${fileURLToPath(outputFile)}`);
} catch (error) {
  console.error(
    error instanceof Error
      ? error.message
      : typeof error === "object" && error !== null
        ? JSON.stringify(error)
        : String(error),
  );
  process.exit(1);
}
