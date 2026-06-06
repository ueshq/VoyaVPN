# Diagnostics Privacy Contract

Batch: `04-01-diagnostics-contract`

This contract defines the production stable diagnostics boundary for Phase 04. It is the implementation source of truth until privacy/security approval records a stricter replacement.

## Scope

Diagnostics are default-on for production stable, user-disableable, and limited to release health. The feature exists to answer whether app startup, updates, core acquisition, core apply, runtime start, and crash-class reporting are healthy across the supported Windows, macOS, and Linux x64/arm64 matrix.

Diagnostics must never be used as proxy analytics, traffic analytics, subscription analytics, or user behavior tracking. Events are best-effort and must not block app startup, update checks, downloads, core apply, runtime start/stop, UI actions, or shutdown.

## Default And Opt-Out Behavior

- Default: diagnostics are enabled on first launch for production stable.
- User control: the app must expose a visible opt-out setting before network delivery is considered release-ready.
- Persistence: disabling diagnostics persists across restarts and app updates.
- Disable effect: opt-out stops new event creation, stops network delivery, and clears any pending local diagnostics queue.
- Re-enable effect: the user may re-enable diagnostics from settings. Re-enable does not authorize any forbidden payload fields.
- Endpoint absence: if no approved endpoint is configured, network delivery is disabled even when the local diagnostics preference is enabled. Stable publication remains incomplete until the endpoint configuration and privacy approval are recorded.

## Event Envelope

Diagnostics are serialized through an allowlisted event envelope. Public constructors must not accept arbitrary maps, raw JSON blobs, free-form error strings, logs, URLs, config text, or caller-provided network identifiers.

Events are delivered as JSON over HTTPS:

| Field | Required | Allowed values or notes |
| --- | --- | --- |
| `schema_version` | Yes | Integer schema version. Initial implementation should use `1`. |
| `app_version` | Yes | App version from release metadata. This is the allowed app version field. |
| `release_channel` | Yes | `stable`, `beta`, `debug`, or another documented release channel. |
| `os` | Yes | `windows`, `macos`, or `linux`. This is the allowed OS/arch OS field. |
| `arch` | Yes | `x64` or `arm64`. This is the allowed OS/arch architecture field. |
| `anonymous_install_id` | Yes | Random locally generated UUID or equivalent random id. This is the allowed anonymous install id field. It must not be derived from hardware ids, usernames, hostnames, MAC addresses, IP addresses, or account data. |
| `event_type` | Yes | Allowlisted event type such as `app_start`, `update_check`, `update_download`, `app_update_install`, `core_download`, `core_apply`, `runtime_start`, `runtime_stop`, `runtime_start_failure`, `core_missing`, `panic_class`, or `release_smoke`. This is the allowed event type field. |
| `result` | Yes | `success`, `failure`, `skipped`, `disabled`, or `dropped`. This is the allowed result field. |
| `error_class` | No | Coarse enum only, for example `network_unavailable`, `endpoint_unavailable`, `checksum_mismatch`, `signature_invalid`, `permission_denied`, `core_missing`, `runtime_start_failed`, `updater_install_failed`, `panic`, or `unknown`. This is the allowed error class field. Raw error messages are forbidden. |
| `subject_kind` | No | `app`, `xray`, `mihomo`, `sing_box`, `geo`, `srs`, or `runtime`. No node, subscription, or destination detail. |
| `duration_bucket_ms` | No | Coarse bucket only, such as `0-99`, `100-999`, `1000-4999`, `5000-29999`, or `30000_plus`. No exact timings are required. |
| `queue_depth_bucket` | No | Coarse bucket only, such as `0`, `1-9`, `10-49`, or `50_plus`. |
| `retry_count_bucket` | No | Coarse bucket only, such as `0`, `1`, `2-3`, or `4_plus`. |
| `occurred_at_minute_utc` | No | UTC timestamp truncated to minute precision. Do not include timezone, locale, or wall-clock offset. |

Any new field requires updating this document, adding focused tests, and privacy/security owner approval before it is allowed in production stable.

## Forbidden Fields

The following fields and payloads are forbidden in diagnostics, including nested fields, event names, error text, queue storage, request headers, endpoint URLs, and local evidence:

