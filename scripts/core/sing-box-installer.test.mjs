import { createHash } from "node:crypto";
import { existsSync, mkdirSync, writeFileSync } from "node:fs";
import { mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it, vi } from "vitest";

import {
  isCliEntrypoint,
  repoRootFromScript,
  truthy,
} from "../lib/common.mjs";
import {
  ALLOW_SEED_BACKFILL_ENV,
  ALLOW_UNPINNED_SING_BOX_ENV,
  DEFAULT_SING_BOX_VERSION,
  assertPinnedSingBoxArchive,
  assertSingBoxVersion,
  defaultAppConfigDir,
  ensureSingBoxSeedForBuild,
  expectedSingBoxArchiveSha256,
  fetchAndStageSingBoxSeed,
  installSingBoxCore,
  isCliEntrypoint as installerIsCliEntrypoint,
  readSingBoxSeedManifest,
  repoRootFromScript as installerRepoRootFromScript,
  shouldSkipSingBoxInstall,
  singBoxAppExecutable,
  singBoxAssetName,
  singBoxPinStatus,
  singBoxSeedDir,
  truthy as installerTruthy,
  verifyStagedSingBoxSeed,
} from "./sing-box-installer.mjs";

const windowsPin = expectedSingBoxArchiveSha256({
  arch: "x64",
  platform: "win32",
  version: DEFAULT_SING_BOX_VERSION,
});

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

/** Writes the manifest a verified `sing-box-<version>-windows-amd64.zip` staging run leaves behind. */
async function writeStagedWindowsSeed(seedDir, executableBody = "seed-sing-box") {
  await mkdir(seedDir, { recursive: true });
  await writeFile(join(seedDir, "sing-box.exe"), executableBody);
  await writeFile(join(seedDir, "LICENSE"), "seed-license");
  await writeFile(
    join(seedDir, "sing-box.seed.json"),
    `${JSON.stringify(
      {
        assetName: singBoxAssetName({ arch: "x64", platform: "win32", version: DEFAULT_SING_BOX_VERSION }),
        bytes: 1234,
        executableSha256: sha256(executableBody),
        kept: ["LICENSE", "sing-box.exe"],
        pinned: true,
        sha256: windowsPin,
        version: DEFAULT_SING_BOX_VERSION,
      },
      null,
      2,
    )}\n`,
  );
}

