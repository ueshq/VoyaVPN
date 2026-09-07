import { createHash } from "node:crypto";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

import { stableTargets } from "../matrix.mjs";
import { selectUpdaterPayload } from "../validation.mjs";
import { buildReleaseRecord, validateReleaseRecordText } from "./record.mjs";

const repoRoot = resolve(fileURLToPath(new URL("../../..", import.meta.url)));
const version = "0.1.0";
const recordTargets = stableTargets.map((target) => target.releaseTarget);

function targetPlatform(releaseTarget) {
  const target = stableTargets.find((entry) => entry.releaseTarget === releaseTarget);
  return target ? [target.os, target.arch] : null;
}

function table(headers, rows) {
  return [
    `| ${headers.join(" | ")} |`,
    `| ${headers.map(() => "---").join(" | ")} |`,
    ...rows.map((row) => `| ${headers.map((header) => row[header] ?? "").join(" | ")} |`),
  ].join("\n");
}

async function readManifest(root, target) {
  return JSON.parse(await readFile(resolve(repoRoot, root, target, "artifact-manifest.json"), "utf8"));
}

async function buildFixtureRecord(workDir) {
  const packageManifests = await Promise.all(
    recordTargets.map((target) => readManifest("tests/fixtures/release/artifacts", target)),
  );
  const updaterManifests = await Promise.all(
    recordTargets.map((target) => readManifest("tests/fixtures/release/signed-updater", target)),
  );
  const releaseIndexArtifacts = [...packageManifests, ...updaterManifests].flatMap((manifest) =>
    manifest.artifacts.map((artifact) => {
      const [target, arch] = targetPlatform(manifest.target);
      return {
        ...artifact,
        target,
        arch,
        releaseTarget: manifest.target,
        url: `https://cdn.voyavpn.dev/stable/${artifact.name}`,
      };
    }),
  );
  const releaseIndexText = `${JSON.stringify(
    {
      productName: "VoyaVPN",
      channel: "stable",
      version,
      baseUrl: "https://cdn.voyavpn.dev/stable",
      artifacts: releaseIndexArtifacts,
    },
    null,
    2,
  )}\n`;
  const releaseIndexPath = join(workDir, "release-index.json");
  await writeFile(releaseIndexPath, releaseIndexText);
  const releaseIndexSha = createHash("sha256").update(releaseIndexText).digest("hex");

  const artifactRows = recordTargets.map((target, index) => {
    const packageArtifact = packageManifests[index].artifacts[0];
    const updaterPayload = selectUpdaterPayload(updaterManifests[index].artifacts);
    const signature = updaterManifests[index].artifacts.find((artifact) => artifact.kind === "signature");
    return {
      Target: target,
      "Package artifact name": packageArtifact.name,
      "Package SHA-256": packageArtifact.sha256,
      "Updater payload": updaterPayload.name,
      "Updater SHA-256": updaterPayload.sha256,
      "Signature artifact": signature.name,
      "Signature SHA-256": signature.sha256,
      "Smoke evidence": `smoke-${target}`,
      Notes: "pass",
    };
  });

  const commands = [
    "pnpm run verify:local",
    "pnpm run build",
    "pnpm run check:frontend:smoke:mock",
    "pnpm run check:desktop:smoke",
    "pnpm release -- readiness --mode dry-run",
    "pnpm release -- updater-config",
    "pnpm release -- readiness --mode stable",
    "pnpm release -- verify-staging --probe",
    "pnpm release -- verify-staging --download-and-hash",
  ];

  const markdown = `# VoyaVPN Stable Release Record

## Header

${table(["Field", "Value"], [
  { Field: "Release version", Value: version },
  { Field: "Channel", Value: "stable" },
  { Field: "Frozen commit SHA", Value: "1".repeat(40) },
  { Field: "Frozen short SHA", Value: "1".repeat(12) },
  { Field: "Branch", Value: "main" },
  { Field: "Upstream", Value: "origin/main" },
  { Field: "Worktree status at record generation", Value: "clean" },
  { Field: "Generated at", Value: "2026-06-01T00:00:00.000Z" },
  { Field: "Tag or release branch", Value: "v0.1.0" },
  { Field: "GitHub Actions Release workflow run URL", Value: "https://github.com/voyavpn/voyavpn/actions/runs/1" },
  { Field: "Evidence tracker or release issue ID", Value: "REL-1" },
  { Field: "Final readiness artifact name", Value: "readiness.zip" },
  { Field: "Final readiness artifact SHA-256", Value: "a".repeat(64) },
  { Field: "Previous stable release version", Value: "n/a" },
  { Field: "Previous stable release-index pointer hash", Value: "n/a" },
  { Field: "Previous stable latest.json pointer hash", Value: "n/a" },
  { Field: "Previous stable core, geo, and SRS pointer hashes", Value: "n/a" },
  { Field: "Release owner and backup owner", Value: "release-owner" },
  { Field: "Rollback owner and backup owner", Value: "rollback-owner" },
  { Field: "Monitoring window start and end", Value: "2026-06-01/2026-06-02" },
])}

## Required Command Evidence

${commands.map((command) => `- [x] ${command}`).join("\n")}

${table(
  ["Command", "Runner or machine", "Started at", "Finished at", "Result", "Evidence path or hash", "Notes"],
  commands.map((command) => ({
    Command: command,
    "Runner or machine": "ci",
    "Started at": "2026-06-01T00:00:00Z",
    "Finished at": "2026-06-01T00:01:00Z",
    Result: "pass",
    "Evidence path or hash": "evidence",
    Notes: "none",
  })),
)}

## Artifact Evidence

${table(
  [
    "Target",
    "Package artifact name",
    "Package SHA-256",
    "Updater payload",
    "Updater SHA-256",
    "Signature artifact",
    "Signature SHA-256",
    "Smoke evidence",
    "Notes",
  ],
  artifactRows,
)}

## CDN Pointer Evidence

${table(
  [
    "Pointer",
    "Previous URL",
    "Previous SHA-256",
    "Staged URL",
    "Staged SHA-256",
    "Active URL after promotion",
    "Active SHA-256 after promotion",
    "Probe evidence",
  ],
  [
    {
      Pointer: "release-index.json",
      "Previous URL": "n/a",
      "Previous SHA-256": "n/a",
      "Staged URL": "https://cdn.voyavpn.dev/stable/release-index.json",
      "Staged SHA-256": releaseIndexSha,
      "Active URL after promotion": "https://cdn.voyavpn.dev/stable/release-index.json",
      "Active SHA-256 after promotion": releaseIndexSha,
      "Probe evidence": "probe",
    },
    ...["latest.json", "core-assets.json", "geo manifest", "SRS manifest", "THIRD_PARTY_NOTICES.md", "SHA256SUMS"].map(
      (pointer) => ({
        Pointer: pointer,
        "Previous URL": "n/a",
        "Previous SHA-256": "n/a",
        "Staged URL": "n/a",
        "Staged SHA-256": "n/a",
        "Active URL after promotion": "n/a",
        "Active SHA-256 after promotion": "n/a",
        "Probe evidence": "n/a",
      }),
    ),
  ],
)}

## External Gate Checklist

${table(
  ["Gate", "Default owner", "Status", "Completed by", "Completed at", "Evidence links or hashes", "Stop, rollback, or residual risk notes"],
  [
    {
      Gate: "Updater signing and key custody",
      "Default owner": "Security owner",
      Status: "pass",
      "Completed by": "security",
      "Completed at": "2026-06-01T00:00:00Z",
      "Evidence links or hashes": "evidence",
      "Stop, rollback, or residual risk notes": "none",
    },
  ],
)}

## Go / No-Go Decision

${table(["Field", "Value"], [
  { Field: "Decision", Value: "go" },
  { Field: "Decision owner", Value: "owner" },
  { Field: "Decision timestamp", Value: "2026-06-01T00:00:00Z" },
  { Field: "Residual risks accepted", Value: "none" },
  { Field: "Rollback trigger thresholds confirmed", Value: "yes" },
  { Field: "Monitoring owner active", Value: "yes" },
  { Field: "Communication owner active", Value: "yes" },
])}

## Promotion Log

${table(["Step", "Operator", "Timestamp", "Result", "Evidence"], [
  {
    Step: "Freeze commit and tag",
    Operator: "release",
    Timestamp: "2026-06-01T00:00:00Z",
    Result: "pass",
    Evidence: "tag",
  },
])}
`;

  return { markdown, releaseIndexPath };
}

