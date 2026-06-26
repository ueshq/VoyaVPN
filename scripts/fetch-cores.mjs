// Fetch & stage proxy core binaries into src-tauri/resources/core-seeds/<core>/ so that
// Tauri bundles them and the app's startup seed-copy (copy_seed_core_assets) installs them
// into {appConfigDir}/bin/<core>/ on first run.
//
// Cores are intentionally NOT committed to the repo (large, separately-licensed binaries —
// see crates/voya-net/src/update.rs: redistribute_in_installer=false). Run this before a
// `tauri build`/`tauri dev` when you want the bundled-seed path to work:
//
//   node scripts/fetch-cores.mjs                 # host os/arch, pinned versions
//   XRAY_VERSION=v26.3.27 node scripts/fetch-cores.mjs
//
// By default it only stages the cores listed in CORES (currently Xray). The seed directory
// layout and names mirror voya-platform `core_type_dir_name` (Xray -> "xray").

import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { cpSync, existsSync, mkdirSync, mkdtempSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(fileURLToPath(new URL("..", import.meta.url)));
const seedRoot = join(repoRoot, "src-tauri", "resources", "core-seeds");

const platform = process.platform; // "win32" | "darwin" | "linux"
const arch = process.arch; // "x64" | "arm64" | ...

const CORES = [
  {
    dir: "xray", // must match voya-platform core_type_dir_name(CoreType::Xray)
    repo: "XTLS/Xray-core",
    version: process.env.XRAY_VERSION ?? "v26.3.27",
    // {0} is replaced with the version tag.
    assets: {
      "win32:x64": "Xray-windows-64.zip",
      "win32:arm64": "Xray-windows-arm64-v8a.zip",
      "darwin:x64": "Xray-macos-64.zip",
      "darwin:arm64": "Xray-macos-arm64-v8a.zip",
      "linux:x64": "Xray-linux-64.zip",
      "linux:arm64": "Xray-linux-arm64-v8a.zip",
    },
    // Files to keep from the archive (executable + geo assets for XRAY_LOCATION_ASSET).
    keep: (name) => /^xray(\.exe)?$/i.test(name) || /\.dat$/i.test(name),
  },
];

async function download(url, destFile) {
  const res = await fetch(url, { headers: { "User-Agent": "voyavpn-fetch-cores" }, redirect: "follow" });
  if (!res.ok) {
    throw new Error(`download failed ${res.status} ${res.statusText}: ${url}`);
  }
  const buf = Buffer.from(await res.arrayBuffer());
  writeFileSync(destFile, buf);

  return buf;
}

async function verifyChecksum(buf, dgstUrl) {
  try {
    const res = await fetch(dgstUrl, { headers: { "User-Agent": "voyavpn-fetch-cores" } });
    if (!res.ok) {
      console.warn(`  ! checksum file unavailable (${res.status}); skipping verification`);

      return;
    }
    const text = await res.text();
    const match = text.match(/sha2?-?256[^0-9a-f]*([0-9a-f]{64})/i) ?? text.match(/\b([0-9a-f]{64})\b/i);
    if (!match) {
      console.warn("  ! could not parse SHA256 from checksum file; skipping verification");

      return;
    }
    const expected = match[1].toLowerCase();
    const actual = createHash("sha256").update(buf).digest("hex");
    if (expected !== actual) {
      throw new Error(`checksum mismatch: expected ${expected}, got ${actual}`);
    }
    console.log("  ✓ SHA256 verified");
  } catch (error) {
    if (error.message.startsWith("checksum mismatch")) {
      throw error;
    }
    console.warn(`  ! checksum verification skipped: ${error.message}`);
  }
}

function extractZip(zipFile, destDir) {
  mkdirSync(destDir, { recursive: true });
  const cmd =
    platform === "win32"
      ? {
          file: "powershell",
          args: ["-NoProfile", "-Command", `Expand-Archive -Path "${zipFile}" -DestinationPath "${destDir}" -Force`],
        }
      : { file: "unzip", args: ["-o", zipFile, "-d", destDir] };

  const result = spawnSync(cmd.file, cmd.args, { stdio: "inherit" });
  if (result.status !== 0) {
    throw new Error(`extraction failed (${cmd.file} exited ${result.status ?? "null"})`);
  }
}

async function stageCore(core) {
  const key = `${platform}:${arch}`;
  const assetName = core.assets[key];
  if (!assetName) {
    console.warn(`- ${core.dir}: no asset for ${key}; skipping`);

    return;
  }

  const url = `https://github.com/${core.repo}/releases/download/${core.version}/${assetName}`;
  console.log(`- ${core.dir}: ${core.repo} ${core.version} (${assetName})`);

  const tmp = mkdtempSync(join(tmpdir(), `voyavpn-core-${core.dir}-`));
  try {
    const zipFile = join(tmp, assetName);
    const buf = await download(url, zipFile);
    await verifyChecksum(buf, `${url}.dgst`);

    const extractDir = join(tmp, "extract");
    extractZip(zipFile, extractDir);

    const seedDir = join(seedRoot, core.dir);
    mkdirSync(seedDir, { recursive: true });
    const kept = [];
    for (const entry of readdirSync(extractDir, { recursive: true, withFileTypes: true })) {
      if (!entry.isFile() || !core.keep(entry.name)) {
        continue;
      }
      cpSync(join(entry.parentPath ?? entry.path, entry.name), join(seedDir, entry.name));
      kept.push(entry.name);
    }
    if (kept.length === 0) {
      throw new Error(`no expected files found in ${assetName}`);
    }
    console.log(`  ✓ staged ${kept.join(", ")} -> resources/core-seeds/${core.dir}/`);
  } finally {
    rmSync(tmp, { force: true, recursive: true });
  }
}

mkdirSync(seedRoot, { recursive: true });
if (!existsSync(join(seedRoot, ".gitignore"))) {
  console.warn("Note: seed .gitignore missing; binaries may be committed unintentionally.");
}

for (const core of CORES) {
  await stageCore(core);
}

console.log("Done. Core seeds staged for", `${platform}:${arch}`);
