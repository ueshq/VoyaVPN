import { createHash } from "node:crypto";
import { closeSync, createReadStream, openSync, readSync, readdirSync, readFileSync } from "node:fs";
import { readdir, readFile } from "node:fs/promises";
import { join, resolve } from "node:path";

/**
 * Filesystem helpers shared by the quality, release, and native scripts.
 * Recursive walks skip symlinks so a planted link cannot pull content from
 * outside the tree being scanned.
 */

/** Files under `directory`, recursively. Symlinks are skipped. */
export function walkFilesSync(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    if (entry.isSymbolicLink()) {
      return [];
    }
    const path = resolve(directory, entry.name);
    return entry.isDirectory() ? walkFilesSync(path) : [path];
  });
}

/**
 * Files under `root`, recursively. `match(name, path)` can filter entries;
 * directories that fail `matchDirectory` (default: always descend) are skipped.
 * Missing roots resolve to `[]`.
 */
export async function walkFiles(root, { match = () => true, matchDirectory = () => true } = {}) {
  let entries;
  try {
    entries = await readdir(root, { withFileTypes: true });
  } catch (error) {
    if (error && error.code === "ENOENT") {
      return [];
    }
    throw error;
  }

  const files = [];
  for (const entry of entries) {
    if (entry.isSymbolicLink()) {
      continue;
    }
    const path = join(root, entry.name);
    if (entry.isDirectory()) {
      if (matchDirectory(entry.name, path)) {
        files.push(...(await walkFiles(path, { match, matchDirectory })));
      }
    } else if (entry.isFile() && match(entry.name, path)) {
      files.push(path);
    }
  }
  return files;
}

export function sha256Text(value) {
  return createHash("sha256").update(value).digest("hex");
}

/** Synchronous whole-file hash. For small files and sync APIs (tunnel service). */
export function sha256FileSync(path) {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

/**
 * Hashes in fixed chunks instead of buffering the whole file. Stays available
 * as a sync entry point for callers such as the staged-seed verifier.
 */
export function sha256FileChunksSync(path, chunkBytes = 1024 * 1024) {
  const hash = createHash("sha256");
  const chunk = Buffer.allocUnsafe(chunkBytes);
  const fd = openSync(path, "r");
  try {
    let bytesRead;
    while ((bytesRead = readSync(fd, chunk, 0, chunk.length, null)) > 0) {
      hash.update(chunk.subarray(0, bytesRead));
    }
  } finally {
    closeSync(fd);
  }
  return hash.digest("hex");
}

export async function sha256File(path) {
  const hash = createHash("sha256");
  await new Promise((resolvePromise, rejectPromise) => {
    const stream = createReadStream(path);
    stream.on("data", (chunk) => hash.update(chunk));
    stream.on("error", rejectPromise);
    stream.on("end", resolvePromise);
  });
  return hash.digest("hex");
}

export function readJson(path) {
  return JSON.parse(readFileSync(path, "utf8"));
}

/** Async JSON read. Distinct name so it never shadows the sync `readJson`. */
export async function readJsonAsync(path) {
  return JSON.parse(await readFile(path, "utf8"));
}
