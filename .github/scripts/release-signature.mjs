/**
 * Minisign signature verification for Tauri updater artifacts.
 *
 * Tauri stores the updater public key as base64 of the whole minisign.pub
 * file, and each `latest.json` platform entry carries base64 of the
 * corresponding `.sig` file. This module verifies an artifact against that
 * pair using Node's built-in Ed25519 + BLAKE2b (OpenSSL 3, Node 18+).
 *
 * Why: v0.73.0 shipped a corrupted mirror artifact because nightly and stable
 * upload jobs wrote the same FTP path concurrently. Uploading is now gated by
 * this cryptographic check and followed by a mirror re-verification.
 */
import {
  createHash,
  createPublicKey,
  verify as cryptoVerify,
} from "node:crypto";
import { createReadStream } from "node:fs";

// DER prefix for a raw 32-byte Ed25519 public key (SubjectPublicKeyInfo).
const ED25519_SPKI_PREFIX = Buffer.from("302a300506032b6570032100", "hex");

export function decodePublicKey(publicKeyBase64) {
  const text = Buffer.from(publicKeyBase64, "base64").toString("utf8");
  const keyLine = text
    .split(/\r?\n/)
    .map((line) => line.trim())
    .find((line) => line.startsWith("RW"));
  if (!keyLine) {
    throw new Error("updater public key does not contain a minisign key line");
  }
  const raw = Buffer.from(keyLine, "base64");
  if (raw.length !== 42) {
    throw new Error(`updater public key has ${raw.length} bytes, expected 42`);
  }
  return {
    algorithm: raw.subarray(0, 2),
    keyId: raw.subarray(2, 10),
    keyObject: createPublicKey({
      key: Buffer.concat([ED25519_SPKI_PREFIX, raw.subarray(10, 42)]),
      format: "der",
      type: "spki",
    }),
  };
}

export function parseSignature(signatureBase64) {
  const text = Buffer.from(signatureBase64, "base64").toString("utf8");
  const lines = text.split(/\r?\n/);
  if (lines.length < 4) {
    throw new Error("minisign signature must contain four lines");
  }
  const bin1 = Buffer.from(lines[1], "base64");
  const trustedLine = lines[2];
  const bin2 = Buffer.from(lines[3], "base64");
  if (bin1.length !== 74 || bin2.length !== 64) {
    throw new Error("minisign signature has invalid length");
  }
  if (!trustedLine.startsWith("trusted comment: ")) {
    throw new Error("minisign signature is missing its trusted comment");
  }
  const algorithm = bin1.subarray(0, 2);
  return {
    keyId: bin1.subarray(2, 10),
    signature: bin1.subarray(10, 74),
    trustedComment: trustedLine.slice("trusted comment: ".length),
    globalSignature: bin2,
    prehashed: algorithm[0] === 0x45 && algorithm[1] === 0x44,
  };
}

function verifyDigest(digest, parsed, publicKey) {
  if (!publicKey.keyId.equals(parsed.keyId)) {
    throw new Error("minisign key id does not match the updater public key");
  }
  if (!parsed.prehashed) {
    throw new Error("legacy non-prehashed minisign signatures are unsupported");
  }
  const globalMessage = Buffer.concat([
    parsed.signature,
    Buffer.from(parsed.trustedComment),
  ]);
  if (!cryptoVerify(null, globalMessage, publicKey.keyObject, parsed.globalSignature)) {
    throw new Error("minisign global signature is invalid");
  }
  if (!cryptoVerify(null, digest, publicKey.keyObject, parsed.signature)) {
    throw new Error("artifact signature is invalid");
  }
}

/**
 * Verify a readable stream against a Tauri latest.json signature.
 * Returns the minisign trusted comment on success.
 */
export async function verifyMinisignStream(
  stream,
  signatureBase64,
  publicKeyBase64,
) {
  const publicKey = decodePublicKey(publicKeyBase64);
  const parsed = parseSignature(signatureBase64);
  if (!parsed.prehashed) {
    throw new Error("legacy non-prehashed minisign signatures are unsupported");
  }
  const hash = createHash("blake2b512");
  for await (const chunk of stream) {
    hash.update(chunk);
  }
  verifyDigest(hash.digest(), parsed, publicKey);
  return parsed.trustedComment;
}

export async function verifyMinisignFile(
  filePath,
  signatureBase64,
  publicKeyBase64,
) {
  return verifyMinisignStream(
    createReadStream(filePath),
    signatureBase64,
    publicKeyBase64,
  );
}
