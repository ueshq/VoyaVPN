export function lineSummary(lines, maxLines = 6) {
  const selected = lines.slice(0, maxLines).map((line) => `       ${line}`);
  if (lines.length > maxLines) {
    selected.push(`       ... ${lines.length - maxLines} more`);
  }
  return selected;
}

export class Reporter {
  constructor(mode) {
    this.mode = mode;
    this.records = [];
  }

  pass(name, details = []) {
    this.records.push({ status: "PASS", name, details });
  }

  /** A check that had nothing to examine here; it says nothing either way. */
  skip(name, details = []) {
    this.records.push({ status: "SKIP", name, details });
  }

  warn(name, details = []) {
    this.records.push({ status: "WARN", name, details });
  }

  fail(name, details = []) {
    this.records.push({ status: "FAIL", name, details });
  }

  blocker(name, details = []) {
    if (this.mode === "stable") {
      this.fail(name, details);
    } else {
      this.warn(`${name} (stable blocker, allowed in dry-run)`, details);
    }
  }

  print({ cdnBaseUrl, updatesBaseUrl, workDir }) {
    console.log(`VoyaVPN release readiness (${this.mode})`);
    console.log(`CDN base URL: ${cdnBaseUrl}`);
    console.log(`Updater base URL: ${updatesBaseUrl}`);
    console.log(`Generated output: ${workDir}`);
    console.log("");

    for (const record of this.records) {
      console.log(`[${record.status}] ${record.name}`);
      for (const detail of record.details) {
        console.log(`       ${detail}`);
      }
    }

    const counts = this.records.reduce(
      (current, record) => {
        current[record.status] += 1;
        return current;
      },
      { PASS: 0, WARN: 0, FAIL: 0, SKIP: 0 },
    );

    console.log("");
    const tally = `${counts.PASS} passed, ${counts.WARN} warnings, ${counts.FAIL} failed, ${counts.SKIP} skipped`;
    console.log(`Readiness result: ${counts.FAIL > 0 ? "FAIL" : "PASS"} (${tally})`);
  }

  hasFailures() {
    return this.records.some((record) => record.status === "FAIL");
  }
}
