import { createHash, createPublicKey, verify as verifyEd25519 } from "node:crypto";
import { createReadStream } from "node:fs";
import { readFile } from "node:fs/promises";

const ed25519SpkiPrefix = Buffer.from("302a300506032b6570032100", "hex");
const placeholderPattern = /placeholder|replace_before_release|replace-before-release|changeme|\btodo\b|\btbd\b/i;
const base64Pattern = /^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/;

class UpdaterSignatureError extends Error {
  constructor(message, options = {}) {
    super(message, options);
    this.name = "UpdaterSignatureError";
  }
}

function strictBase64ToBuffer(value, label) {
  const text = String(value ?? "").trim();
  if (!text || text.length % 4 !== 0 || !base64Pattern.test(text)) {
    throw new UpdaterSignatureError(`${label} is not valid standard base64`);
  }

  return Buffer.from(text, "base64");
}

function strictBase64ToUtf8(value, label) {
  const buffer = strictBase64ToBuffer(value, label);
  const text = buffer.toString("utf8");
  if (!Buffer.from(text, "utf8").equals(buffer)) {
    throw new UpdaterSignatureError(`${label} does not decode to UTF-8 text`);
  }
  return text;
}

function assertNonPlaceholderPublicKey(value, label) {
  const text = String(value ?? "").trim();
  if (!text || text.length < 32 || placeholderPattern.test(text)) {
    throw new UpdaterSignatureError(`${label} must be the approved non-placeholder Tauri updater public key`);
  }
  return text;
}

/**
 * The single updater public key resolver. The overlay generator, readiness and
 * the updater metadata command all resolve the key through here so the accepted
 * variable names match what the release docs promise: either
 * VOYAVPN_UPDATER_PUBLIC_KEY or TAURI_UPDATER_PUBLIC_KEY, and when both are set
 * they must be the same key.
 */
function approvedUpdaterPublicKeyFromEnv(env = process.env) {
  const primary = String(env.VOYAVPN_UPDATER_PUBLIC_KEY ?? "").trim();
  const secondary = String(env.TAURI_UPDATER_PUBLIC_KEY ?? "").trim();

  if (!primary && !secondary) {
    throw new UpdaterSignatureError(
      "VOYAVPN_UPDATER_PUBLIC_KEY or TAURI_UPDATER_PUBLIC_KEY must be the approved non-placeholder Tauri updater public key",
    );
  }

  const approved = primary
    ? assertNonPlaceholderPublicKey(primary, "VOYAVPN_UPDATER_PUBLIC_KEY")
    : assertNonPlaceholderPublicKey(secondary, "TAURI_UPDATER_PUBLIC_KEY");

  if (primary && secondary && assertNonPlaceholderPublicKey(secondary, "TAURI_UPDATER_PUBLIC_KEY") !== approved) {
    throw new UpdaterSignatureError(
      "TAURI_UPDATER_PUBLIC_KEY must exactly match the approved VOYAVPN_UPDATER_PUBLIC_KEY",
    );
  }

  return approved;
}

function resolveApprovedUpdaterPublicKey(env = process.env) {
  const approved = approvedUpdaterPublicKeyFromEnv(env);
  decodeTauriUpdaterPublicKey(approved, "approved updater public key");
  return approved;
}

function parseMinisignPacketAlgorithm(bytes, label) {
  if (bytes.length < 2) {
    throw new UpdaterSignatureError(`${label} is too short to contain a minisign algorithm`);
  }
  const algorithm = bytes.subarray(0, 2).toString("latin1");
  if (algorithm !== "Ed" && algorithm !== "ED") {
    throw new UpdaterSignatureError(`${label} uses unsupported minisign algorithm ${JSON.stringify(algorithm)}`);
  }
  return {
    algorithm,
    isPrehashed: algorithm === "ED",
  };
}

