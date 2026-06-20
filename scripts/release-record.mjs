import { createHash } from "node:crypto";
import { execFile } from "node:child_process";
import { mkdir, readdir, readFile, stat, writeFile } from "node:fs/promises";
import { basename, dirname, join, relative, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
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
    validate: null,
    artifactManifests: [],
    releaseIndex: null,
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
      case "--validate":
      case "--record":
        options.validate = next();
        break;
      case "--artifact-manifest":
      case "--artifact-manifests":
      case "--release-artifacts":
      case "--updater-artifacts":
        options.artifactManifests.push(next());
        break;
      case "--release-index":
        options.releaseIndex = next();
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
       node scripts/release-record.mjs --validate <record.md> --artifact-manifests <dir> --release-index <file>

Writes a fillable stable release record seeded with the current version, branch,
commit, and repository status. The record is evidence scaffolding only; it does
not publish artifacts or approve release gates. The validation mode checks a
completed record against artifact-manifest.json files and release-index.json,
and exits non-zero when required cells are blank or hashes do not match.

Options:
  --out <file>                 Output markdown path. Default: dist/release/stable-release-record.md
  --version <version>          Release version. Default: package.json version
  --channel <name>             Release channel. Default: stable
  --workflow-url <url>         GitHub Actions Release workflow run URL
  --evidence-tracker <id|url>  Release issue, tracker, or evidence packet ID
  --previous-stable <version>  Previous stable release version
  --stdout                     Print markdown instead of writing it
  --validate <file>            Validate a completed stable release record
  --artifact-manifests <path>  artifact-manifest.json file or directory root. Repeatable
  --release-index <file>       release-index.json used for CDN staging evidence`);
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

function isStableTarget(target) {
  return stableTargets.includes(target);
}

function sha256Bytes(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

async function sha256File(path) {
  return sha256Bytes(await readFile(path));
}

function splitMarkdownRow(line) {
  const text = line.trim().replace(/^\|/, "").replace(/\|$/, "");
  const cells = [];
  let current = "";
  let escaped = false;

  for (const char of text) {
    if (char === "|" && !escaped) {
      cells.push(current.trim().replaceAll("\\|", "|"));
      current = "";
      escaped = false;
      continue;
    }

    current += char;
    escaped = char === "\\" && !escaped;
    if (char !== "\\") {
      escaped = false;
    }
  }

  cells.push(current.trim().replaceAll("\\|", "|"));
  return cells;
}

function isMarkdownSeparator(line) {
  return splitMarkdownRow(line).every((cell) => /^:?-{3,}:?$/.test(cell.trim()));
}

export function parseMarkdownTables(markdown) {
  const tables = new Map();
  const lines = markdown.split(/\r?\n/);
  let section = null;

  for (let index = 0; index < lines.length; index += 1) {
    const heading = /^##\s+(.+?)\s*$/.exec(lines[index]);
    if (heading) {
      section = heading[1].trim();
      continue;
    }

    if (!section || !lines[index].trim().startsWith("|")) {
      continue;
    }

    const tableLines = [];
    while (index < lines.length && lines[index].trim().startsWith("|")) {
      tableLines.push(lines[index]);
      index += 1;
    }
    index -= 1;

    if (tableLines.length < 2 || !isMarkdownSeparator(tableLines[1])) {
      continue;
    }

    const headers = splitMarkdownRow(tableLines[0]);
    const rows = tableLines.slice(2).map((line) => {
      const cells = splitMarkdownRow(line);
      return Object.fromEntries(headers.map((header, cellIndex) => [header, cells[cellIndex] ?? ""]));
    });
    const sectionTables = tables.get(section) ?? [];
    sectionTables.push({ headers, rows });
    tables.set(section, sectionTables);
  }

  return tables;
}

function firstTable(tables, section, failures) {
  const table = tables.get(section)?.[0] ?? null;
  if (!table) {
    failures.push(`${section}: required table is missing`);
  }
  return table;
}

function isBlank(value) {
  return String(value ?? "").trim().length === 0;
}

function requiredCell(failures, section, rowLabel, row, field) {
  const value = row[field];
  if (isBlank(value)) {
    failures.push(`${section}: ${rowLabel} has blank ${field}`);
  }
  return String(value ?? "").trim();
}

function requireTableCells(failures, tables, section, labelField, requiredFields) {
  const table = firstTable(tables, section, failures);
  if (!table) {
    return [];
  }

  for (const row of table.rows) {
    const label = row[labelField] || row[table.headers[0]] || "row";
    for (const field of requiredFields) {
      requiredCell(failures, section, label, row, field);
    }
  }

  return table.rows;
}

function validateRequiredRecordFields(markdown, tables) {
  const failures = [];

  const unchecked = [...markdown.matchAll(/^- \[ \]\s+(.+)$/gim)].map((match) => match[1].trim());
  for (const label of unchecked) {
    failures.push(`Required Command Evidence: checkbox is not checked: ${label}`);
  }

  const headerRows = requireTableCells(failures, tables, "Header", "Field", ["Field", "Value"]);
  for (const row of headerRows) {
    const fieldName = String(row.Field ?? "").trim();
    if (fieldName.endsWith("SHA-256") && !/^n\/a$/i.test(String(row.Value ?? "").trim())) {
      const value = requiredCell(failures, "Header", fieldName, row, "Value").toLowerCase();
      if (value && !/^[a-f0-9]{64}$/.test(value)) {
        failures.push(`Header: ${fieldName} is not a valid SHA-256`);
      }
    }
  }

  requireTableCells(failures, tables, "Required Command Evidence", "Command", [
    "Command",
    "Runner or machine",
    "Started at",
    "Finished at",
    "Result",
    "Evidence path or hash",
  ]);
  requireTableCells(failures, tables, "Artifact Evidence", "Target", [
    "Target",
    "Package artifact name",
    "Package SHA-256",
    "Updater payload",
    "Updater SHA-256",
    "Signature artifact",
    "Signature SHA-256",
    "Smoke evidence",
  ]);
  requireTableCells(failures, tables, "CDN Pointer Evidence", "Pointer", [
    "Pointer",
    "Previous URL",
    "Previous SHA-256",
    "Staged URL",
    "Staged SHA-256",
    "Active URL after promotion",
    "Active SHA-256 after promotion",
    "Probe evidence",
  ]);
  requireTableCells(failures, tables, "External Gate Checklist", "Gate", [
    "Gate",
    "Default owner",
    "Status",
    "Completed by",
    "Completed at",
    "Evidence links or hashes",
  ]);
  requireTableCells(failures, tables, "Go / No-Go Decision", "Field", ["Field", "Value"]);
  requireTableCells(failures, tables, "Promotion Log", "Step", ["Step", "Operator", "Timestamp", "Result", "Evidence"]);

  return failures;
}

async function walkArtifactManifests(root) {
  let rootStat;
  try {
    rootStat = await stat(root);
  } catch (error) {
    if (error && error.code === "ENOENT") {
      return [];
    }
    throw error;
  }

  if (rootStat.isFile()) {
    return basename(root) === "artifact-manifest.json" ? [root] : [];
  }

  const entries = await readdir(root, { withFileTypes: true });
  const manifests = [];

  for (const entry of entries) {
    if (entry.isSymbolicLink()) {
      continue;
    }

    const path = join(root, entry.name);
    if (entry.isDirectory()) {
      manifests.push(...(await walkArtifactManifests(path)));
    } else if (entry.isFile() && entry.name === "artifact-manifest.json") {
      manifests.push(path);
    }
  }

  return manifests.sort((left, right) => left.localeCompare(right));
}

function addArtifactIndexEntry(index, key, entry) {
  if (!key || typeof key !== "string") {
    return;
  }

  const normalized = key.trim();
  if (!normalized) {
    return;
  }

  const entries = index.get(normalized) ?? [];
  entries.push(entry);
  index.set(normalized, entries);
}

function indexArtifactEntries(entries) {
  const index = new Map();
  for (const entry of entries) {
    addArtifactIndexEntry(index, entry.name, entry);
    addArtifactIndexEntry(index, entry.path, entry);
    addArtifactIndexEntry(index, entry.originalName, entry);
  }
  return index;
}

async function loadArtifactManifestEntries(roots) {
  const manifestPaths = [];
  for (const root of roots) {
    manifestPaths.push(...(await walkArtifactManifests(resolve(repoRoot, root))));
  }
  if (manifestPaths.length === 0) {
    throw new Error("No artifact-manifest.json files found for release record validation");
  }

  const entries = [];
  for (const manifestPath of manifestPaths) {
    const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
    if (!Array.isArray(manifest.artifacts)) {
      throw new Error(`${relative(repoRoot, manifestPath)} is missing artifacts[]`);
    }

    for (const artifact of manifest.artifacts) {
      entries.push({
        ...artifact,
        manifestTarget: manifest.target,
        manifestPath,
      });
    }
  }

  return {
    entries,
    byName: indexArtifactEntries(entries),
  };
}

function releaseTargetForIndexArtifact(artifact) {
  if (isStableTarget(artifact.releaseTarget)) {
    return artifact.releaseTarget;
  }

  const target = String(artifact.target ?? "").toLowerCase();
  const arch = String(artifact.arch ?? "").toLowerCase();
  if (target === "macos" && arch === "x64") {
    return "darwin-x86_64";
  }
  if (target === "macos" && arch === "arm64") {
    return "darwin-aarch64";
  }
  if (target === "windows" && arch === "x64") {
    return "windows-x86_64";
  }
  if (target === "windows" && arch === "arm64") {
    return "windows-aarch64";
  }
  if (target === "linux" && arch === "x64") {
    return "linux-x86_64";
  }
  if (target === "linux" && arch === "arm64") {
    return "linux-aarch64";
  }
  return null;
}

async function loadReleaseIndexEntries(releaseIndexPath) {
  if (!releaseIndexPath) {
    throw new Error("--release-index is required when validating a release record");
  }

  const path = resolve(repoRoot, releaseIndexPath);
  const index = JSON.parse(await readFile(path, "utf8"));
  if (!Array.isArray(index.artifacts)) {
    throw new Error(`${releaseIndexPath} is missing artifacts[]`);
  }

  const entries = index.artifacts.map((artifact) => ({
    ...artifact,
    releaseTarget: releaseTargetForIndexArtifact(artifact),
  }));

  return {
    path,
    entries,
    byName: indexArtifactEntries(entries),
    sha256: await sha256File(path),
  };
}

function artifactMatchesTarget(artifact, target) {
  return [artifact.target, artifact.releaseTarget, artifact.manifestTarget].some((value) => value === target);
}

function findArtifact(index, name, target) {
  const candidates = index.get(name) ?? [];
  return candidates.find((artifact) => artifactMatchesTarget(artifact, target)) ?? candidates[0] ?? null;
}

function normalizedSha(value) {
  return String(value ?? "").trim().toLowerCase();
}

function compareRecordArtifact(failures, sourceLabel, index, target, name, hash) {
  const artifact = findArtifact(index, name, target);
  if (!artifact) {
    failures.push(`Artifact Evidence: ${target} ${sourceLabel} ${name} is missing from source metadata`);
    return;
  }

  const expectedHash = normalizedSha(hash);
  if (!/^[a-f0-9]{64}$/.test(expectedHash)) {
    failures.push(`Artifact Evidence: ${target} ${sourceLabel} hash is not a valid SHA-256`);
    return;
  }

  if (normalizedSha(artifact.sha256) !== expectedHash) {
    failures.push(
      `Artifact Evidence: ${target} ${sourceLabel} hash mismatch for ${name}: record ${expectedHash}, metadata ${artifact.sha256}`,
    );
  }
}

function validateArtifactEvidenceRows(rows, manifestEvidence, releaseIndexEvidence) {
  const failures = [];
  const presentTargets = new Set(rows.map((row) => String(row.Target ?? "").trim()).filter(Boolean));
  const missingTargets = stableTargets.filter((target) => !presentTargets.has(target));
  if (missingTargets.length > 0) {
    failures.push(`Artifact Evidence: missing stable target row(s): ${missingTargets.join(", ")}`);
  }

  for (const row of rows) {
    const target = String(row.Target ?? "").trim();
    const checks = [
      ["package artifact", row["Package artifact name"], row["Package SHA-256"]],
      ["updater payload", row["Updater payload"], row["Updater SHA-256"]],
      ["signature artifact", row["Signature artifact"], row["Signature SHA-256"]],
    ];

    for (const [label, nameValue, hashValue] of checks) {
      const name = String(nameValue ?? "").trim();
      compareRecordArtifact(failures, label, manifestEvidence.byName, target, name, hashValue);
      compareRecordArtifact(failures, label, releaseIndexEvidence.byName, target, name, hashValue);
    }
  }

  return failures;
}

function validatePointerEvidenceRows(rows, releaseIndexEvidence) {
  const failures = [];
  const releaseIndexRow = rows.find((row) => String(row.Pointer ?? "").trim() === "release-index.json");
  if (!releaseIndexRow) {
    failures.push("CDN Pointer Evidence: release-index.json row is missing");
    return failures;
  }

  for (const field of ["Staged SHA-256", "Active SHA-256 after promotion"]) {
    const value = normalizedSha(releaseIndexRow[field]);
    if (!/^[a-f0-9]{64}$/.test(value)) {
      failures.push(`CDN Pointer Evidence: release-index.json ${field} is not a valid SHA-256`);
    } else if (value !== releaseIndexEvidence.sha256) {
      failures.push(
        `CDN Pointer Evidence: release-index.json ${field} mismatch: record ${value}, file ${releaseIndexEvidence.sha256}`,
      );
    }
  }

  return failures;
}

export async function validateReleaseRecordText(markdown, options) {
  const failures = [];
  const tables = parseMarkdownTables(markdown);

  failures.push(...validateRequiredRecordFields(markdown, tables));

  const artifactRows = firstTable(tables, "Artifact Evidence", failures)?.rows ?? [];
  const pointerRows = firstTable(tables, "CDN Pointer Evidence", failures)?.rows ?? [];
  const artifactManifestRoots = options.artifactManifests ?? [];
  if (artifactManifestRoots.length === 0) {
    failures.push("--artifact-manifests is required when validating a release record");
  }

  if (failures.length > 0) {
    return failures;
  }

  const [manifestEvidence, releaseIndexEvidence] = await Promise.all([
    loadArtifactManifestEntries(artifactManifestRoots),
    loadReleaseIndexEntries(options.releaseIndex),
  ]);

  failures.push(...validateArtifactEvidenceRows(artifactRows, manifestEvidence, releaseIndexEvidence));
  failures.push(...validatePointerEvidenceRows(pointerRows, releaseIndexEvidence));

  return failures;
}

async function validateReleaseRecordFile(options) {
  const recordPath = resolve(repoRoot, options.validate);
  const markdown = await readFile(recordPath, "utf8");
  return validateReleaseRecordText(markdown, options);
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
  if (options.validate) {
    const failures = await validateReleaseRecordFile(options);
    if (failures.length > 0) {
      console.error("Stable release record validation failed:");
      for (const failure of failures) {
        console.error(`- ${failure}`);
      }
      process.exit(1);
    }
    console.log(`Stable release record validation passed for ${relative(repoRoot, resolve(repoRoot, options.validate))}`);
    return;
  }

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

function isCliEntrypoint() {
  return process.argv[1] ? pathToFileURL(resolve(process.argv[1])).href === import.meta.url : false;
}

if (isCliEntrypoint()) {
  main().catch((error) => {
    console.error(error.message);
    process.exit(1);
  });
}

export { buildReleaseRecord };
