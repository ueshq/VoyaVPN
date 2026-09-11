import { execFile } from "node:child_process";
import { mkdtemp, mkdir, readFile, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { afterEach, describe, expect, it } from "vitest";

import { classifyArtifact, main, selectUpdaterPayloadPath, updaterPayloadPlatform } from "./artifacts.mjs";
import { selectUpdaterPayload } from "../validation.mjs";

const execFileAsync = promisify(execFile);
const repoRoot = resolve(fileURLToPath(new URL("../../..", import.meta.url)));
const workDirs = [];

afterEach(async () => {
  await Promise.all(workDirs.splice(0).map((path) => rm(path, { force: true, recursive: true })));
});

async function workDir() {
  const path = await mkdtemp(join(tmpdir(), "voyavpn-release-artifacts-"));
  workDirs.push(path);
  return path;
}

/** Writes a bundle tree shaped like `tauri build` output with in-place updater signatures. */
async function writeBundle(root, files) {
  for (const [relativePath, contents] of Object.entries(files)) {
    const path = join(root, relativePath);
    await mkdir(dirname(path), { recursive: true });
    await writeFile(path, contents);
  }
  return root;
}

const approvedOverlay = {
  bundle: { createUpdaterArtifacts: true },
  plugins: {
    updater: { pubkey: "A".repeat(64), endpoints: ["https://updates.voyavpn.dev/stable/latest.json"] },
  },
};

/** Runs `release artifacts` for a stable channel with an approved updater overlay. */
async function collect(inputDir, outputDir, target, extra = []) {
  const overlayArgs = extra.includes("--stable-updater-config") ? [] : ["--stable-updater-config", await overlayPath()];
  await main([
    "--input",
    inputDir,
    "--output",
    outputDir,
    "--target",
    target,
    "--channel",
    "stable",
    "--version",
    "0.1.0",
    ...overlayArgs,
    ...extra,
  ]);
  return JSON.parse(await readFile(join(outputDir, "artifact-manifest.json"), "utf8"));
}

async function overlayPath() {
  const root = await workDir();
  const path = join(root, "tauri.updater.stable.generated.json");
  await writeFile(path, JSON.stringify(approvedOverlay));
  return path;
}

// Tauri 2 bundle trees with `createUpdaterArtifacts: true`: the installers are
// signed in place, and only macOS emits a separate .app.tar.gz.
const windowsBundle = {
  "msi/VoyaVPN_0.1.0_x64_en-US.msi": "msi bytes",
  "msi/VoyaVPN_0.1.0_x64_en-US.msi.sig": "msi signature",
  "nsis/VoyaVPN_0.1.0_x64-setup.exe": "nsis bytes",
  "nsis/VoyaVPN_0.1.0_x64-setup.exe.sig": "nsis signature",
};

const linuxBundle = {
  "appimage/VoyaVPN_0.1.0_amd64.AppImage": "appimage bytes",
  "appimage/VoyaVPN_0.1.0_amd64.AppImage.sig": "appimage signature",
  "deb/VoyaVPN_0.1.0_amd64.deb": "deb bytes",
  "deb/VoyaVPN_0.1.0_amd64.deb.sig": "deb signature",
  "rpm/VoyaVPN-0.1.0-1.x86_64.rpm": "rpm bytes",
  "rpm/VoyaVPN-0.1.0-1.x86_64.rpm.sig": "rpm signature",
};

const darwinBundle = {
  "dmg/VoyaVPN_0.1.0_aarch64.dmg": "dmg bytes",
  "macos/VoyaVPN.app.tar.gz": "app archive bytes",
  "macos/VoyaVPN.app.tar.gz.sig": "app archive signature",
};

describe("release artifacts", () => {
  it("skips symbolic links while walking bundle outputs", async () => {
    const root = await workDir();
    const inputDir = join(root, "bundle");
    const outputDir = join(root, "out");
    const realArtifact = join(inputDir, "dmg", "VoyaVPN_0.1.0_x64.dmg");

    await mkdir(join(inputDir, "dmg"), { recursive: true });
    await writeFile(realArtifact, "real dmg bytes");
    try {
      await symlink(realArtifact, join(inputDir, "linked.dmg"));
    } catch (error) {
      if (error && (error.code === "EPERM" || error.code === "ENOSYS")) {
        return;
      }
      throw error;
    }

    await execFileAsync(
      process.execPath,
      [
        "scripts/release/cli.mjs",
        "artifacts",
        "--input",
        inputDir,
        "--output",
        outputDir,
        "--target",
        "darwin-x86_64",
        "--channel",
        "beta",
        "--version",
        "0.1.0",
      ],
      { cwd: repoRoot },
    );

    const manifest = JSON.parse(await readFile(join(outputDir, "artifact-manifest.json"), "utf8"));
    expect(manifest.artifacts).toHaveLength(1);
    expect(manifest.artifacts[0].originalRelativePath).toBe("dmg/VoyaVPN_0.1.0_x64.dmg");
  });

  it("classifies bundle outputs by suffix and bundler directory", () => {
    const cases = [
      ["macos/VoyaVPN.app.tar.gz", ".tar.gz", "updater"],
      ["macos/VoyaVPN.app.tar.gz.sig", ".tar.gz.sig", "signature"],
      ["dmg/VoyaVPN_0.1.0_aarch64.dmg", ".dmg", "dmg"],
      ["nsis/VoyaVPN_0.1.0_x64-setup.exe", ".exe", "nsis"],
      ["nsis/VoyaVPN_0.1.0_x64-setup.exe.sig", ".exe.sig", "signature"],
      ["VoyaVPN_0.1.0_x64-setup.exe", ".exe", "setup"],
      ["msi/VoyaVPN_0.1.0_x64_en-US.msi", ".msi", "msi"],
      ["appimage/VoyaVPN_0.1.0_amd64.AppImage", ".AppImage", "appimage"],
      ["deb/VoyaVPN_0.1.0_amd64.deb", ".deb", "deb"],
      ["rpm/VoyaVPN-0.1.0-1.x86_64.rpm", ".rpm", "rpm"],
      ["updater/VoyaVPN_0.1.0_x64.updater.zip", ".zip", "artifact"],
    ];

    for (const [relativePath, suffix, kind] of cases) {
      expect(classifyArtifact(join("/bundle", relativePath), "/bundle", suffix), relativePath).toBe(kind);
    }
  });

  it("picks the designated signed installer as the updater payload for each OS", () => {
    expect(updaterPayloadPlatform("windows-aarch64")).toBe("windows");
    expect(updaterPayloadPlatform("freebsd-x86_64")).toBeNull();

    // Windows and Linux both sign several installers in place; the choice must
    // be the designated one, not whichever sorts first.
    expect(selectUpdaterPayloadPath(Object.keys(windowsBundle), "windows-x86_64")).toBe(
      "nsis/VoyaVPN_0.1.0_x64-setup.exe",
    );
    expect(selectUpdaterPayloadPath(Object.keys(linuxBundle), "linux-x86_64")).toBe(
      "appimage/VoyaVPN_0.1.0_amd64.AppImage",
    );
    expect(selectUpdaterPayloadPath(Object.keys(darwinBundle), "darwin-aarch64")).toBe("macos/VoyaVPN.app.tar.gz");

    // An unsigned bundle has no updater payload at all.
    expect(selectUpdaterPayloadPath(["nsis/VoyaVPN_0.1.0_x64-setup.exe"], "windows-x86_64")).toBeNull();
  });

  it("does not select retired updater archives or unmarked manifest entries", () => {
    for (const [target, path] of [
      ["windows-x86_64", "nsis/VoyaVPN.nsis.zip"],
      ["linux-x86_64", "appimage/VoyaVPN.AppImage.tar.gz"],
      ["darwin-aarch64", "macos/unrelated.tar.gz"],
      ["windows-x86_64", "updater/VoyaVPN.zip"],
    ]) {
      expect(selectUpdaterPayloadPath([path, `${path}.sig`], target)).toBeNull();
    }
    expect(selectUpdaterPayload([{ kind: "updater", name: "old.zip" }])).toBeNull();
    expect(selectUpdaterPayload([
      { kind: "signature", name: "app.sig", updaterPayload: true },
    ])).toBeNull();
    expect(() => selectUpdaterPayload([
      { name: "first.exe", updaterPayload: true },
      { name: "second.exe", updaterPayload: true },
    ])).toThrow(/marks 2 updater payloads/);
  });

  it("marks the Tauri 2 in-place updater payload and its signature in the manifest", async () => {
    const root = await workDir();
    const manifest = await collect(
      await writeBundle(join(root, "bundle"), windowsBundle),
      join(root, "out"),
      "windows-x86_64",
    );

    expect(manifest.updaterPayloadSource).toBe("nsis/VoyaVPN_0.1.0_x64-setup.exe");
    expect(manifest.artifacts.map((artifact) => [artifact.kind, artifact.updaterPayload === true])).toEqual([
      ["msi", false],
      ["signature", false],
      ["nsis", true],
      ["signature", false],
    ]);

    const payload = selectUpdaterPayload(manifest.artifacts);
    expect(payload.name).toBe("voyavpn-0.1.0-stable-windows-x86-64-nsis.exe");
    expect(manifest.artifacts.find((artifact) => artifact.updaterSignature === true).name).toBe(
      "voyavpn-0.1.0-stable-windows-x86-64-nsis.exe.sig",
    );
  });

  it("marks the AppImage on Linux and the .app.tar.gz on macOS", async () => {
    const root = await workDir();

    const linux = await collect(await writeBundle(join(root, "linux"), linuxBundle), join(root, "linux-out"), "linux-x86_64");
    expect(selectUpdaterPayload(linux.artifacts).originalRelativePath).toBe("appimage/VoyaVPN_0.1.0_amd64.AppImage");

    const darwin = await collect(
      await writeBundle(join(root, "darwin"), darwinBundle),
      join(root, "darwin-out"),
      "darwin-aarch64",
    );
    expect(selectUpdaterPayload(darwin.artifacts).originalRelativePath).toBe("macos/VoyaVPN.app.tar.gz");
  });

  it("fails a stable collection whose bundle has no signed updater payload", async () => {
    const root = await workDir();
    const inputDir = await writeBundle(join(root, "bundle"), {
      "msi/VoyaVPN_0.1.0_x64_en-US.msi": "msi bytes",
      "nsis/VoyaVPN_0.1.0_x64-setup.exe": "nsis bytes",
    });

    await expect(collect(inputDir, join(root, "out"), "windows-x86_64")).rejects.toThrow(
      /No signed updater payload found for windows-x86_64/,
    );
  });

  it("suffixes colliding artifact names instead of overwriting them", async () => {
    const root = await workDir();
    const manifest = await collect(
      await writeBundle(join(root, "bundle"), {
        "dmg/VoyaVPN_0.1.0_aarch64.dmg": "first dmg",
        "extra/VoyaVPN_0.1.0_aarch64.dmg": "second dmg",
        "macos/VoyaVPN.app.tar.gz": "app archive bytes",
        "macos/VoyaVPN.app.tar.gz.sig": "app archive signature",
      }),
      join(root, "out"),
      "darwin-aarch64",
    );

    expect(manifest.artifacts.filter((artifact) => artifact.kind === "dmg").map((artifact) => artifact.name)).toEqual([
      "voyavpn-0.1.0-stable-darwin-aarch64-dmg.dmg",
      "voyavpn-0.1.0-stable-darwin-aarch64-dmg-2.dmg",
    ]);
  });

  it("rejects a stable updater overlay with a placeholder key or endpoints", async () => {
    const root = await workDir();
    const inputDir = await writeBundle(join(root, "bundle"), darwinBundle);
    const overlayPath = join(root, "overlay.json");

    const writeOverlay = async (overlay) => writeFile(overlayPath, JSON.stringify(overlay));
    const run = () =>
      collect(inputDir, join(root, "out"), "darwin-aarch64", ["--stable-updater-config", overlayPath]);

    await writeOverlay({ bundle: { createUpdaterArtifacts: false } });
    await expect(run()).rejects.toThrow(/must enable bundle.createUpdaterArtifacts/);

    await writeOverlay({
      bundle: { createUpdaterArtifacts: true },
      plugins: { updater: { pubkey: "REPLACE_BEFORE_RELEASE", endpoints: ["https://updates.voyavpn.dev/latest.json"] } },
    });
    await expect(run()).rejects.toThrow(/approved non-placeholder updater public key/);

    await writeOverlay({
      bundle: { createUpdaterArtifacts: true },
      plugins: { updater: { pubkey: "A".repeat(64), endpoints: [] } },
    });
    await expect(run()).rejects.toThrow(/non-placeholder updater endpoints/);

    await writeOverlay(approvedOverlay);
    const manifest = await run();
    expect(manifest.stableUpdaterConfig).toMatchObject({ createUpdaterArtifacts: true });
  });
});
