import { readFileSync } from "node:fs";
import { scValToNative } from "@stellar/stellar-sdk";
import { target, type FlowConfig } from "./config.ts";
import { readEntries } from "./ledger.ts";
import { isContractError, read } from "./stellar.ts";
import {
  keyPair,
  openTrustList,
  TrustListError,
  type Bytes,
  type KeyWrap,
  type SealedTrustList,
  type TrustKeyPair,
} from "./trust.ts";

/** `TrustKeyNotSet` of the governance contract. */
const TRUST_KEY_NOT_SET = 22;

/** Lists this close to the end of their TTL are reported: 30 days of 5 s ledgers. */
const EXPIRY_WARNING_LEDGERS = (30 * 24 * 3600) / 5;

/** The operator key file `trust-key.ts` writes. */
export type TrustKeyFile = { publicKey: string; privateKey: string };

const hexBytes = (hex: string) => new Uint8Array(Buffer.from(hex, "hex")) as Bytes;
const hex = (bytes: Uint8Array) => Buffer.from(bytes).toString("hex");
const copy = (bytes: Buffer) => new Uint8Array(bytes) as Bytes;

type NativeWrap = Record<keyof KeyWrap, Buffer>;

/** `TrustList` of the governance contract, as `scValToNative` reads it. */
function sealedList(native: { list: Buffer; member: NativeWrap; operator: NativeWrap }): SealedTrustList {
  const wrap = (w: NativeWrap) => ({ key: copy(w.key), ephemeral: copy(w.ephemeral), sealed: copy(w.sealed) });
  return { list: copy(native.list), member: wrap(native.member), operator: wrap(native.operator) };
}

/**
 * The operator key from its file, checked against the key the governance
 * contract gives the app: lists sealed for another key would not open.
 */
export async function loadOperatorKey(config: FlowConfig): Promise<TrustKeyPair> {
  const path = config.trustKeyFile;
  if (!path) {
    throw new Error(
      "the config has no trustKeyFile: reading trust lists needs the operator key (node scripts/trust-key.ts)",
    );
  }
  let file: TrustKeyFile;
  try {
    file = JSON.parse(readFileSync(path, "utf8"));
  } catch (error) {
    throw new Error(
      `cannot read the operator key ${path}: ${error instanceof Error ? error.message : error}\n` +
        "Generate one with node scripts/trust-key.ts.",
    );
  }
  const operator = await keyPair(hexBytes(file.privateKey));
  if (hex(operator.publicKey) !== file.publicKey) {
    throw new Error(`${path}: the public key does not match the private key`);
  }
  const onChain = readTrustKey(config, `set it with set_trust_key --key ${file.publicKey}`);
  if (hex(onChain) !== file.publicKey) {
    throw new Error(
      `the governance contract encrypts trust lists to ${hex(onChain)}, not to the key in ${path}`,
    );
  }
  return operator;
}

/** The operator key the governance contract holds, which every list is sealed for. */
export function readTrustKey(config: FlowConfig, hint = "its admin sets it with set_trust_key"): Bytes {
  try {
    return hexBytes(String(read(target(config, config.governance), "get_trust_key")));
  } catch (error) {
    if (isContractError(error, TRUST_KEY_NOT_SET)) {
      throw new Error(`the governance contract has no trust key: ${hint}`);
    }
    throw error;
  }
}

/** One member's stored list, still sealed, or null if they saved none. */
export async function readSealedList(config: FlowConfig, memberId: number): Promise<SealedTrustList | null> {
  const { entries } = await readEntries(config, config.governance, "TrustList", [memberId]);
  const entry = entries.get(memberId);
  return entry ? sealedList(scValToNative(entry)) : null;
}

/**
 * The trust lists of `memberIds`, decrypted with the operator key. Members
 * who saved none are absent. A list that does not open is left out and
 * reported, so one bad list does not stop the round. `expiring` names the
 * lists whose TTL ends within 30 days or has ended.
 */
export async function readTrustLists(
  config: FlowConfig,
  operator: TrustKeyPair,
  memberIds: number[],
): Promise<{
  lists: Map<number, number[]>;
  unreadable: Array<{ id: number; reason: string }>;
  expiring: Array<{ id: number; liveUntil: number }>;
  ledger: number;
}> {
  const { entries, liveUntil, ledger } = await readEntries(config, config.governance, "TrustList", memberIds);
  const lists = new Map<number, number[]>();
  const unreadable: Array<{ id: number; reason: string }> = [];
  for (const [id, entry] of entries) {
    const context = {
      networkPassphrase: config.networkPassphrase,
      contract: config.governance,
      memberId: id,
    };
    try {
      lists.set(id, await openTrustList(sealedList(scValToNative(entry)), context, operator, "operator"));
    } catch (error) {
      if (!(error instanceof TrustListError)) throw error;
      unreadable.push({ id, reason: error.message });
    }
  }
  const expiring = [...liveUntil]
    .filter(([, until]) => until - ledger < EXPIRY_WARNING_LEDGERS)
    .map(([id, until]) => ({ id, liveUntil: until }));
  return { lists, unreadable, expiring, ledger };
}
