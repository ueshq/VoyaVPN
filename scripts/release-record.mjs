import { execFile } from "node:child_process";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";

const execFileAsync = promisify(execFile);
const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const stableTargets = [
  "darwin-x86_64",
  "darwin-aarch64",
  "windows-x86_64",
  "windows-aarch64",
  "linux-x86_64",
  "linux-aarch64",
];
const gateRows = [
  ["Automated regression evidence handoff", "Release engineer"],
  ["CDN staging", "CDN owner"],
  ["Stable pointer promotion", "Release owner and CDN owner"],
  ["Updater signing and key custody", "Security owner and release engineer"],
  ["macOS signing and notarization", "macOS release owner"],
  ["Windows Authenticode signing", "Windows release owner"],
  ["Linux package verification", "Linux release owner"],
  ["Updater smoke", "Release engineer and platform owners"],
  ["Manual download smoke", "Platform owners"],
  ["Core smoke", "Platform owners and release engineer"],
  ["Diagnostics smoke", "Privacy/security owner and platform owners"],
  ["Legal redistribution approval", "Legal or release owner"],
  ["Privacy diagnostics approval", "Privacy/security owner"],
  ["Rollback readiness", "Release owner with CDN, security, platform, legal, and privacy owners"],
  ["Bad artifact quarantine readiness", "Release engineer and CDN owner"],
  ["Monitoring and rollback trigger watch", "Release owner"],
];

function parseArgs(argv) {
  const options = {
    output: "dist/release/stable-release-record.md",
    version: null,
    channel: "stable",
    workflowUrl: "",
    evidenceTracker: "",
    previousStable: "",
    stdout: false,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    const next = () => {
      const value = argv[index + 1];
      if (!value || value.startsWith("--")) {
        throw new Error(`${arg} requires a value`);
      }
      index += 1;
      return value;
    };

    switch (arg) {
      case "--out":
      case "--output":
        options.output = next();
        break;
      case "--version":
        options.version = next();
        break;
      case "--channel":
        options.channel = next();
        break;
      case "--workflow-url":
        options.workflowUrl = next();
        break;
      case "--evidence-tracker":
        options.evidenceTracker = next();
        break;
      case "--previous-stable":
        options.previousStable = next();
        break;
      case "--stdout":
        options.stdout = true;
        break;
      case "--help":
        printHelp();
        process.exit(0);
        break;
      default:
        throw new Error(`Unknown argument: ${arg}`);
    }
  }

  return options;
}

function printHelp() {
  console.log(`Usage: node scripts/release-record.mjs [options]

Writes a fillable stable release record seeded with the current version, branch,
commit, and repository status. The record is evidence scaffolding only; it does
not publish artifacts or approve release gates.

Options:
  --out <file>                 Output markdown path. Default: dist/release/stable-release-record.md
  --version <version>          Release version. Default: package.json version
  --channel <name>             Release channel. Default: stable
  --workflow-url <url>         GitHub Actions Release workflow run URL
  --evidence-tracker <id|url>  Release issue, tracker, or evidence packet ID
  --previous-stable <version>  Previous stable release version
  --stdout                     Print markdown instead of writing it`);
}

async function git(args) {
  try {
    const { stdout } = await execFileAsync("git", args, { cwd: repoRoot });
    return stdout.trim();
  } catch {
    return "";
  }
}

async function packageVersion() {
  const packageJson = JSON.parse(await readFile(resolve(repoRoot, "package.json"), "utf8"));
  return packageJson.version;
}

function field(value) {
  return value ? value.replaceAll("|", "\\|") : "";
}

function checkbox(label) {
  return `- [ ] ${label}`;
}

function stableTargetRows() {
  return stableTargets
    .map((target) => `| ${target} |  |  |  |  |  |  |  |  |`)
    .join("\n");
}

function gateChecklistRows() {
  return gateRows.map(([gate, owner]) => `| ${gate} | ${owner} |  |  |  |  |  |`).join("\n");
}

