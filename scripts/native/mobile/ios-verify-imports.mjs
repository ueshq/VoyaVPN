import { existsSync } from "node:fs";
import { resolve } from "node:path";

import { isCliEntrypoint } from "../../lib/common.mjs";
import { bundleImportReport } from "../macos/macho-imports.mjs";

/**
 * The iOS side of the Mac App Store's Guideline 2.5.1 gate: every Mach-O in a
 * built `.app` (the app, the PacketTunnel appex, embedded frameworks) must
 * import no symbol App Review has named as non-public and link no library
 * outside the SDK. Run it on the archived app before uploading:
 *
 *   pnpm native:mobile:ios:verify-imports <path/to/VoyaVPN.app>
 *
 * See docs/release/mobile-ios-signing.md.
 */
function main(argv) {
  const target = argv[0];
  if (!target) {
    throw new Error("Pass the built VoyaVPN.app (inside the .xcarchive, under Products/Applications/).");
  }
  const root = resolve(target);
  if (!existsSync(root)) {
    throw new Error(`${root} does not exist.`);
  }
  const { binaries, problems } = bundleImportReport(root);
  if (binaries.length === 0) {
    throw new Error(`${root} contains no Mach-O binaries.`);
  }
  if (problems.length) {
    throw new Error(`App Store validation would reject this app:\n${problems.map((line) => `  - ${line}`).join("\n")}`);
  }
  for (const name of binaries) {
    console.log(`✓ ${name}: public API only`);
  }
}

if (isCliEntrypoint(import.meta.url)) {
  try {
    main(process.argv.slice(2));
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(1);
  }
}
