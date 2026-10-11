import { resolve } from "node:path";

import { writeJson } from "../lib/fs.mjs";

/**
 * LSMinimumSystemVersion of an Apple Silicon package. `tauri.conf.json`
 * declares the Intel floor (12.3); this is raised over it because the two
 * packages are built for different WebKits (`apps/desktop/vite.config.ts`):
 * the arm64 renderer keeps Vite's default target, Safari 16.4, which is the
 * system WKWebView from macOS 13.3. A lower declaration lets the app install
 * where its window stays blank.
 */
export const appleSiliconMinimumSystemVersion = "13.3";

function appleSiliconMinimumOverlay() {
  return { bundle: { macOS: { minimumSystemVersion: appleSiliconMinimumSystemVersion } } };
}

export function writeAppleSiliconMinimumOverlay({ repoRoot }) {
  const overlayPath = resolve(repoRoot, "target", "release-config", "tauri.macos-arm64.generated.json");
  writeJson(overlayPath, appleSiliconMinimumOverlay(), { onlyIfChanged: true });
  return overlayPath;
}
