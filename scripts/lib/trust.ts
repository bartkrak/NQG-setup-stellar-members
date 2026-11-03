/**
 * Trust lists: who a member trusts, as token ids, stored encrypted in the
 * NQG contract. A fresh data key encrypts the list, and is itself sealed
 * twice: for the member, with a key derived from a wallet signature, and for
 * the operator, whose public key the NQG contract holds. Only the WebCrypto
 * API is used, so the app and Node read the same bytes; the NQG repository
 * keeps a copy of this file and of `trust.vectors.json`.
 *
 * The NQG contract stores a `TrustList { list, member, operator }`:
 *
 * | Field      | Bytes        | What                                                     |
 * | ---------- | ------------ | -------------------------------------------------------- |
 * | `list`     | 1 + 12 + 273 | version 1, nonce, AES-256-GCM of the ids, tag included   |
 * | `member`   | 32 + 32 + 48 | `key`, `ephemeral`, `sealed`: the member's copy of the key |
 * | `operator` | 32 + 32 + 48 | the same for the operator key                            |
 *
 * A copy is X25519 between a one-time key, `ephemeral`, and the recipient's
 * `key`, HKDF-SHA256 of the shared secret (salt: `ephemeral` then `key`;
 * info: the role and the context) and AES-256-GCM of the data key under a
 * zero nonce, each derived key sealing once. The ids are a count byte and 64
 * big-endian `u32` slots, sorted ids first and zeros after, so a list says
 * nothing of how many members it holds. Each part authenticates the context,
 * network, NQG contract and token id, so a list opens only where it was
 * sealed; the list does not cover the copies, so one copy can be replaced
 * without the other.
 */

export const TRUST_VERSION = 1;
/** Most members one trust list holds. */
export const MAX_TRUSTED = 64;

const KEY = 32;
const NONCE = 12;
const IDS = 1 + 4 * MAX_TRUSTED;
const NONCE_AT = 1;
const CIPHERTEXT_AT = NONCE_AT + NONCE;
/** Bytes of `list`, version 1. */
export const LIST_SIZE = CIPHERTEXT_AT + IDS + 16;

const DOMAIN = "stroopies-trust-v1";
/** PKCS #8 prefix of a raw X25519 private key (RFC 8410). */
const PKCS8_X25519 = [
  0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x6e, 0x04,
  0x22, 0x04, 0x20,
];

export type Bytes = Uint8Array<ArrayBuffer>;
type Role = "member" | "operator";

/** One recipient's copy of the data key, `KeyWrap` in the NQG contract. */
export interface KeyWrap {
  /** The recipient's X25519 public key. */
  key: Bytes;
  /** The one-time X25519 public key the copy was sealed with. */
  ephemeral: Bytes;
  /** The data key, sealed. */
  sealed: Bytes;
}

/** A sealed trust list, `TrustList` in the NQG contract. */
export interface SealedTrustList {
  list: Bytes;
  member: KeyWrap;
  operator: KeyWrap;
}

/** Where a list belongs: it opens nowhere else. */
export interface TrustContext {
  networkPassphrase: string;
  /** The NQG contract holding the list. */
  contract: string;
  memberId: number;
}

/** An X25519 key pair: the private half for WebCrypto, the public as bytes. */
export interface TrustKeyPair {
  privateKey: CryptoKey;
  publicKey: Bytes;
}

export type TrustFailure =
  "signature" | "member-key" | "operator-key" | "decrypt" | "content";

export class TrustListError extends Error {
  readonly reason: TrustFailure;

  constructor(reason: TrustFailure, message: string) {
    super(message);
    this.name = "TrustListError";
    this.reason = reason;
  }
}

/**
 * The text a member signs to unlock the list. A wallet signs it with the
 * account's Ed25519 key, which gives the same signature every time, so the
 * key derived from it needs no storage.
 */
export function trustMessage(
  networkPassphrase: string,
  memberId: number,
  account: string,
): string {
  return [
    "Stroopies trust list",
    `Network: ${networkPassphrase}`,
    `Member: #${memberId}`,
    `Account: ${account}`,
    "Signing this unlocks your trust list. Only sign it in the Stroopies app.",
  ].join("\n");
}

/** The 64 bytes of an Ed25519 signature as wallets return it: base64 or hex. */
export function signatureBytes(signed: string): Bytes {
  const hex = /^[0-9a-f]{128}$/i.test(signed);
  const base64 = /^[A-Za-z0-9+/_-]{86}={0,2}$/.test(signed);
  if (!hex && !base64) {
    throw new TrustListError(
      "signature",
      "The wallet did not return an Ed25519 signature.",
    );
  }
  return hex ? hexBytes(signed) : base64Bytes(signed);
}

