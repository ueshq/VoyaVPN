import { relative } from "node:path";
import { capture, repoRootFromScript } from "../../lib/common.mjs";

const repoRoot = repoRootFromScript(import.meta.url);

/**
 * App Store Connect rejects a package that holds any file carrying
 * `com.apple.quarantine` (ITMS-91109), and `productbuild` keeps extended
 * attributes in the payload.
 *
 * The attribute reaches a bundle from downloaded inputs: every provisioning
 * profile saved from a browser is quarantined, and copies of it inherited the
 * source's quarantine event with a fresh timestamp.
 */
export const quarantineAttribute = "com.apple.quarantine";

/**
 * Paths under `root` that carry the quarantine attribute, parsed from
 * `xattr -r` output: one `path: attribute` line per attribute.
 */
export function quarantinedPaths(xattrListing, root) {
  const paths = new Set();
  for (const line of String(xattrListing ?? "").split(/\r?\n/u)) {
    // Attribute names never contain ": ", so the last one ends the path even
    // when the path itself contains one.
    const separator = line.lastIndexOf(": ");
    if (separator >= 0 && line.slice(separator + 2).trim() === quarantineAttribute) {
      paths.add(relative(root, line.slice(0, separator)) || ".");
    }
  }
  return [...paths];
}

/** Every quarantined path under `root`, relative to it. */
export function findQuarantined(root) {
  const listing = capture("xattr", ["-r", root], { cwd: repoRoot });
  if (listing.error) {
    throw listing.error;
  }
  if (listing.status !== 0) {
    throw new Error(`Unable to list extended attributes under ${root}: ${listing.stderr || listing.stdout}`);
  }
  return quarantinedPaths(listing.stdout, root);
}
