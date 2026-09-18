import { appendFileSync, existsSync, lstatSync, readdirSync, statSync } from "node:fs";
import { join, relative } from "node:path";

import { isCliEntrypoint, repoRootFromScript } from "../lib/common.mjs";

/**
 * Reports what a build produced and how big it is: the shell binaries and every
 * package in the Tauri bundle directory, with a per-part breakdown of a macOS
 * `.app`. Read-only and budget-free for now; the release workflow appends the
 * table to its job summary so sizes can be compared across releases.
 *
 *   pnpm size:report [--target <rust-triple>] [--profile release|debug]
 */

const BINARIES = ["voyavpn", "voyavpn-tunnel-service"];
const PACKAGE_EXTENSIONS = [".dmg", ".exe", ".msi", ".deb", ".rpm", ".AppImage", ".tar.gz"];
const APP_PARTS = ["Contents/MacOS", "Contents/Resources", "Contents/PlugIns", "Contents/Library", "Contents/Frameworks"];

export function parseArgs(argv) {
  const options = { profile: "release", target: null };
  for (let index = 0; index < argv.length; index += 1) {
    const [flag, inline] = argv[index].split("=", 2);
    const value = inline ?? argv[index + 1];
    if (flag === "--target" || flag === "--profile") {
      options[flag.slice(2)] = value;
      if (inline === undefined) index += 1;
    }
  }
  return options;
}

/** Bytes under `path`, following nothing: symlinks count as themselves. */
export function diskSize(path) {
  const stat = lstatSync(path);
  if (!stat.isDirectory()) return stat.size;
  return readdirSync(path).reduce((sum, name) => sum + diskSize(join(path, name)), 0);
}

export function formatBytes(bytes) {
  return `${(bytes / 1024 / 1024).toFixed(1)} MiB`;
}

function outputDirs(repoRoot, { profile, target }) {
  const dirs = target ? [join(repoRoot, "target", target, profile)] : [];
  dirs.push(join(repoRoot, "target", profile));
  return dirs.filter((dir) => existsSync(dir));
}

function findPackages(bundleDir) {
  if (!existsSync(bundleDir)) return [];
  return readdirSync(bundleDir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(bundleDir, entry.name);
    if (entry.name.endsWith(".app")) return [path];
    if (entry.isDirectory()) return findPackages(path);
    return PACKAGE_EXTENSIONS.some((extension) => entry.name.endsWith(extension)) ? [path] : [];
  });
}

export function collectSizes(repoRoot, options) {
  const rows = [];
  const [outputDir] = outputDirs(repoRoot, options);
  if (!outputDir) return rows;

  for (const name of BINARIES) {
    for (const file of [name, `${name}.exe`]) {
      const path = join(outputDir, file);
      if (existsSync(path)) rows.push({ bytes: statSync(path).size, path: relative(repoRoot, path) });
    }
  }
  for (const path of findPackages(join(outputDir, "bundle"))) {
    rows.push({ bytes: diskSize(path), path: relative(repoRoot, path) });
    if (!path.endsWith(".app")) continue;
    for (const part of APP_PARTS) {
      const partPath = join(path, part);
      if (existsSync(partPath)) rows.push({ bytes: diskSize(partPath), path: `  ${part}` });
    }
  }
  return rows;
}

export function markdownTable(rows) {
  return ["| Artifact | Size |", "| --- | ---: |", ...rows.map(({ bytes, path }) => `| \`${path}\` | ${formatBytes(bytes)} |`)].join("\n");
}

if (isCliEntrypoint(import.meta.url)) {
  const repoRoot = repoRootFromScript(import.meta.url);
  const options = parseArgs(process.argv.slice(2));
  const rows = collectSizes(repoRoot, options);
  if (rows.length === 0) {
    console.log(`No ${options.profile} build output found under target/.`);
    process.exit(0);
  }
  const table = markdownTable(rows);
  console.log(table);
  if (process.env.GITHUB_STEP_SUMMARY) {
    appendFileSync(process.env.GITHUB_STEP_SUMMARY, `\n### Package sizes\n\n${table}\n`);
  }
}
