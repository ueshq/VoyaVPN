# Screen QR import acceptance

The Nodes toolbar has Add (Add node / Add subscription) and Import, in that order.
Clipboard text and screen QR imports execute directly; QR image imports retain the
editable dialog.

## Native acceptance (Windows, macOS, Linux X11 / Wayland)

1. Show two different node QR codes plus a duplicate on the desktop, including a
   secondary display when available. Include a high-DPI / mixed-scale setup.
2. Open Nodes > Import > Scan screen. After permission checks, VoyaVPN temporarily
   hides for capture and immediately returns before decoding. Verify restored
   visibility, maximized/full-screen state and focus; no app import dialog appears.
3. Verify all unique nodes appear, the first imported node is selected, and the
   page summary counts each node once. Repeat with separate Base64 and JSON QR
   payloads; each payload is parsed independently.
4. Mix an invalid QR payload with valid nodes: valid nodes still import and the
   page shows the failures. Try a blank desktop and an existing node as well.
5. Deny capture permissions or make a display unavailable. Verify a localized
   inline error, restored window, no new nodes for failed captures, and working
   image/file import alternatives. For partial capture failure, valid displays
   still import with a warning. No browser display-picker fallback is permitted.
6. Verify duplicate clicks are blocked through capture, import and list refresh.
   Simulated slow capture is covered by deterministic Rust tests: at 15 seconds
   the window returns; late results are discarded; the original worker retains
   the capture lock until it exits. Page unmount stops further frontend imports.

The operating system may present its own authorization UI. Wayland support
varies by compositor and portal; record desktop/session details and unavailable
results instead of treating a browser mock as native evidence. Screenshots are
not returned through IPC or retained by the app. Portal-owned temporary images
are removed by the capture library after reading.

## Build and verification

Linux build hosts require `pkg-config libclang-dev libxcb1-dev libxrandr-dev
libdbus-1-dev libpipewire-0.3-dev libwayland-dev libegl-dev`, in addition to the
existing Tauri requirements. CI and release workflows install these packages;
deb/rpm packages declare the corresponding runtime libraries.

Run `pnpm run verify:local`. CI's platform-check matrix supplies strict Clippy
checks on Windows and macOS, and the baseline-rust job on Linux. Native tests use
injected capture and window adapters plus generated QR pixels, so the Rust test
suite does not require a desktop session. Record real-device OS/version, display setup, permission
state, outcomes and logs with the release evidence. Any macOS packaged fixture
must follow the NetworkExtension cleanup rules in AGENTS.md.

## Local evidence (2026-09-12)

- `pnpm run verify:local` passed all gates, including 931 frontend unit tests,
  76 Playwright scenarios, generated IPC bindings, locale alignment and strict
  macOS workspace Clippy. A final format/Clippy run also passed after the
  Windows-only import cleanup.
- Windows `voya-platform --all-targets` strict cross-target Clippy passed.
  Full Windows workspace checking is pending CI: this macOS host lacks Windows
  SDK headers required by `aws-lc-sys`. Linux checking is pending a Linux runner.
- Native host: macOS 26.5.2 (25F84), Apple M2 Max; DELL U2414H at 1280×720 and
  built-in Retina at 3456×2234, configured as mirrored displays. Different QR
  contents on independent extended displays remain a release acceptance item.
- The isolated `app.voyavpn.qrqa` debug bundle contained no `Contents/PlugIns`.
  Its own configuration directory was used; synthetic `.example.test` nodes
  were never connected.
- Before screen recording access was available, Scan screen showed the inline
  permission error with no import dialog. After authorization became available,
  a blank capture showed “No QR code found.” and restored the window.
- A real desktop fixture with two unique QR payloads and one duplicate imported
  exactly two nodes, selected the first and showed “Imported 2 node(s).” inline.
  Scanning the same fixture again kept two nodes and reported two updates.
  The foreground test window returned after capture, with no app import dialog.
- Generated-pixel Rust tests cover distinct display frames, multiple codes,
  deduplication, partial capture failures, timeout, panic and cancellation.
  Native Windows/Linux, extended-display and full-screen restoration checks
  remain part of the release matrix.