/** The member's key pair, from the signature of `trustMessage`. */
export async function memberKeyPair(signature: Bytes): Promise<TrustKeyPair> {
  return keyPair(await hkdf(signature, encode(DOMAIN), encode("member key")));
}

/** The X25519 key pair of a 32-byte private key. */
export async function keyPair(privateKey: Bytes): Promise<TrustKeyPair> {
  const key = await crypto.subtle.importKey(
    "pkcs8",
    concat(Uint8Array.from(PKCS8_X25519), privateKey),
    { name: "X25519" },
    true,
    ["deriveBits"],
  );
  const { x } = await crypto.subtle.exportKey("jwk", key);
  return { privateKey: key, publicKey: base64Bytes(x!) };
}

/** A new random X25519 private key, as for the operator. */
export function newPrivateKey(): Bytes {
  return randomBytes(KEY);
}

/**
 * Encrypt a trust list for the member and the operator. `random` gives the
 * data key, the two one-time keys and the nonce, in this order; tests pass
 * fixed bytes.
 */
export async function sealTrustList(
  ids: readonly number[],
  context: TrustContext,
  memberKey: Bytes,
  operatorKey: Bytes,
  random: (length: number) => Bytes = randomBytes,
): Promise<SealedTrustList> {
  const plain = encodeIds(ids, context.memberId);
  const info = contextBytes(context);
  const dataKey = random(KEY);
  const member = await wrap(dataKey, memberKey, random(KEY), "member", info);
  const operator = await wrap(
    dataKey,
    operatorKey,
    random(KEY),
    "operator",
    info,
  );
  const nonce = random(NONCE);
  const version = Uint8Array.of(TRUST_VERSION);
  const ciphertext = await crypto.subtle.encrypt(
    { name: "AES-GCM", iv: nonce, additionalData: concat(version, info) },
    await aesKey(dataKey),
    plain,
  );
  return {
    list: concat(version, nonce, new Uint8Array(ciphertext)),
    member,
    operator,
  };
}

/**
 * Decrypt a trust list as its member or as the operator. Fails with a
 * `TrustListError` saying why: a list sealed for another key, for another
 * member or contract, or altered.
 */
export async function openTrustList(
  sealed: SealedTrustList,
  context: TrustContext,
  reader: TrustKeyPair,
  role: Role,
): Promise<number[]> {
  const copy = sealed[role];
  if (!equal(copy.key, reader.publicKey)) {
    throw new TrustListError(
      role === "member" ? "member-key" : "operator-key",
      role === "member"
        ? "The trust list was saved with another key."
        : "The trust list was sealed for another operator key.",
    );
  }
  const { list } = sealed;
  if (list.length !== LIST_SIZE || list[0] !== TRUST_VERSION) {
    throw new TrustListError(
      "content",
      "The trust list is not a version 1 list.",
    );
  }
  const info = contextBytes(context);
  let plain: Bytes;
  try {
    const dataKey = await unwrap(copy, reader, role, info);
    plain = new Uint8Array(
      await crypto.subtle.decrypt(
        {
          name: "AES-GCM",
          iv: list.slice(NONCE_AT, CIPHERTEXT_AT),
          additionalData: concat(list.slice(0, NONCE_AT), info),
        },
        await aesKey(dataKey),
        list.slice(CIPHERTEXT_AT),
      ),
    );
  } catch {
    throw new TrustListError(
      "decrypt",
      "The trust list does not open here: it belongs to another member or contract, or was altered.",
    );
  }
  return decodeIds(plain, context.memberId);
}

/** Sorted, unique ids other than the member's own, at most `MAX_TRUSTED`. */
function encodeIds(ids: readonly number[], memberId: number): Bytes {
  for (const id of ids) {
    if (!Number.isInteger(id) || id < 0 || id > 0xffffffff) {
      throw new TrustListError("content", `${id} is not a token id.`);
    }
    if (id === memberId) {
      throw new TrustListError("content", "A member cannot trust themselves.");
    }
  }
  const sorted = [...new Set(ids)].sort((a, b) => a - b);
  if (sorted.length > MAX_TRUSTED) {
    throw new TrustListError(
      "content",
      `A trust list holds at most ${MAX_TRUSTED} members.`,
    );
  }
  const plain = new Uint8Array(IDS);
  const view = new DataView(plain.buffer);
  plain[0] = sorted.length;
  sorted.forEach((id, i) => view.setUint32(1 + 4 * i, id));
  return plain;
}