async function buildReleaseRecord(options) {
  const version = options.version ?? (await packageVersion());
  const [commit, branch, status, upstream] = await Promise.all([
    git(["rev-parse", "HEAD"]),
    git(["branch", "--show-current"]),
    git(["status", "--porcelain"]),
    git(["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"]),
  ]);
  const shortCommit = commit ? commit.slice(0, 12) : "";
  const statusSummary = status ? "dirty - resolve before tag or promotion" : "clean";
  const generatedAt = new Date().toISOString();

  return `# VoyaVPN Stable Release Record

This record is the release-owner evidence packet for one stable publication. It
must be completed before CDN stable pointer promotion. Secret values, private
keys, signing tokens, and account passwords must not be pasted into this file.

## Header

| Field | Value |
| --- | --- |
| Release version | ${field(version)} |
| Channel | ${field(options.channel)} |
| Frozen commit SHA | ${field(commit)} |
| Frozen short SHA | ${field(shortCommit)} |
| Branch | ${field(branch)} |
| Upstream | ${field(upstream)} |
| Worktree status at record generation | ${field(statusSummary)} |
| Generated at | ${field(generatedAt)} |
| Tag or release branch |  |
| GitHub Actions Release workflow run URL | ${field(options.workflowUrl)} |
| Evidence tracker or release issue ID | ${field(options.evidenceTracker)} |
| Final readiness artifact name |  |
| Final readiness artifact SHA-256 |  |
| Previous stable release version | ${field(options.previousStable)} |
| Previous stable release-index pointer hash |  |
| Previous stable latest.json pointer hash |  |
| Previous stable core, geo, and SRS pointer hashes |  |
| Release owner and backup owner |  |
| Rollback owner and backup owner |  |
| Monitoring window start and end |  |

## Required Command Evidence

${checkbox("pnpm run verify:ci")}
${checkbox("pnpm run build")}
${checkbox("pnpm run smoke:frontend")}
${checkbox("pnpm run check:release:dry-run")}
${checkbox("VOYAVPN_RELEASE_CHANNEL=stable pnpm tauri:stable-updater-config")}
${checkbox("pnpm run check:release:stable")}
${checkbox("pnpm run release:verify-staging -- --probe")}
${checkbox("pnpm run release:verify-staging -- --download-and-hash")}

| Command | Runner or machine | Started at | Finished at | Result | Evidence path or hash | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| pnpm run verify:ci |  |  |  |  |  |  |
| pnpm run build |  |  |  |  |  |  |
| pnpm run smoke:frontend |  |  |  |  |  |  |
| pnpm run check:release:dry-run |  |  |  |  |  |  |
| pnpm tauri:stable-updater-config |  |  |  |  |  |  |
| pnpm run check:release:stable |  |  |  |  |  |  |
| pnpm run release:verify-staging -- --probe |  |  |  |  |  |  |
| pnpm run release:verify-staging -- --download-and-hash |  |  |  |  |  |  |

## Artifact Evidence

| Target | Package artifact name | Package SHA-256 | Updater payload | Updater SHA-256 | Signature artifact | Signature SHA-256 | Smoke evidence | Notes |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
${stableTargetRows()}

## CDN Pointer Evidence

| Pointer | Previous URL | Previous SHA-256 | Staged URL | Staged SHA-256 | Active URL after promotion | Active SHA-256 after promotion | Probe evidence |
| --- | --- | --- | --- | --- | --- | --- | --- |
| release-index.json |  |  |  |  |  |  |  |
| latest.json |  |  |  |  |  |  |  |
| core-assets.json |  |  |  |  |  |  |  |
| geo manifest |  |  |  |  |  |  |  |
| SRS manifest |  |  |  |  |  |  |  |
| THIRD_PARTY_NOTICES.md |  |  |  |  |  |  |  |
| SHA256SUMS |  |  |  |  |  |  |  |

## External Gate Checklist

Use pass, blocked, or owner-approved skip. An owner-approved skip must record
why the gate does not apply and which owner accepted the residual risk.

| Gate | Default owner | Status | Completed by | Completed at | Evidence links or hashes | Stop, rollback, or residual risk notes |
| --- | --- | --- | --- | --- | --- | --- |
${gateChecklistRows()}

## Go / No-Go Decision

| Field | Value |
| --- | --- |
| Decision |  |
| Decision owner |  |
| Decision timestamp |  |
| Residual risks accepted |  |
| Rollback trigger thresholds confirmed |  |
| Monitoring owner active |  |
| Communication owner active |  |

## Promotion Log

| Step | Operator | Timestamp | Result | Evidence |
| --- | --- | --- | --- | --- |
| Freeze commit and tag |  |  |  |  |
| Stage immutable CDN objects |  |  |  |  |
| Verify staged objects |  |  |  |  |
| Promote stable pointers |  |  |  |  |
| Probe active stable pointers |  |  |  |  |
| Start monitoring window |  |  |  |  |
`;
}

async function main() {
  const options = parseArgs(process.argv.slice(2));
  const record = await buildReleaseRecord(options);

  if (options.stdout) {
    process.stdout.write(record);
    return;
  }

  const outputPath = resolve(repoRoot, options.output);
  await mkdir(dirname(outputPath), { recursive: true });
  await writeFile(outputPath, record);
  console.log(`Wrote stable release record to ${relative(repoRoot, outputPath).replaceAll("\\", "/")}`);
}

main().catch((error) => {
  console.error(error.message);
  process.exit(1);
});
