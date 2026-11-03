import { Address, scValToNative, xdr } from "@stellar/stellar-sdk";
import { target, type FlowConfig } from "./config.ts";
import { readEntries } from "./ledger.ts";
import { read, type Target } from "./stellar.ts";

/** A minted membership token as read from chain. */
export type Member = { id: number; revoked: boolean; role: number };

type MembershipEvent = { topicsXdr: string[]; bodyXdr: string; paging_token: string };

/**
 * Every minted token, read straight from the membership contract's storage
 * (`MemberKey::Member(token_id)`).
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
  console.error(`Reading ${count} members of ${membership}...`);
  const ids = Array.from({ length: count }, (_, id) => id);
  const { entries, ledger } = await readEntries(config, membership, "Member", ids);
  const members = ids.map((id) => {
    const entry = entries.get(id);
    if (!entry) {
      throw new Error(`member ${id} is not in live storage (archived?)`);
    }
    const { revoked, role } = scValToNative(entry) as { revoked: boolean; role: number };
    return { id, revoked, role };
  });
  return { members, ledger };
}

function recordAddress(memberIdByAddress: Map<string, string>, address: string, id: string): void {
  const previousId = memberIdByAddress.get(address);
  if (previousId !== undefined && previousId !== id) {
    throw new Error(
      `Voting address ${address} is linked to multiple membership ids: ${previousId} and ${id}. ` +
      "Historical voting addresses must belong to one member throughout their history.",
    );
  }
  memberIdByAddress.set(address, id);
}

/** Transfers cover every address change. */
async function readTransferEvents(
  membershipContract: string,
  explorerUrl: string,
): Promise<MembershipEvent[]> {
  const url = new URL(`${explorerUrl}/contract/${membershipContract}/events`);
  url.search = new URLSearchParams({ topic: "transfer", order: "asc", limit: "200" }).toString();
  const seenCursors = new Set<string>();
  const allEvents: MembershipEvent[] = [];

  while (true) {
    const response = await fetch(url, { signal: AbortSignal.timeout(30_000) });
    if (!response.ok) throw new Error(`Cannot read membership transfer events: HTTP ${response.status}`);
    const events = (await response.json())?._embedded?.records;
    if (!Array.isArray(events)) throw new Error("Invalid membership transfer event response.");
    if (!events.length) return allEvents;

    const cursor = events.at(-1)?.paging_token;
    if (typeof cursor !== "string" || !cursor || seenCursors.has(cursor)) {
      throw new Error("Membership transfer history pagination did not advance.");
    }
    seenCursors.add(cursor);
    allEvents.push(...events);
    url.searchParams.set("cursor", cursor);
  }
}

/** Transfer topics contain the previous and next addresses; the body is the token id. */
function decodeTransferEvent(event: MembershipEvent) {
  try {
    if (!Array.isArray(event.topicsXdr) || event.topicsXdr.length !== 3) {
      throw new Error("Unexpected event topics.");
    }
    const [name, from, to] = event.topicsXdr.map(value => xdr.ScVal.fromXdr(value, "base64"));
    const tokenId = xdr.ScVal.fromXdr(event.bodyXdr, "base64");
    if (scValToNative(name) !== "transfer" || tokenId.type !== "scvU32") {
      throw new Error("Unexpected event name or token id.");
    }
    return {
      id: String(tokenId.u32),
      from: Address.fromScVal(from).toString(),
      to: Address.fromScVal(to).toString(),
    };
  } catch (error) {
    throw new Error(`Invalid membership transfer event ${event?.paging_token ?? ""}.`, { cause: error });
  }
}

/**
 * Historical addresses from transfers, keyed by address for vote lookup.
 * Addresses must never be reused by a different member.
 */
export async function readMemberAddresses(
  membershipContract: string,
  explorerUrl: string,
): Promise<Map<string, string>> {
  const memberIdByAddress = new Map<string, string>();
  const events = await readTransferEvents(membershipContract, explorerUrl);
  for (const event of events) {
    const { id, from, to } = decodeTransferEvent(event);
    recordAddress(memberIdByAddress, from, id);
    recordAddress(memberIdByAddress, to, id);
  }
  return memberIdByAddress;
}

/** Resolve past and current addresses, rejecting conflicting member identities. */
export function resolveMemberId(
  membership: Target,
  address: string,
  memberIdByAddress: Map<string, string>,
): string | null {
  const tokenId = read(membership, "token_of", ["--owner", address]);
  if (tokenId !== null) {
    if (typeof tokenId !== "number" || !Number.isInteger(tokenId)) {
      throw new Error(`Invalid membership token for ${address}.`);
    }
    recordAddress(memberIdByAddress, address, String(tokenId));
  }
  return memberIdByAddress.get(address) ?? null;
}