function decodeIds(plain: Bytes, memberId: number): number[] {
  const count = plain[0] ?? 0;
  const view = new DataView(plain.buffer, plain.byteOffset, plain.byteLength);
  const ids = Array.from({ length: Math.min(count, MAX_TRUSTED) }, (_, i) =>
    view.getUint32(1 + 4 * i),
  );
  const valid =
    plain.length === IDS &&
    count <= MAX_TRUSTED &&
    ids.every((id, i) => id !== memberId && (i === 0 || id > ids[i - 1]!)) &&
    plain.slice(1 + 4 * count).every((byte) => byte === 0);
  if (!valid) {
    throw new TrustListError("content", "The trust list is malformed.");
  }
  return ids;
}

async function wrap(
  dataKey: Bytes,
  recipient: Bytes,
  ephemeralKey: Bytes,
  role: Role,
  info: Bytes,
): Promise<KeyWrap> {
  const ephemeral = await keyPair(ephemeralKey);
  const key = await wrapKey(
    ephemeral.privateKey,
    recipient,
    concat(ephemeral.publicKey, recipient),
    role,
    info,
  );
  const sealed = await crypto.subtle.encrypt(
    { name: "AES-GCM", iv: new Uint8Array(NONCE), additionalData: info },
    key,
    dataKey,
  );
  return {
    key: recipient.slice(),
    ephemeral: ephemeral.publicKey,
    sealed: new Uint8Array(sealed),
  };
}

async function unwrap(
  copy: KeyWrap,
  reader: TrustKeyPair,
  role: Role,
  info: Bytes,
): Promise<Bytes> {
  const key = await wrapKey(
    reader.privateKey,
    copy.ephemeral,
    concat(copy.ephemeral, reader.publicKey),
    role,
    info,
  );
  return new Uint8Array(
    await crypto.subtle.decrypt(
      { name: "AES-GCM", iv: new Uint8Array(NONCE), additionalData: info },
      key,
      copy.sealed,
    ),
  );
}

/** The key sealing a data key, from the X25519 secret of `privateKey` and `peer`. */
async function wrapKey(
  privateKey: CryptoKey,
  peer: Bytes,
  salt: Bytes,
  role: Role,
  info: Bytes,
): Promise<CryptoKey> {
  const shared = await crypto.subtle.deriveBits(
    {
      name: "X25519",
      public: await crypto.subtle.importKey(
        "raw",
        peer,
        { name: "X25519" },
        false,
        [],
      ),
    },
    privateKey,
    256,
  );
  return aesKey(
    await hkdf(
      new Uint8Array(shared),
      salt,
      concat(encode(`${role} wrap\n`), info),
    ),
  );
}

function contextBytes({
  networkPassphrase,
  contract,
  memberId,
}: TrustContext): Bytes {
  return encode(`${DOMAIN}\n${networkPassphrase}\n${contract}\n${memberId}`);
}

async function hkdf(ikm: Bytes, salt: Bytes, info: Bytes): Promise<Bytes> {
  const key = await crypto.subtle.importKey("raw", ikm, "HKDF", false, [
    "deriveBits",
  ]);
  return new Uint8Array(
    await crypto.subtle.deriveBits(
      { name: "HKDF", hash: "SHA-256", salt, info },
      key,
      8 * KEY,
    ),
  );
}

function aesKey(raw: Bytes): Promise<CryptoKey> {
  return crypto.subtle.importKey("raw", raw, "AES-GCM", false, [
    "encrypt",
    "decrypt",
  ]);
}

function randomBytes(length: number): Bytes {
  return crypto.getRandomValues(new Uint8Array(length));
}

function encode(text: string): Bytes {
  return new TextEncoder().encode(text) as Bytes;
}

function concat(...parts: Bytes[]): Bytes {
  const out = new Uint8Array(parts.reduce((n, part) => n + part.length, 0));
  let at = 0;
  for (const part of parts) {
    out.set(part, at);
    at += part.length;
  }
  return out;
}

function equal(a: Bytes, b: Bytes): boolean {
  return a.length === b.length && a.every((byte, i) => byte === b[i]);
}

function hexBytes(hex: string): Bytes {
  return Uint8Array.from(hex.match(/../g) ?? [], (pair) => parseInt(pair, 16));
}

/** Standard or url-safe base64, padded or not. */
function base64Bytes(text: string): Bytes {
  const binary = atob(
    text
      .replace(/-/g, "+")
      .replace(/_/g, "/")
      .padEnd(Math.ceil(text.length / 4) * 4, "="),
  );
  return Uint8Array.from(binary, (char) => char.charCodeAt(0));
}
