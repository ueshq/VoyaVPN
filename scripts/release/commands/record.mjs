import { execFile } from "node:child_process";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, relative, resolve } from "node:path";
import { promisify } from "node:util";
import { parseArgs } from "../../lib/args.mjs";
import { readJson, readPackageVersion, repoRootFromScript } from "../../lib/common.mjs";
import { stableTargets } from "../matrix.mjs";
import { sha256File, walkArtifactManifests } from "../validation.mjs";

const execFileAsync = promisify(execFile);
const repoRoot = repoRootFromScript(import.meta.url);
const recordReleaseTargets = stableTargets.map((target) => target.releaseTarget);
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
  ["Legal redistribution approval", "Legal or release owner"],
  ["Rollback readiness", "Release owner with CDN, security, platform, legal, and privacy owners"],
  ["Bad artifact quarantine readiness", "Release engineer and CDN owner"],
  ["Monitoring and rollback trigger watch", "Release owner"],
];

const argSpec = {
  "--out|--output": { key: "output" },
  "--version": { key: "version" },
  "--channel": { key: "channel" },
  "--workflow-url": { key: "workflowUrl" },
  "--evidence-tracker": { key: "evidenceTracker" },
  "--previous-stable": { key: "previousStable" },
  "--validate|--record": { key: "validate" },
  "--artifact-manifest|--artifact-manifests|--release-artifacts|--updater-artifacts": {
    key: "artifactManifests",
    list: true,
  },
  "--release-index": { key: "releaseIndex" },
  "--stdout": { key: "stdout", value: true },
};

function parseOptions(argv) {
  return parseArgs(argv, argSpec, {
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
  });
}

function printHelp() {
  console.log(`Usage: pnpm release -- record [options]
       pnpm release -- record --validate <record.md> --artifact-manifests <dir> --release-index <file>

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

function field(value) {
  return value ? value.replaceAll("|", "\\|") : "";
}

function checkbox(label) {
  return `- [ ] ${label}`;
}

function isStableTarget(target) {
  return recordReleaseTargets.includes(target);
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
    const manifest = readJson(manifestPath);
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
  return stableTargets.find((entry) => entry.os === target && entry.arch === arch)?.releaseTarget ?? null;
}

async function loadReleaseIndexEntries(releaseIndexPath) {
  if (!releaseIndexPath) {
    throw new Error("--release-index is required when validating a release record");
  }

  const path = resolve(repoRoot, releaseIndexPath);
  const index = readJson(path);
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
  const missingTargets = recordReleaseTargets.filter((target) => !presentTargets.has(target));
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
  return recordReleaseTargets
    .map((target) => `| ${target} |  |  |  |  |  |  |  |  |`)
    .join("\n");
}

function gateChecklistRows() {
  return gateRows.map(([gate, owner]) => `| ${gate} | ${owner} |  |  |  |  |  |`).join("\n");
}

async function buildReleaseRecord(options) {
  const version = options.version ?? (await readPackageVersion(repoRoot));
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

${checkbox("pnpm run verify:local")}
${checkbox("pnpm run build")}
${checkbox("pnpm run check:frontend:smoke:mock")}
${checkbox("pnpm run check:desktop:smoke")}
${checkbox("pnpm release -- readiness --mode dry-run")}
${checkbox("pnpm release -- updater-config")}
${checkbox("pnpm release -- readiness --mode stable")}
${checkbox("pnpm release -- verify-staging --probe")}
${checkbox("pnpm release -- verify-staging --download-and-hash")}

| Command | Runner or machine | Started at | Finished at | Result | Evidence path or hash | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| pnpm run verify:local |  |  |  |  |  |  |
| pnpm run build |  |  |  |  |  |  |
| pnpm run check:frontend:smoke:mock |  |  |  |  |  |  |
| pnpm run check:desktop:smoke |  |  |  |  |  |  |
| pnpm release -- readiness --mode dry-run |  |  |  |  |  |  |
| pnpm release -- updater-config |  |  |  |  |  |  |
| pnpm release -- readiness --mode stable |  |  |  |  |  |  |
| pnpm release -- verify-staging --probe |  |  |  |  |  |  |
| pnpm release -- verify-staging --download-and-hash |  |  |  |  |  |  |

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

async function main(argv = []) {
  const options = parseOptions(argv);
  if (options.help) {
    printHelp();
    return;
  }
  if (options.validate) {
    const failures = await validateReleaseRecordFile(options);
    if (failures.length > 0) {
      console.error("Stable release record validation failed:");
      for (const failure of failures) {
        console.error(`- ${failure}`);
      }
      throw new Error("Stable release record validation failed");
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

export { buildReleaseRecord, main, printHelp };
