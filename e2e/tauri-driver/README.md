# Tauri Driver Smoke Notes

This batch adds browser-level Playwright coverage in `e2e/smoke.spec.ts`. It runs against Vite with a browser-side Tauri IPC mock, so it is safe for non-interactive local and CI runs and does not mutate OS proxy, TUN, autostart, or hotkey state.

`tauri-driver` is intentionally not wired as a required automated gate yet. The current app flows under test either need fake backend state or touch OS surfaces that must be verified on real Windows, macOS, and Linux machines. Running `tauri-driver` locally is still useful after a platform build is available:

1. Install the platform WebDriver prerequisites for the current OS.
2. Install or expose `tauri-driver` on `PATH`.
3. Start the app with the Tauri dev or packaged build under the driver.
4. Re-run the same smoke flow from `e2e/smoke.spec.ts` against the WebDriver endpoint, replacing the browser IPC mock with the real Tauri runtime.

Exact manual checks and skipped automated surfaces are tracked in `docs/verification/cross-platform-smoke.md`.
