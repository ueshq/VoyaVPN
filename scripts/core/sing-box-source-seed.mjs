import { chmodSync, cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, relative } from "node:path";

import { capture, checkedCapture, isCliEntrypoint, repoRootFromScript, run } from "../lib/common.mjs";
import { sha256FileSync, writeJson } from "../lib/fs.mjs";
import { ensureSingBoxSource, singBoxSourceDir } from "../native/sing-box-source.mjs";
import {
  isMachOFile,
  machOImportProblems,
  parseLinkedLibraries,
  parseUndefinedSymbols,
} from "../native/macos/macho-imports.mjs";
import {
  DEFAULT_SING_BOX_VERSION,
  SING_BOX_SEED_MANIFEST,
  SING_BOX_SOURCE_BUILD_TAGS,
  SING_BOX_SOURCE_EXCLUDED_TAGS,
  SING_BOX_SOURCE_REPOSITORY,
  assertSingBoxVersion,
  sameSingBoxBuildTags,
  singBoxExecutableName,
  singBoxPinKey,
  singBoxSeedDir,
  singBoxSourcePinStatus,
} from "./sing-box-installer.mjs";

/**
 * Builds the bundled sing-box seed from the pinned upstream commit instead of
 * downloading the release archive, with upstream's own `make build` tag list.
 *
 * The Mac App Store build needs this: the upstream macOS release binary is
 * built with `with_naive_outbound`, which links Chromium's Cronet, and Cronet
 * imports the non-public `__kCFBundleNumericVersionKey` and links private
 * `/usr/lib/libpm*.dylib` libraries. The source is unmodified; the binary
 * differs only in its build tags. See docs/release/sing-box-seed-pinning.md.
 */

export const GO_TOOLCHAIN_ENV = "VOYAVPN_SING_BOX_GO_TOOLCHAIN";
const BUILD_SCRIPT = "scripts/core/sing-box-source-seed.mjs";
const SING_BOX_MAIN_PACKAGE = "./cmd/sing-box";

/**
 * The tags to build with, read from the checkout's
 * `release/DEFAULT_BUILD_TAGS_OTHERS` and required to equal the pinned list.
 * A mismatch means upstream changed its defaults at this tag, and the pin in
 * sing-box-installer.mjs has to be reviewed rather than followed.
 */
export function sourceBuildTags(defaultTagsText) {
  const tags = String(defaultTagsText ?? "")
    .split(",")
    .map((tag) => tag.trim())
    .filter(Boolean)
    .filter((tag) => !SING_BOX_SOURCE_EXCLUDED_TAGS.includes(tag));
  if (!sameSingBoxBuildTags(tags)) {
    throw new Error(
      `upstream release/DEFAULT_BUILD_TAGS_OTHERS is ${tags.join(",")}, but SING_BOX_SOURCE_BUILD_TAGS pins ` +
        `${SING_BOX_SOURCE_BUILD_TAGS.join(",")}. Review the upstream change and update the pin ` +
        "(docs/release/sing-box-seed-pinning.md).",
    );
  }
  return [...SING_BOX_SOURCE_BUILD_TAGS];
}

/**
 * `go build` arguments equivalent to upstream's `make build`. The version
 * string comes from the pin rather than upstream's `read_tag@latest`, which
 * would fetch a module from the network in the middle of a release build.
 */
export function goBuildArgs({ version, tags, ldflagsShared = "", output }) {
  const label = assertSingBoxVersion(version).replace(/^v/i, "");
  const ldflags = [
    `-X 'github.com/sagernet/sing-box/constant.Version=${label}'`,
    String(ldflagsShared).trim(),
    "-s -w -buildid=",
  ]
    .filter(Boolean)
    .join(" ");
  return ["build", "-trimpath", "-ldflags", ldflags, "-tags", tags.join(","), "-o", output, SING_BOX_MAIN_PACKAGE];
}