describe("sing-box core installer", () => {
  it("keeps the moved common helpers available from the installer", () => {
    const repoRoot = resolve(fileURLToPath(new URL("../..", import.meta.url)));

    expect(installerIsCliEntrypoint).toBe(isCliEntrypoint);
    expect(installerRepoRootFromScript).toBe(repoRootFromScript);
    expect(installerRepoRootFromScript()).toBe(repoRoot);
    expect(installerTruthy).toBe(truthy);
    expect(truthy(" yes ")).toBe(true);
  });

  it("selects pinned upstream assets for supported platforms", () => {
    expect(singBoxAssetName({ arch: "arm64", platform: "darwin", version: "v1.13.14" })).toBe(
      "sing-box-1.13.14-darwin-arm64.tar.gz",
    );
    expect(singBoxAssetName({ arch: "x64", platform: "linux", version: "v1.13.14" })).toBe(
      "sing-box-1.13.14-linux-amd64.tar.gz",
    );
    expect(singBoxAssetName({ arch: "x64", platform: "win32", version: "v1.13.14" })).toBe(
      "sing-box-1.13.14-windows-amd64.zip",
    );
    expect(singBoxAssetName({ arch: "riscv64", platform: "darwin" })).toBeNull();
  });

  it("resolves app config directories and honors explicit overrides", () => {
    expect(defaultAppConfigDir({ env: { VOYAVPN_APP_CONFIG_DIR: "/tmp/custom-voya" }, platform: "darwin" })).toBe(
      "/tmp/custom-voya",
    );
    expect(defaultAppConfigDir({ env: {}, home: "/Users/tester", platform: "darwin" })).toBe(
      "/Users/tester/Library/Application Support/app.voyavpn.desktop",
    );
    expect(defaultAppConfigDir({ env: { XDG_CONFIG_HOME: "/home/tester/.config" }, platform: "linux" })).toBe(
      "/home/tester/.config/app.voyavpn.desktop",
    );
  });

  it("skips postinstall for explicit opt-out and CI", () => {
    expect(shouldSkipSingBoxInstall({ env: { VOYAVPN_SKIP_SING_BOX_POSTINSTALL: "1" }, postinstall: true })).toEqual({
      reason: "VOYAVPN_SKIP_SING_BOX_POSTINSTALL=1",
      skip: true,
    });
    expect(shouldSkipSingBoxInstall({ env: { CI: "true" }, postinstall: true })).toEqual({
      reason: "CI postinstall without VOYAVPN_FETCH_SING_BOX_ON_INSTALL=1",
      skip: true,
    });
    expect(
      shouldSkipSingBoxInstall({
        env: { CI: "true", VOYAVPN_FETCH_SING_BOX_ON_INSTALL: "1" },
        postinstall: true,
      }),
    ).toEqual({ reason: null, skip: false });
  });

  it("copies a pinned, manifest-verified seed into app data without fetching", async () => {
    const workDir = await mkdtemp(join(tmpdir(), "voyavpn-sing-box-install-"));
    try {
      const repoRoot = join(workDir, "repo");
      const appConfigDir = join(workDir, "app-data");
      await writeStagedWindowsSeed(singBoxSeedDir(repoRoot));

      const stageSeed = vi.fn(async () => {
        throw new Error("stageSeed should not be called");
      });
      const result = await installSingBoxCore({
        appConfigDir,
        arch: "x64",
        platform: "win32",
        probeExecutable: (path) => existsSync(path) && path === singBoxAppExecutable(appConfigDir, "win32"),
        repoRoot,
        stageSeed,
      });

      expect(result.status).toBe("installed");
      expect(stageSeed).not.toHaveBeenCalled();
      expect(await readFile(singBoxAppExecutable(appConfigDir, "win32"), "utf8")).toBe("seed-sing-box");
      expect(await readFile(join(appConfigDir, "bin", "sing_box", "LICENSE"), "utf8")).toBe("seed-license");
    } finally {
      await rm(workDir, { force: true, recursive: true });
    }
  });

  it("never backfills the bundled seed from app data unless a developer opts in", async () => {
    const workDir = await mkdtemp(join(tmpdir(), "voyavpn-sing-box-backfill-"));
    try {
      const repoRoot = join(workDir, "repo");
      const appConfigDir = join(workDir, "app-data");
      const appExecutable = singBoxAppExecutable(appConfigDir, "win32");
      await mkdir(join(appConfigDir, "bin", "sing_box"), { recursive: true });
      await writeFile(appExecutable, "installed-sing-box");

      const guarded = await installSingBoxCore({
        appConfigDir,
        env: {},
        platform: "win32",
        probeExecutable: (path) => existsSync(path) && path === appExecutable,
        repoRoot,
      });

      expect(guarded.status).toBe("already-installed");
      expect(existsSync(join(singBoxSeedDir(repoRoot), "sing-box.exe"))).toBe(false);

      const optedIn = await installSingBoxCore({
        appConfigDir,
        env: { [ALLOW_SEED_BACKFILL_ENV]: "1" },
        platform: "win32",
        probeExecutable: (path) => existsSync(path) && path === appExecutable,
        repoRoot,
      });

      expect(optedIn.status).toBe("already-installed");
      expect(await readFile(join(singBoxSeedDir(repoRoot), "sing-box.exe"), "utf8")).toBe("installed-sing-box");
    } finally {
      await rm(workDir, { force: true, recursive: true });
    }
  });

  it("stages sing-box when no seed executable exists", async () => {
    const workDir = await mkdtemp(join(tmpdir(), "voyavpn-sing-box-stage-"));
    try {
      const repoRoot = join(workDir, "repo");
      const appConfigDir = join(workDir, "app-data");
      const stageSeed = vi.fn(async () => {
        const seedDir = singBoxSeedDir(repoRoot);
        await mkdir(seedDir, { recursive: true });
        await writeFile(join(seedDir, "sing-box.exe"), "downloaded-sing-box");
      });

      const result = await installSingBoxCore({
        appConfigDir,
        platform: "win32",
        probeExecutable: (path) => existsSync(path) && path === singBoxAppExecutable(appConfigDir, "win32"),
        repoRoot,
        stageSeed,
      });

      expect(stageSeed).toHaveBeenCalledTimes(1);
      expect(result.status).toBe("installed");
      expect(await readFile(singBoxAppExecutable(appConfigDir, "win32"), "utf8")).toBe("downloaded-sing-box");
    } finally {
      await rm(workDir, { force: true, recursive: true });
    }
  });
});

