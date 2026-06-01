# Architecture Decision Records

Batch: `00-02-architecture-adrs`

These ADRs are baseline contracts for the VoyaVPN full rewrite. They are based on:

- `.agents/rollouts/voyavpn-full-rewrite/spec.md`
- `.agents/rollouts/voyavpn-full-rewrite/plan.md`
- `/Users/afu/.claude/plans/typescript-shadcn-tauri-silly-marble.md`
- `docs/source-inventory.md`
- `docs/fidelity-hotspots.md`
- read-only v2rayN reference areas under `/Users/afu/Dev/refs/v2rayN/v2rayN`

## Index

- [0001 - Target Architecture](0001-target-architecture.md)
- [0002 - Typed IPC And Events](0002-typed-ipc-and-events.md)
- [0003 - Config Generation Parity](0003-config-generation-parity.md)
- [0004 - Platform Boundaries](0004-platform-boundaries.md)

## Batch Verification Note

This documentation batch only verifies ADR presence. Cargo, pnpm, generated-binding drift, golden parity, core acceptance, and OS smoke checks are intentionally not run here because the Rust workspace, frontend package, IPC generator, golden fixtures, and packaged runtime do not exist yet. Follow-up batch `00-03-verification-scaffold-plan` must define those checks and their guarded skip behavior.
