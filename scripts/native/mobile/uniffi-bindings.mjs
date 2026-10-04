import { run } from "../../lib/common.mjs";

/**
 * Generates the `voya-mobile-ffi` bindings for one language.
 *
 * uniffi reads the built library rather than the source, so this runs after
 * the build: the bindings then describe the very symbols that library exports.
 * `outDir` is written into, never emptied — on Android it is the app's own
 * Kotlin source root.
 */
export function generateUniffiBindings({ language, library, outDir, repoRoot }) {
  run(
    "cargo",
    [
      "run",
      "-p",
      "voya-mobile-ffi",
      "--features",
      "bindgen",
      "--bin",
      "uniffi-bindgen",
      "--",
      "generate",
      "--library",
      library,
      "--language",
      language,
      "--out-dir",
      outDir,
    ],
    { cwd: repoRoot },
  );
}
