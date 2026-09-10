# macOS VPN and Manual System Proxy Acceptance

This flow changes VPN connections and requires the tester to edit system proxy
settings. Run it only in an explicitly approved test window. Do not interrupt
an existing VPN merely to run automated repository checks.

## Automated checks (no system network writes)

Run `pnpm run verify:local`. On macOS its Rust-test step also compiles and runs
`scripts/native/macos/test-bridge.mjs`: it injects statuses and a monotonic clock
into the same waiter as the native bridge. Its ten reconnect cycles are
simulations, not signed-package or real-traffic acceptance.

## Signed channel matrix

Repeat the following for the local Apple Development `.appex`, the notarized
Developer ID System Extension, and the App Store/TestFlight sandbox package.
Record macOS version, app revision, channel, architecture, signing identity,
provider bundle/path, observations and timestamps. Retain App Sandbox and the
channel's existing NetworkExtension entitlements. Use only the installed copy.

Before testing, record HTTP/HTTPS/SOCKS/PAC settings for all network services so
the tester can restore their original settings manually. Do not disclose proxy
credentials in evidence. Perform external `pnpm native:macos:ne:doctor` checks
when registration evidence is needed; sandbox discovery-unavailable output in
the app is not proof of a registration defect.

## VPN lifecycle

1. Enable TUN mode, then connect/disconnect ten consecutive times. Each success requires agreement
   between Home, `scutil --nc list`, current NetworkExtension state and actual
   traffic/exit-IP checks from a browser and a terminal. A provider log alone
   is insufficient.
2. During startup confirm initial Disconnected samples do not cause a false
   failure. Confirm the busy indicator settles after success or failure.
3. With a deliberately invalid test-provider configuration, verify startup
   failure triggers cleanup. No tunnel may remain after reported cleanup
   success. Preserve the original error and any cleanup error.
4. If stop times out, Home must show pending cleanup and offer Retry disconnect.
   Retry must address the retained session. Do not reconnect until ownership
   has been resolved. Check delayed provider starts during cleanup as well.

## Manual system proxy

1. Turn off TUN mode, connect the local core and copy its displayed
   address. Home must say Local proxy ready, separately showing the observed
   system configuration. Verify no proxy-setting script was created/executed.
2. Open Network settings using the app's fixed action. Follow the displayed
   navigation and configure HTTP/HTTPS or SOCKS manually for the test service,
   copying the bypass list as appropriate. Recheck should detect a local proxy.
3. Enable PAC. Its copyable URL must appear only after the local server starts;
   repeated status reads must keep the same URL. Fetch it and inspect the PAC
   response. Occupying the test PAC port must produce an error with no stale
   copyable PAC URL.
4. Enable TUN mode, disconnect and exit in separate trials.
   Each leaves manually configured OS proxies untouched and displays a cleanup
   reminder. Exit rechecks settings before terminating. A local proxy shows a
   potential loss-of-connectivity reminder; an unknown observation explains
   that settings could not be confirmed without claiming a network failure.
   The default action opens Network settings and leaves the app/connection
   running. Cancel/Escape also keeps it running; only Quit anyway exits.
   Repeat through tray Quit and Cmd+Q, including with the main window hidden.
   Verify all three buttons are localized in both Chinese locales. If opening
   settings fails, show the manual navigation path and keep the app running.
5. With a legacy `proxy-dirty` in the isolated test app-data directory, startup
   must not execute recovery scripts or remove the marker. Remove the local
   proxies manually, then click Check again or request exit; only a complete
   observation with no local proxy permits marker removal. After verified
   cleanup, exit must proceed without a stale reminder, including when the
   user returns from Network settings without clicking Check again first.
   A denied read stays Unknown and preserves the marker.
6. Repeat with a third-party proxy configured: report it separately and leave
   every third-party setting unchanged. A local proxy on any other network
   service must still prevent clearing the marker.
7. Restore the tester's original proxy settings manually and recheck.

## Cleanup and evidence

After copying, launching or testing any `.app` with the production PacketTunnel
appex, quit the test app and follow the existing NE hygiene flow:
`pnpm native:macos:ne:doctor --fix` (`--app <path>` for a non-default bundle,
`--dev` for the repository release bundle). Do this before deleting entitlement
fixtures. Do not add the mutating doctor command to CI or broad verification.

Attach per-channel results and record any unexecuted cases explicitly. A green
unit/mock suite does not substitute for this signed-package acceptance matrix.
