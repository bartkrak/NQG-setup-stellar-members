import { Address, rpc, scValToNative, xdr } from "@stellar/stellar-sdk";
import type { FlowConfig } from "./config.ts";

/** Most ledger keys one `getLedgerEntries` request takes. */
const LEDGER_KEYS_PER_REQUEST = 200;

/**
 * The persistent entries `variant(id)` of a contract, read straight from its
 * storage, up to 200 per request: one simulation per id would take minutes.
 * Ids without a live entry are missing from the map. `liveUntil` is the last
 * ledger of each entry's TTL. Couples the caller to the contract's storage
 * layout.
 */
export async function readEntries(
  config: FlowConfig,
  contract: string,
  variant: string,
  ids: number[],
): Promise<{ entries: Map<number, xdr.ScVal>; liveUntil: Map<number, number>; ledger: number }> {
  const server = new rpc.Server(config.rpcUrl, {
    timeout: 30_000,
    // A local network serves plain HTTP.
    allowHttp: config.rpcUrl.startsWith("http://"),
  });
  const entries = new Map<number, xdr.ScVal>();
  const liveUntil = new Map<number, number>();
  let ledger = 0;
  for (let start = 0; start < ids.length; start += LEDGER_KEYS_PER_REQUEST) {
    const keys = ids
      .slice(start, start + LEDGER_KEYS_PER_REQUEST)
      .map((id) => entryKey(contract, variant, id));
    const response = await server.getLedgerEntries(...keys);
    ledger = response.latestLedger;
    for (const entry of response.entries) {
      if (entry.val.type !== "contractData") {
        throw new Error("Unexpected ledger entry type");
      }
      const data = entry.val.contractData;
      const [, id] = scValToNative(data.key) as [string, number];
      entries.set(id, data.val);
      if (entry.liveUntilLedgerSeq !== undefined) liveUntil.set(id, entry.liveUntilLedgerSeq);
    }
  }
  return { entries, liveUntil, ledger };
}

function entryKey(contract: string, variant: string, id: number): xdr.LedgerKey {
  return xdr.LedgerKey.contractData(new xdr.LedgerKeyContractData({
    contract: new Address(contract).toScAddress(),
    key: xdr.ScVal.scvVec([
      xdr.ScVal.scvSymbol(variant),
      xdr.ScVal.scvU32(id),
    ]),
    durability: xdr.ContractDataDurability.persistent,
  }));
}