/**
 * The toolchain is the installed one unless the developer names another: an
 * implicit `GOTOOLCHAIN=auto` download would make the binary depend on
 * whatever Go the network served that day. cgo is required on macOS.
 */
export function goBuildEnv(env = process.env) {
  return {
    ...env,
    CGO_ENABLED: "1",
    GOTOOLCHAIN: env[GO_TOOLCHAIN_ENV]?.trim() || "local",
  };
}

/** The `Tags:` and `Revision:` lines of `sing-box version`. */
export function parseSingBoxVersionOutput(text) {
  const value = (label) => new RegExp(`^${label}:\\s*(.+)$`, "mu").exec(String(text ?? ""))?.[1]?.trim() ?? null;
  const version = /^sing-box version\s+(\S+)/mu.exec(String(text ?? ""))?.[1] ?? null;
  const tags = value("Tags");
  return {
    revision: value("Revision"),
    tags: tags ? tags.split(",").map((tag) => tag.trim()).filter(Boolean) : [],
    version,
  };
}

/** What `sing-box.seed.json` records for a source seed; see verifyStagedSingBoxSeed. */
export function sourceSeedManifest({
  bytes,
  builtAt = new Date().toISOString(),
  commit,
  executableSha256,
  goVersion,
  kept,
  ldflags,
  pinned,
  tags,
  target,
  version,
}) {
  return {
    bytes,
    buildScript: BUILD_SCRIPT,
    builtAt,
    commit,
    excludedTags: [...SING_BOX_SOURCE_EXCLUDED_TAGS],
    executableSha256,
    goVersion,
    kept,
    ldflags,
    origin: "source",
    pinned,
    sourceRepository: SING_BOX_SOURCE_REPOSITORY,
    tags,
    target,
    version,
  };
}

/** The Go toolchain the build will use, or a readable error before any work starts. */
export function requireGoToolchain({ env = process.env, captureCommand = capture } = {}) {
  const result = captureCommand("go", ["version"], { env: goBuildEnv(env) });
  const text = String(result?.stdout ?? "").trim();
  if (result?.error || result?.status !== 0 || !text.startsWith("go version ")) {
    throw new Error(
      "The Mac App Store build compiles the sing-box seed from source and needs Go on PATH " +
        "(brew install go). See docs/release/macos-app-store.md.",
    );
  }
  return text.replace(/^go version\s+/, "");
}

function macOSPublicApiProblems(binary, name) {
  if (!isMachOFile(binary)) {
    return [`${name} is not a Mach-O binary`];
  }
  return machOImportProblems({
    name,
    undefinedSymbols: parseUndefinedSymbols(checkedCapture("nm", ["-u", "-arch", "all", binary]).stdout),
    linkedLibraries: parseLinkedLibraries(checkedCapture("otool", ["-arch", "all", "-L", binary]).stdout),
  });
}

