import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import {
  keyPair,
  memberKeyPair,
  newPrivateKey,
  openTrustList,
  signatureBytes,
  type Bytes,
  type KeyWrap,
  type SealedTrustList,
  type TrustContext,
} from "./trust.ts";

type HexWrap = Record<keyof KeyWrap, string>;

/** Lists sealed by the Stroopies app (`shared/trust.vectors.json` there). */
const fixture = JSON.parse(
  readFileSync(new URL("trust.vectors.json", import.meta.url), "utf8"),
) as {
  operatorPrivateKey: string;
  vectors: Array<
    TrustContext & { signature: string; ids: number[]; list: string; member: HexWrap; operator: HexWrap }
  >;
};

const bytes = (hex: string) => new Uint8Array(Buffer.from(hex, "hex")) as Bytes;
const wrap = (w: HexWrap): KeyWrap => ({
  key: bytes(w.key),
  ephemeral: bytes(w.ephemeral),
  sealed: bytes(w.sealed),
});
const sealedOf = (v: (typeof fixture.vectors)[number]): SealedTrustList => ({
  list: bytes(v.list),
  member: wrap(v.member),
  operator: wrap(v.operator),
});

test("the operator opens the lists the app seals", async () => {
  const operator = await keyPair(bytes(fixture.operatorPrivateKey));
  for (const vector of fixture.vectors) {
    assert.deepEqual(await openTrustList(sealedOf(vector), vector, operator, "operator"), vector.ids);
  }
});

test("a list opens only with the operator key and where it was saved", async () => {
  const vector = fixture.vectors[0]!;
  const sealed = sealedOf(vector);
  const operator = await keyPair(bytes(fixture.operatorPrivateKey));
  const member = await memberKeyPair(signatureBytes(vector.signature));
  const reason = (promise: Promise<unknown>) =>
    promise.then(
      () => "opened",
      (error: { reason: string }) => error.reason,
    );

  assert.deepEqual(await openTrustList(sealed, vector, member, "member"), vector.ids);
  assert.equal(
    await reason(openTrustList(sealed, vector, await keyPair(newPrivateKey()), "operator")),
    "operator-key",
  );
  assert.equal(
    await reason(openTrustList(sealed, { ...vector, memberId: vector.memberId + 1 }, operator, "operator")),
    "decrypt",
  );
});