describe("sing-box seed integrity pinning", () => {
  const pinnedVersion = DEFAULT_SING_BOX_VERSION;
  const unpinnedVersion = "v9.9.9";

  function fakeFetch(body) {
    return vi.fn(async () => ({
      arrayBuffer: async () => body.buffer.slice(body.byteOffset, body.byteOffset + body.byteLength),
      ok: true,
      status: 200,
      statusText: "OK",
    }));
  }

  /** Stands in for `tar -xzf <archive> -C <dest>` by materializing the payload. */
  function fakeExtractSpawn() {
    return vi.fn((file, args) => {
      const destDir = args[args.indexOf("-C") + 1];
      mkdirSync(destDir, { recursive: true });
      writeFileSync(join(destDir, "sing-box"), "extracted-sing-box");
      writeFileSync(join(destDir, "LICENSE"), "extracted-license");
      return { status: 0 };
    });
  }

  it("validates SING_BOX_VERSION before it reaches a URL", () => {
    expect(assertSingBoxVersion("v1.13.14")).toBe("v1.13.14");
    expect(assertSingBoxVersion("v1.13.14-beta.1")).toBe("v1.13.14-beta.1");
    for (const invalid of ["1.13.14", "latest", "v1.13", "v1.13.14; rm -rf /", "v1.13.14 && calc"]) {
      expect(() => assertSingBoxVersion(invalid)).toThrow(/SING_BOX_VERSION must look like/);
    }
  });

  it("pins every {version, platform, arch} the installer can stage", () => {
    for (const platform of ["darwin", "linux", "win32"]) {
      for (const arch of ["arm64", "x64"]) {
        const status = singBoxPinStatus({ arch, env: {}, platform, version: pinnedVersion });
        expect(status.pinned, `${platform}:${arch} must be pinned`).toBe(true);
        expect(status.expected).toMatch(/^[a-f0-9]{64}$/);
      }
    }
  });

  it("refuses an unpinned target unless the escape hatch is set", () => {
    expect(() =>
      assertPinnedSingBoxArchive({
        actualSha256: "a".repeat(64),
        arch: "x64",
        assetName: "sing-box-9.9.9-linux-amd64.tar.gz",
        env: {},
        platform: "linux",
        version: unpinnedVersion,
      }),
    ).toThrow(/has no pinned SHA-256 for linux:x64/);

    expect(
      assertPinnedSingBoxArchive({
        actualSha256: "a".repeat(64),
        arch: "x64",
        assetName: "sing-box-9.9.9-linux-amd64.tar.gz",
        env: { [ALLOW_UNPINNED_SING_BOX_ENV]: "1" },
        platform: "linux",
        version: unpinnedVersion,
      }),
    ).toEqual({ pinned: false, sha256: "a".repeat(64) });
  });

  it("rejects a mismatching download before extracting or staging anything", async () => {
    const workDir = await mkdtemp(join(tmpdir(), "voyavpn-sing-box-pin-"));
    try {
      const repoRoot = join(workDir, "repo");
      const fetchImpl = fakeFetch(Buffer.from("tampered sing-box archive"));
      const spawn = fakeExtractSpawn();

      await expect(
        fetchAndStageSingBoxSeed({
          arch: "x64",
          env: {},
          fetchImpl,
          logger: { log() {}, warn() {} },
          platform: "linux",
          repoRoot,
          spawn,
          version: pinnedVersion,
        }),
      ).rejects.toThrow(/failed SHA-256 pinning: expected/);

      expect(fetchImpl).toHaveBeenCalledTimes(1);
      expect(spawn).not.toHaveBeenCalled();
      expect(existsSync(join(singBoxSeedDir(repoRoot), "sing-box"))).toBe(false);
    } finally {
      await rm(workDir, { force: true, recursive: true });
    }
  });

  it("never reaches the network for a target with no pinned digest", async () => {
    const workDir = await mkdtemp(join(tmpdir(), "voyavpn-sing-box-unpinned-"));
    try {
      const fetchImpl = fakeFetch(Buffer.from("payload"));

      await expect(
        fetchAndStageSingBoxSeed({
          arch: "x64",
          env: {},
          fetchImpl,
          logger: { log() {}, warn() {} },
          platform: "linux",
          repoRoot: join(workDir, "repo"),
          spawn: fakeExtractSpawn(),
          version: unpinnedVersion,
        }),
      ).rejects.toThrow(new RegExp(`${ALLOW_UNPINNED_SING_BOX_ENV}=1`));

      expect(fetchImpl).not.toHaveBeenCalled();
    } finally {
      await rm(workDir, { force: true, recursive: true });
    }
  });

  it("records the verified archive and executable digests in the seed manifest", async () => {
    const workDir = await mkdtemp(join(tmpdir(), "voyavpn-sing-box-manifest-"));
    try {
      const repoRoot = join(workDir, "repo");
      const payload = Buffer.from("unverified archive bytes");
      const env = { [ALLOW_UNPINNED_SING_BOX_ENV]: "1" };

      const result = await fetchAndStageSingBoxSeed({
        arch: "x64",
        env,
        fetchImpl: fakeFetch(payload),
        logger: { log() {}, warn() {} },
        platform: "linux",
        repoRoot,
        spawn: fakeExtractSpawn(),
        version: unpinnedVersion,
      });

      const manifest = readSingBoxSeedManifest(singBoxSeedDir(repoRoot));
      expect(result.sha256).toBe(sha256(payload));
      expect(manifest).toMatchObject({
        assetName: "sing-box-9.9.9-linux-amd64.tar.gz",
        executableSha256: sha256("extracted-sing-box"),
        pinned: false,
        sha256: sha256(payload),
        version: unpinnedVersion,
      });
      expect(
        verifyStagedSingBoxSeed({ arch: "x64", env, platform: "linux", repoRoot, version: unpinnedVersion }),
      ).toMatchObject({ ok: true });
    } finally {
      await rm(workDir, { force: true, recursive: true });
    }
  });

  it("keeps an unverifiable seed usable only behind the escape hatch", async () => {
    const workDir = await mkdtemp(join(tmpdir(), "voyavpn-sing-box-legacy-"));
    try {
      const repoRoot = join(workDir, "repo");
      const seedDir = singBoxSeedDir(repoRoot);
      await mkdir(seedDir, { recursive: true });
      await writeFile(join(seedDir, "sing-box.exe"), "seed-sing-box");

      const verify = (env) =>
        verifyStagedSingBoxSeed({ arch: "x64", env, platform: "win32", repoRoot, version: pinnedVersion });

      // A seed with no manifest at all is re-staged by default...
      expect(verify({})).toMatchObject({ code: "manifest-missing", ok: false });
      // ...and accepted, but reported as unpinned, for an offline developer.
      expect(verify({ [ALLOW_UNPINNED_SING_BOX_ENV]: "1" })).toMatchObject({ ok: true, pinned: false });
    } finally {
      await rm(workDir, { force: true, recursive: true });
    }
  });

  it("re-fetches a stale, foreign, or locally modified seed instead of bundling it", async () => {
    const workDir = await mkdtemp(join(tmpdir(), "voyavpn-sing-box-stale-"));
    try {
      const repoRoot = join(workDir, "repo");
      const seedDir = singBoxSeedDir(repoRoot);
      await writeStagedWindowsSeed(seedDir);

      const verified = await ensureSingBoxSeedForBuild({
        arch: "x64",
        env: {},
        logger: { log() {} },
        platform: "win32",
        repoRoot,
        stageSeed: vi.fn(async () => {
          throw new Error("a verified seed must not be re-staged");
        }),
        version: pinnedVersion,
      });
      expect(verified.status).toBe("already-staged");

      // A binary swapped in behind the manifest must not survive into a bundle.
      await writeFile(join(seedDir, "sing-box.exe"), "swapped-binary");
      const stageSeed = vi.fn(async () => ({ seedDir }));
      await ensureSingBoxSeedForBuild({
        arch: "x64",
        env: {},
        logger: { log() {} },
        platform: "win32",
        repoRoot,
        stageSeed,
        version: pinnedVersion,
      });
      expect(stageSeed).toHaveBeenCalledTimes(1);

      // So must a seed staged for a different version or architecture.
      await writeStagedWindowsSeed(seedDir);
      expect(
        verifyStagedSingBoxSeed({ arch: "x64", env: {}, platform: "win32", repoRoot, version: "v1.14.0" }),
      ).toMatchObject({ ok: false, reason: expect.stringContaining("expected v1.14.0") });
      expect(
        verifyStagedSingBoxSeed({ arch: "arm64", env: {}, platform: "win32", repoRoot, version: pinnedVersion }),
      ).toMatchObject({ ok: false, reason: expect.stringContaining("windows-arm64") });
    } finally {
      await rm(workDir, { force: true, recursive: true });
    }
  });
});