- Node URLs, share links, server links, proxy links, and raw outbound definitions.
- Subscription URLs, subscription headers, subscription names that contain service or account identifiers, and subscription response bodies.
- Credentials, tokens, passwords, API keys, cookies, bearer headers, private keys, updater signing keys, WebDAV credentials, proxy credentials, and embedded user secrets.
- IP addresses and IPs of any kind, including local IPs, public IPs, proxy IPs, DNS resolver IPs, endpoint IPs, destination IPs, and subnet/CIDR values.
- Full logs, log excerpts that include raw errors, process output, core stdout/stderr, panic payload text, or support bundles.
- Generated configs for Xray, sing-box, mihomo, TUN, DNS, routing, rulesets, or system proxy state.
- Traffic destinations, destination hostnames, domains, SNI values, DNS queries, URLs visited through the proxy, ports, protocols, HTTP headers, and request paths.
- Hardware identifiers, MAC addresses, serial numbers, device names, usernames, home directory paths, account ids, email addresses, locale-derived identity, or precise geolocation.
- Raw file paths outside documented release artifact names, because local paths can include usernames or project names.

Redaction must happen before queueing, persistence, serialization, logging, or network delivery. The preferred control is construction by allowlist; redaction is a defense-in-depth step, not permission to accept arbitrary payloads.

## Bounded Queueing

- Diagnostics are best-effort. Failure to enqueue or deliver an event must not surface as a user-facing error.
- The default local queue limit is 100 events or 64 KiB, whichever is reached first.
- Batch delivery should send at most 25 events per request.
- Queue overflow drops the oldest pending diagnostics events and may enqueue one `dropped` result event if doing so stays within the same allowlist.
- Pending events expire after 24 hours.
- Opt-out clears the queue immediately.
- Endpoint failures use bounded retry and backoff. Repeated failures must not create an unbounded retry loop, battery drain, or startup delay.

## Endpoint Configuration

- Production stable diagnostics use a configured HTTPS JSON POST endpoint from release configuration, for example `VOYAVPN_DIAGNOSTICS_ENDPOINT` or an equivalent generated release overlay.
- The committed repo must not contain a production secret, endpoint token tied to a user, or any user credential.
- Stable endpoint configuration must use an approved Voya-operated HTTPS host. Stable builds must reject HTTP, local file URLs, localhost endpoints, source-control hosts, and unapproved mirrors.
- The endpoint accepts only the allowlisted JSON envelope described above. It must not set or require cookies.
- If an ingest key is required, it must be a non-user release ingest key approved by privacy/security. User credentials, subscription credentials, and proxy credentials are never allowed in request headers or URLs.
- `2xx` responses mark a batch delivered. `4xx` responses drop the batch after recording a coarse local delivery failure. `5xx`, timeout, and offline responses retry only within the bounded queue policy.

## Retention Assumptions

These retention assumptions are release gates until the privacy/security owner approves the final endpoint policy:

- Client pending queue retention is at most 24 hours.
- Raw server-side diagnostics event retention is at most 30 days.
- Aggregated release health counts may be retained up to 180 days.
- Diagnostics payloads are not joined to accounts, subscriptions, traffic data, payment data, support tickets, or advertising systems.
- Transport-level access logs are not diagnostics payloads. Endpoint operators must disable, truncate, or minimize source IP logging where available. If infrastructure requires short security logs, they must not be joined to anonymous install ids and should be retained for no more than 7 days.
- Opt-out state is not overridden by remote configuration or app update.

## Implementation Requirements

- Store `anonymous_install_id` in app configuration as a random value. Do not derive it from hardware or OS identity.
- Keep diagnostics settings separate from update preferences so update checks can run when diagnostics are disabled.
- Implement event constructors around enums and typed fields, not open-ended JSON.
- Add tests that prove default-on settings, opt-out persistence, queue bounds, endpoint failure behavior, and forbidden field exclusion.
- Add tests with sensitive fixtures containing node URLs, subscription URLs, credentials, IP addresses, full logs, generated configs, and traffic destinations. None of those values may appear in serialized diagnostics or local queue storage.

## Evidence

- Diagnostics privacy doc path: `docs/release/diagnostics-privacy.md`
- Later Phase 04 batches must record implementation and test output in `docs/verification/updates.md` or a dedicated diagnostics verification document.
