import { parseArgs } from "node:util";
import { loadConfig, target } from "./lib/config.ts";
import { isContractError, read, send, stellar } from "./lib/stellar.ts";
import { readSealedList, readTrustKey } from "./lib/trust-lists.ts";
import {
  memberKeyPair,
  openTrustList,
  sealTrustList,
  signatureBytes,
  TrustListError,
  trustMessage,
  type KeyWrap,
  type SealedTrustList,
} from "./lib/trust.ts";

const USAGE = `Usage:
  node scripts/trust-list.ts --member <token id> --as <identity> [--trust <ids>] [--config <flow.config.json>]

Reads or saves the trust list of a member, as the member: <identity> is the
stellar CLI identity holding the token. Without --trust, prints the saved
list. With --trust, saves the comma-separated token ids (--trust "" saves an
empty list), replacing the saved one. The key is derived from a signature of
the same message the Stroopies app signs, so the app opens these lists too.`;

/** `NotAMember` of the governance contract. */
const NOT_A_MEMBER = 18;
/** `NonExistentToken` of the membership contract. */
const NON_EXISTENT_TOKEN = 200;

const U32 = /^\d{1,10}$/;

function parseFlags() {
  try {
    const { values } = parseArgs({
      options: {
        member: { type: "string" },
        as: { type: "string" },
        trust: { type: "string" },
        config: { type: "string" },
      },
      strict: true,
    });
    if (!values.member || !U32.test(values.member)) throw new Error("--member must be a token id");
    if (!values.as) throw new Error("missing --as");
    const trust = values.trust?.split(",").map((id) => id.trim()).filter(Boolean);
    if (trust?.some((id) => !U32.test(id))) throw new Error("--trust must be comma-separated token ids");
    return { member: Number(values.member), as: values.as, trust: trust?.map(Number), config: values.config };
  } catch (error) {
    throw new Error(`${error instanceof Error ? error.message : error}\n${USAGE}`);
  }
}

const hex = (bytes: Uint8Array) => Buffer.from(bytes).toString("hex");
const wrapJson = (wrap: KeyWrap) => ({ key: hex(wrap.key), ephemeral: hex(wrap.ephemeral), sealed: hex(wrap.sealed) });

/** `TrustList` as the stellar CLI takes it: bytes in hex. */
function trustListJson(sealed: SealedTrustList): string {
  return JSON.stringify({ list: hex(sealed.list), member: wrapJson(sealed.member), operator: wrapJson(sealed.operator) });
}

async function main() {
  const flags = parseFlags();
  const config = loadConfig(flags.config);
  const governance = target(config, config.governance);
  const address = stellar(["keys", "address", flags.as]);

  const membership = String(read(governance, "get_membership_contract"));
  let owner: string;
  try {
    owner = String(read(target(config, membership), "owner_of", ["--token_id", String(flags.member)]));
  } catch (error) {
    if (isContractError(error, NON_EXISTENT_TOKEN)) throw new Error(`token ${flags.member} was never minted`);
    throw error;
  }
  if (owner !== address) {
    throw new Error(`token ${flags.member} is held by ${owner}, not by ${flags.as} (${address})`);
  }

  const message = trustMessage(config.networkPassphrase, flags.member, address);
  const key = await memberKeyPair(signatureBytes(stellar(["message", "sign", "--sign-with-key", flags.as, message])));
  const context = { networkPassphrase: config.networkPassphrase, contract: config.governance, memberId: flags.member };

  if (!flags.trust) {
    const sealed = await readSealedList(config, flags.member);
    if (!sealed) {
      console.error(`Member ${flags.member} has no trust list.`);
      return;
    }
    try {
      console.log(JSON.stringify(await openTrustList(sealed, context, key, "member")));
    } catch (error) {
      if (!(error instanceof TrustListError)) throw error;
      console.error(`The trust list of ${flags.member} does not open with this key: ${error.message}`);
    }
    return;
  }

  const sealed = await sealTrustList(flags.trust, context, key.publicKey, readTrustKey(config));
  try {
    send({ ...governance, source: flags.as }, "set_trust_list", [
      "--member_id", String(flags.member),
      "--trust_list", trustListJson(sealed),
    ]);
  } catch (error) {
    if (isContractError(error, NOT_A_MEMBER)) throw new Error(`member ${flags.member} is not an active member`);
    throw error;
  }
  console.error(`Saved the trust list of ${flags.member}: ${flags.trust.join(", ") || "nobody"}.`);
}

try {
  await main();
} catch (error) {
  console.error(error instanceof Error ? error.message : error);
  process.exit(1);
}
