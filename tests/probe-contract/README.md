# Probe service contract

The self-hosted node's reachability check talks to the probe Worker in
`apps/probe`. These fixtures are the wire contract between the two sides:

- `crates/voya-net/src/probe/reachability.rs` serializes its request and
  decodes the responses from them.
- `apps/probe/test/contract.test.ts` validates the Worker's handler output and
  request parsing against the same files.

Changing a field means changing it here first; both suites fail until both
sides agree.
