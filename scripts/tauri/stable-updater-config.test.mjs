import { readFileSync } from "node:fs";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";

import { repoRootFromScript } from "../lib/common.mjs";
import {
  normalizeCiEnv,
  requestedStableUpdaterConfig,
  writeStableUpdaterOverlay,
} from "./stable-updater-config.mjs";

const temporaryDirectories = [];
const publicKey = readFileSync(
  join(repoRootFromScript(import.meta.url), "tests/fixtures/release/updater-signing/public.key"),
  "utf8",
).trim();

afterEach(async () => {
  await Promise.all(temporaryDirectories.splice(0).map((path) => rm(path, { force: true, recursive: true })));
});

describe("stable updater config", () => {
  it("selects stable builds from explicit or channel settings", () => {
    expect(requestedStableUpdaterConfig({ VOYAVPN_TAURI_UPDATER_CONFIG: "stable" })).toBe(true);
    expect(requestedStableUpdaterConfig({ VOYAVPN_TAURI_UPDATER_CONFIG: "false" })).toBe(false);
    expect(requestedStableUpdaterConfig({ VOYAVPN_RELEASE_CHANNEL: "stable" })).toBe(true);
    expect(requestedStableUpdaterConfig({ CHANNEL: "beta" })).toBe(false);
    expect(() => requestedStableUpdaterConfig({ VOYAVPN_TAURI_UPDATER_CONFIG: "sometimes" })).toThrow(
      /must be stable, true, or false/,
    );
  });

  it("normalizes only numeric CI booleans", () => {
    expect(normalizeCiEnv({ CI: "1", VALUE: "kept" })).toEqual({ CI: "true", VALUE: "kept" });
    expect(normalizeCiEnv({ CI: "0" })).toEqual({ CI: "false" });
    expect(normalizeCiEnv({ CI: "true" })).toEqual({ CI: "true" });
  });

  it("writes a fail-closed stable overlay and matching metadata", async () => {
    const repoRoot = await mkdtemp(join(tmpdir(), "voyavpn-updater-config-"));
    temporaryDirectories.push(repoRoot);
    const overlayPath = writeStableUpdaterOverlay({
      repoRoot,
      env: {
        TAURI_SIGNING_PRIVATE_KEY_PATH: "/secure/updater.key",
        VOYAVPN_UPDATER_PUBLIC_KEY: publicKey,
        VOYAVPN_UPDATES_BASE_URL: "https://updates.voyavpn.dev/stable/",
      },
    });

    const overlay = JSON.parse(await readFile(overlayPath, "utf8"));
    const metadata = JSON.parse(await readFile(
      join(repoRoot, "target/release-config/tauri.updater.stable.generated.metadata.json"),
      "utf8",
    ));
    expect(overlay.bundle.createUpdaterArtifacts).toBe(true);
    expect(overlay.plugins.updater.endpoints).toEqual(["https://updates.voyavpn.dev/stable/latest.json"]);
    expect(overlay.plugins.updater.pubkey).toBe(publicKey);
    expect(metadata.createUpdaterArtifacts).toBe(true);
    expect(metadata.sha256).toMatch(/^[a-f0-9]{64}$/);
    expect(metadata.pubkeySha256).toMatch(/^[a-f0-9]{64}$/);
  });

  it("rejects missing signing input, unsafe URLs, and placeholder keys", async () => {
    const repoRoot = await mkdtemp(join(tmpdir(), "voyavpn-updater-config-errors-"));
    temporaryDirectories.push(repoRoot);
    const baseEnv = {
      TAURI_SIGNING_PRIVATE_KEY: "private",
      VOYAVPN_UPDATER_PUBLIC_KEY: publicKey,
      VOYAVPN_UPDATES_BASE_URL: "https://updates.voyavpn.dev/stable",
    };

    expect(() => writeStableUpdaterOverlay({ repoRoot, env: {} })).toThrow(/TAURI_SIGNING_PRIVATE_KEY/);
    expect(() => writeStableUpdaterOverlay({
      repoRoot,
      env: { ...baseEnv, VOYAVPN_UPDATES_BASE_URL: "http://updates.voyavpn.dev/stable" },
    })).toThrow(/must use https/);
    expect(() => writeStableUpdaterOverlay({
      repoRoot,
      env: { ...baseEnv, VOYAVPN_UPDATES_BASE_URL: "https://cdn.example.com/stable" },
    })).toThrow(/must not use example/);
    expect(() => writeStableUpdaterOverlay({
      repoRoot,
      env: { ...baseEnv, VOYAVPN_UPDATER_PUBLIC_KEY: "placeholder" },
    })).toThrow(/non-placeholder/);
  });

  // Long enough and no placeholder word, so only structural decoding catches it.
  it.each([
    ["not base64", "approved-updater-public-key-material-0123456789"],
    ["base64 of plain text", Buffer.from("approved updater public key material").toString("base64")],
    [
      "a minisign key with a truncated packet",
      Buffer.from("untrusted comment: minisign public key\nRWSw1t07ojiA3yM4oxhVV/s63a5K\n").toString("base64"),
    ],
  ])("rejects a malformed updater public key (%s)", async (_label, malformedKey) => {
    const repoRoot = await mkdtemp(join(tmpdir(), "voyavpn-updater-config-malformed-"));
    temporaryDirectories.push(repoRoot);

    expect(() => writeStableUpdaterOverlay({
      repoRoot,
      env: {
        TAURI_SIGNING_PRIVATE_KEY: "private",
        VOYAVPN_UPDATER_PUBLIC_KEY: malformedKey,
        VOYAVPN_UPDATES_BASE_URL: "https://updates.voyavpn.dev/stable",
      },
    })).toThrow(/approved updater public key/);
  });
});
