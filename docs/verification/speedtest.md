# Speedtest And UDP Verification

Batch: `06-02-speedtest-udptest`

## Automated Coverage

- `voya-udptest` implements SOCKS5 UDP associate packet framing, channel setup, target parsing, and NTP, DNS, STUN, and MCBE probe packets.
- UDP tests use packet fixtures and a local in-process SOCKS5 UDP relay; they do not call public DNS, NTP, STUN, or game servers.
- `voya-app::speedtest` covers all six `SpeedActionType` values: Tcping, Realping, UdpTest, Speedtest, Mixedtest, and FastRealping.
- Mixedtest is fixture-tested to run realping, download speed, and UDP in one action and to persist delay, speed, message, and IP info through `ProfileExItem`.
- Cancellation is fixture-tested with an injected blocking probe and stops the active job before later profiles or follow-up probes run.

## IPC And UI

- Tauri commands: `run_speedtest`, `cancel_speedtest`, and `speedtest_status`.
- Transient event: `speedtestResult`, used by the profile table for live delay, speed, message, and IP info updates.
- The profile table exposes actions for Fast, TCP, Real, UDP, Speed, Mixed, and Stop.

## External Checks

Real proxy-runtime speed tests are not part of the deterministic automated checks for this batch because they require a running core, reachable remote targets, and real network conditions. Follow-up smoke should run from `pnpm tauri dev` with a known working profile and verify that TCP, realping, UDP, speed, mixed, and cancel update the same profile rows.
