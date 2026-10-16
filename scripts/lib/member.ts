import { Address, rpc, scValToNative, xdr } from "@stellar/stellar-sdk";
import { target, type FlowConfig } from "./config.ts";
import { read, type Target } from "./stellar.ts";

/** A minted membership token as read from chain. */
export type Member = { id: number; status: number; role: number };

/** Most ledger keys one `getLedgerEntries` request takes. */
const LEDGER_KEYS_PER_REQUEST = 200;

function memberKey(membership: string, tokenId: number): xdr.LedgerKey {
  return xdr.LedgerKey.contractData(new xdr.LedgerKeyContractData({
    contract: new Address(membership).toScAddress(),
    key: xdr.ScVal.scvVec([
      xdr.ScVal.scvSymbol("Member"),
      xdr.ScVal.scvU32(tokenId),
    ]),
    durability: xdr.ContractDataDurability.persistent,
  }));
}

/**
 * Every minted token, read straight from the membership contract's storage
 * (`MemberKey::Member(token_id)`), up to 200 per request.
 */
export async function readMembers(
  config: FlowConfig,
  membership: string,
): Promise<{ members: Member[]; ledger: number }> {
  const settings = read(target(config, membership), "settings") as { next_token_id: number };
  const count = settings?.next_token_id;
  if (!Number.isInteger(count)) {
    throw new Error("Membership settings returned an invalid next_token_id.");
  }
  const server = new rpc.Server(config.rpcUrl, {
    timeout: 30_000,
    // A local network serves plain HTTP.
    allowHttp: config.rpcUrl.startsWith("http://"),
  });
  const found = new Map<number, Member>();
  let ledger = 0;
  console.error(`Reading ${count} members of ${membership}...`);
  for (let start = 0; start < count; start += LEDGER_KEYS_PER_REQUEST) {
    const keys: xdr.LedgerKey[] = [];
    for (let id = start; id < Math.min(count, start + LEDGER_KEYS_PER_REQUEST); id++) {
      keys.push(memberKey(membership, id));
    }
    const response = await server.getLedgerEntries(...keys);
    ledger = response.latestLedger;
    for (const entry of response.entries) {
      if (entry.val.type !== "contractData") {
        throw new Error("Unexpected ledger entry type");
      }
      const data = entry.val.contractData;
      const [, id] = scValToNative(data.key) as [string, number];
      const { status, role } = scValToNative(data.val) as { status: number; role: number };
      found.set(id, { id, status, role });
    }
  }
  const members: Member[] = [];
  for (let id = 0; id < count; id++) {
    const member = found.get(id);
    if (!member) {
      throw new Error(`member ${id} is not in live storage (archived?)`);
    }
    members.push(member);
  }
  return { members, ledger };
}

/** Resolve a public key to a membership id; unlinked addresses return null. */
export function memberId(membership: Target, address: string): string | null {
  const tokenId = read(membership, "token_of", ["--owner", address]);
  if (tokenId === null) return null;
  if (!Number.isInteger(tokenId)) {
    throw new Error(`Invalid membership token for ${address}.`);
  }
  return String(tokenId);
}
