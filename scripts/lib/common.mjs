import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import { readJson } from "./fs.mjs";

export function truthy(value) {
  return /^(1|true|yes|on)$/i.test(String(value ?? "").trim());
}

export function falsey(value) {
  return /^(0|false|no|off)$/i.test(String(value ?? "").trim());
}

export function requireDarwin(message) {
  if (process.platform !== "darwin") {
    throw new Error(message);
  }
}

export function repoRootFromScript(importMetaUrl = import.meta.url) {
  let directory = dirname(fileURLToPath(importMetaUrl));

  while (!existsSync(resolve(directory, "pnpm-workspace.yaml"))) {
    const parent = dirname(directory);
    if (parent === directory) {
      throw new Error(`Unable to locate repository root from ${importMetaUrl}`);
    }
    directory = parent;
  }

  return directory;
}

export function isCliEntrypoint(importMetaUrl) {
  return process.argv[1] ? pathToFileURL(resolve(process.argv[1])).href === importMetaUrl : false;
}

export async function readPackageVersion(repoRoot) {
  return readJson(resolve(repoRoot, "package.json")).version;
}

// Only long flags: short flags such as `-p` mean "package" to cargo and
// "keychain profile" to notarytool, and masking them would hide useful context.
const secretFlagPattern = /^--(?:pass|passwd|password|api-key|apikey|token|secret|apple-id|keychain-password)$/i;

/**
 * Masks credential values so a failed command can be reported without leaking
 * the secret into stderr, CI logs, or terminal scrollback.
 */
function redactArgs(args) {
  const redacted = [];
  let maskNext = false;

  for (const arg of args) {
    const text = String(arg);
    if (maskNext) {
      redacted.push("***");
      maskNext = false;
      continue;
    }

    const separator = text.indexOf("=");
    const flag = separator === -1 ? text : text.slice(0, separator);
    if (!secretFlagPattern.test(flag)) {
      redacted.push(text);
      continue;
    }
    if (separator === -1) {
      redacted.push(text);
      maskNext = true;
    } else {
      redacted.push(`${flag}=***`);
    }
  }

  return redacted;
}

export function describeCommand(program, args) {
  return `${program} ${redactArgs(args).join(" ")}`.trim();
}

export function capture(program, args, options = {}) {
  return spawnSync(program, args, {
    ...options,
    encoding: "utf8",
  });
}

/** Capture a required command, preserving output and reporting failures safely. */
export function checkedCapture(program, args, options = {}, captureCommand = capture) {
  const result = captureCommand(program, args, options);
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    throw commandFailure(program, args, result);
  }
  return result;
}

export function run(program, args, options = {}) {
  const result = capture(program, args, {
    ...options,
    stdio: "inherit",
  });
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    throw new Error(`${describeCommand(program, args)} failed with status ${result.status}`);
  }
  return result;
}

/** Run a required command and exit the process with its status on failure. */
export function runOrExit(program, args, options = {}) {
  const { log, ...spawnOptions } = options;
  if (log !== false) {
    console.log(`$ ${describeCommand(program, args)}`);
  }
  const result = capture(program, args, {
    ...spawnOptions,
    stdio: spawnOptions.stdio ?? "inherit",
  });
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
  return result;
}

/** The first non-empty value of `names` in `env`, matched case-insensitively. */
export function environmentValue(env, ...names) {
  const wanted = new Set(names.map((name) => name.toLowerCase()));
  for (const [name, value] of Object.entries(env ?? {})) {
    if (wanted.has(name.toLowerCase()) && String(value ?? "").trim()) {
      return String(value).trim();
    }
  }
  return "";
}

export function commandFailure(program, args, result) {
  const detail = String(result.stderr || result.stdout || "").trim();
  return new Error(
    `${describeCommand(program, args)} failed with status ${result.status ?? "unknown"}${detail ? `: ${detail}` : ""}`,
  );
}

const sleeper = new Int32Array(new SharedArrayBuffer(Int32Array.BYTES_PER_ELEMENT));

/** Blocks the thread for `milliseconds`; for synchronous polling loops. */
export function sleepSync(milliseconds) {
  Atomics.wait(sleeper, 0, 0, milliseconds);
}

export function validateTiming(timeoutMs, pollIntervalMs) {
  if (!Number.isFinite(timeoutMs) || timeoutMs < 0) {
    throw new Error("timeoutMs must be a non-negative finite number.");
  }
  if (!Number.isFinite(pollIntervalMs) || pollIntervalMs <= 0) {
    throw new Error("pollIntervalMs must be a positive finite number.");
  }
}