export async function buildAndStageSingBoxSeed({
  arch = process.arch,
  env = process.env,
  logger = console,
  platform = process.platform,
  repoRoot,
  version = env.SING_BOX_VERSION ?? DEFAULT_SING_BOX_VERSION,
} = {}) {
  const resolvedVersion = assertSingBoxVersion(version);
  if (platform !== process.platform || arch !== process.arch) {
    throw new Error(
      `The source-built sing-box seed is built for the host only (${process.platform}:${process.arch}), ` +
        `not ${platform}:${arch}.`,
    );
  }
  const target = singBoxPinKey({ arch, platform });
  if (!target) {
    throw new Error(`no sing-box target is configured for ${platform}:${arch}`);
  }
  const pin = singBoxSourcePinStatus({ env, version: resolvedVersion });
  if (!pin.pinned && !pin.unpinnedAllowed) {
    throw new Error(pin.reason);
  }

  const goVersion = requireGoToolchain({ env });
  const sourceDir = singBoxSourceDir(repoRoot, env);
  logger.log(`- sing-box: building ${resolvedVersion} from source (${goVersion}) in ${relative(repoRoot, sourceDir)}`);
  ensureSingBoxSource({ repoRoot, ref: resolvedVersion, sourceDir });

  const commit = checkedCapture("git", ["rev-parse", "HEAD"], { cwd: sourceDir }).stdout.trim().toLowerCase();
  if (pin.pinned && commit !== pin.expected) {
    throw new Error(`sing-box ${resolvedVersion} resolved to ${commit}, not the pinned ${pin.expected}`);
  }
  if (!pin.pinned) {
    logger.warn?.(`  ! unpinned source build: ${resolvedVersion} at ${commit}`);
  }

  const tags = sourceBuildTags(readFileSync(join(sourceDir, "release", "DEFAULT_BUILD_TAGS_OTHERS"), "utf8"));
  const ldflagsShared = readFileSync(join(sourceDir, "release", "LDFLAGS"), "utf8").trim();
  const executableName = singBoxExecutableName(platform);

  // Build outside the checkout so `ensureSingBoxSource` keeps finding it clean.
  const buildDir = mkdtempSync(join(tmpdir(), "voyavpn-sing-box-source-"));
  try {
    const output = join(buildDir, executableName);
    const args = goBuildArgs({ ldflagsShared, output, tags, version: resolvedVersion });
    run("go", args, { cwd: sourceDir, env: goBuildEnv(env) });

    const reported = parseSingBoxVersionOutput(checkedCapture(output, ["version"]).stdout);
    if (!sameSingBoxBuildTags(reported.tags) || reported.tags.some((tag) => SING_BOX_SOURCE_EXCLUDED_TAGS.includes(tag))) {
      throw new Error(`built sing-box reports tags ${reported.tags.join(",")}, expected ${tags.join(",")}`);
    }
    if (reported.revision && reported.revision.toLowerCase() !== commit) {
      throw new Error(`built sing-box reports revision ${reported.revision}, expected ${commit}`);
    }
    if (platform === "darwin") {
      const problems = macOSPublicApiProblems(output, executableName);
      if (problems.length) {
        throw new Error(`the source-built sing-box would fail App Store validation:\n  - ${problems.join("\n  - ")}`);
      }
    }

    const seedDir = singBoxSeedDir(repoRoot);
    rmSync(seedDir, { force: true, recursive: true });
    mkdirSync(seedDir, { recursive: true });
    const staged = join(seedDir, executableName);
    cpSync(output, staged);
    chmodSync(staged, statSync(staged).mode | 0o755);
    const kept = [executableName];
    const license = join(sourceDir, "LICENSE");
    if (existsSync(license)) {
      cpSync(license, join(seedDir, "LICENSE"));
      kept.unshift("LICENSE");
    }

    const executableSha256 = sha256FileSync(staged);
    writeJson(
      join(seedDir, SING_BOX_SEED_MANIFEST),
      sourceSeedManifest({
        bytes: statSync(staged).size,
        commit,
        executableSha256,
        goVersion,
        kept,
        ldflags: args[args.indexOf("-ldflags") + 1],
        pinned: pin.pinned,
        tags,
        target,
        version: resolvedVersion,
      }),
    );
    logger.log(`  ✓ staged source-built ${kept.join(", ")} -> ${relative(repoRoot, seedDir)}/`);
    logger.log(`  ✓ ${commit}, tags ${tags.join(",")}, SHA256 ${executableSha256}`);

    return { commit, executableSha256, kept, pinned: pin.pinned, seedDir, tags, version: resolvedVersion };
  } finally {
    rmSync(buildDir, { force: true, recursive: true });
  }
}

// `pnpm core:sing-box:build`: stage the source seed by hand, e.g. to inspect
// it before a store build (which builds it on its own when it is missing).
if (isCliEntrypoint(import.meta.url)) {
  try {
    await buildAndStageSingBoxSeed({ repoRoot: repoRootFromScript(import.meta.url) });
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(1);
  }
}