function decodeTauriUpdaterPublicKey(publicKeyBase64, label = "updater public key") {
  const minisignPublicKey = strictBase64ToUtf8(publicKeyBase64, label);
  const lines = minisignPublicKey.trimEnd().split(/\r?\n/);
  if (lines.length !== 2 || !lines[0].startsWith("untrusted comment: ")) {
    throw new UpdaterSignatureError(`${label} must decode to a two-line minisign public key`);
  }

  const publicKeyPacket = strictBase64ToBuffer(lines[1], `${label} minisign key packet`);
  if (publicKeyPacket.length !== 42) {
    throw new UpdaterSignatureError(`${label} minisign key packet must be 42 bytes`);
  }

  const { algorithm } = parseMinisignPacketAlgorithm(publicKeyPacket, `${label} minisign key packet`);
  return {
    algorithm,
    keyId: publicKeyPacket.subarray(2, 10),
    key: publicKeyPacket.subarray(10, 42),
    minisignPublicKey,
    untrustedComment: lines[0],
  };
}

function decodeTauriUpdaterSignature(signatureBase64, label = "updater signature") {
  if (placeholderPattern.test(String(signatureBase64 ?? ""))) {
    throw new UpdaterSignatureError(`${label} is a placeholder`);
  }

  const minisignSignature = strictBase64ToUtf8(signatureBase64, label);
  const lines = minisignSignature.trimEnd().split(/\r?\n/);
  if (
    lines.length !== 4 ||
    !lines[0].startsWith("untrusted comment: ") ||
    !lines[2].startsWith("trusted comment: ")
  ) {
    throw new UpdaterSignatureError(`${label} must decode to a four-line minisign signature`);
  }

  const signaturePacket = strictBase64ToBuffer(lines[1], `${label} signature packet`);
  if (signaturePacket.length !== 74) {
    throw new UpdaterSignatureError(`${label} signature packet must be 74 bytes`);
  }
  const globalSignature = strictBase64ToBuffer(lines[3], `${label} global signature`);
  if (globalSignature.length !== 64) {
    throw new UpdaterSignatureError(`${label} global signature must be 64 bytes`);
  }

  const { algorithm, isPrehashed } = parseMinisignPacketAlgorithm(signaturePacket, `${label} signature packet`);
  return {
    algorithm,
    isPrehashed,
    keyId: signaturePacket.subarray(2, 10),
    signature: signaturePacket.subarray(10, 74),
    trustedComment: lines[2].slice("trusted comment: ".length),
    untrustedComment: lines[0],
    globalSignature,
    minisignSignature,
  };
}

function ed25519KeyObject(rawPublicKey) {
  return createPublicKey({
    key: Buffer.concat([ed25519SpkiPrefix, rawPublicKey]),
    format: "der",
    type: "spki",
  });
}

async function hashFileBlake2b512(path) {
  const hash = createHash("blake2b512");
  await new Promise((resolvePromise, rejectPromise) => {
    const stream = createReadStream(path);
    stream.on("data", (chunk) => hash.update(chunk));
    stream.on("error", rejectPromise);
    stream.on("end", resolvePromise);
  });
  return hash.digest();
}

async function verifyTauriUpdaterSignatureFile(payloadPath, signatureBase64, publicKeyBase64, context = "updater artifact") {
  const publicKey = decodeTauriUpdaterPublicKey(publicKeyBase64, `${context} public key`);
  const signature = decodeTauriUpdaterSignature(signatureBase64, `${context} signature`);
  if (!publicKey.keyId.equals(signature.keyId)) {
    throw new UpdaterSignatureError(`${context} was signed by a different updater key`);
  }

  const signedPayload = signature.isPrehashed ? await hashFileBlake2b512(payloadPath) : await readFile(payloadPath);
  const keyObject = ed25519KeyObject(publicKey.key);
  if (!verifyEd25519(null, signedPayload, keyObject, signature.signature)) {
    throw new UpdaterSignatureError(`${context} signature verification failed`);
  }

  const globalMessage = Buffer.concat([signature.signature, Buffer.from(signature.trustedComment, "utf8")]);
  if (!verifyEd25519(null, globalMessage, keyObject, signature.globalSignature)) {
    throw new UpdaterSignatureError(`${context} trusted comment verification failed`);
  }

  return {
    algorithm: signature.algorithm,
    keyId: signature.keyId.toString("hex").toUpperCase(),
    prehashed: signature.isPrehashed,
    trustedComment: signature.trustedComment,
  };
}

export { resolveApprovedUpdaterPublicKey, verifyTauriUpdaterSignatureFile };