describe("release record validation", () => {
  it("renders artifact evidence rows in stable matrix order", async () => {
    const markdown = await buildReleaseRecord({
      version,
      channel: "stable",
      workflowUrl: "",
      evidenceTracker: "",
      previousStable: "",
    });
    const artifactEvidence = markdown.slice(
      markdown.indexOf("## Artifact Evidence"),
      markdown.indexOf("## CDN Pointer Evidence"),
    );
    const targets = artifactEvidence
      .split("\n")
      .filter((line) => recordTargets.some((target) => line.startsWith(`| ${target} |`)))
      .map((line) => line.split("|")[1].trim());

    expect(targets).toEqual(recordTargets);
  });

  it("compares completed record artifact hashes against manifests and release-index", async () => {
    const workDir = await mkdtemp(join(tmpdir(), "voyavpn-release-record-"));

    try {
      const { markdown, releaseIndexPath } = await buildFixtureRecord(workDir);
      await expect(
        validateReleaseRecordText(markdown, {
          artifactManifests: ["tests/fixtures/release/artifacts", "tests/fixtures/release/signed-updater"],
          releaseIndex: releaseIndexPath,
        }),
      ).resolves.toEqual([]);

      const tampered = markdown.replace("2222222222222222222222222222222222222222222222222222222222222222", "f".repeat(64));
      const failures = await validateReleaseRecordText(tampered, {
        artifactManifests: ["tests/fixtures/release/artifacts", "tests/fixtures/release/signed-updater"],
        releaseIndex: releaseIndexPath,
      });
      expect(failures.some((failure) => failure.includes("hash mismatch"))).toBe(true);
    } finally {
      await rm(workDir, { force: true, recursive: true });
    }
  });
});
