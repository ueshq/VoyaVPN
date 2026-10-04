import { run } from "../../lib/common.mjs";

/**
 * The Cargo profile a mobile build uses, and the directory under
 * `target/<triple>/` its output lands in.
 *
 * Any profile by name, from `VOYAVPN_RUST_PROFILE`. `dev` is the one whose
 * output directory is not named after it; `debug` is accepted as the name
 * that directory has.
 */
export function rustProfile(requested = process.env.VOYAVPN_RUST_PROFILE || "release") {
  const profile = requested === "debug" ? "dev" : requested;

  return { directory: profile === "dev" ? "debug" : profile, profile };
}

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
      "voya-uniffi-bindgen",
      "--features",
      "cli",
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
