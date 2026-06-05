# Orchestration Patterns

Use this reference when the initiative is more like a delivery program than a single implementation ticket. The runner produced by this skill drives **Claude Code** (`claude -p --dangerously-skip-permissions`) on each batch.

## What Complex Plans Usually Need

- current-state assessment or inventory
- target state or target architecture
- goals, non-goals, and explicit design principles
- phase gates instead of a flat task list
- baseline metrics and post-change success metrics
- risk, rollback, compatibility, or cutover notes
- external dependencies such as gateways, cloud resources, vendor systems, or sibling repos
- a clear split between automated code work and external or operational work

## Common Phase Shapes

- `00-baseline` or `00-discovery`: inventories, metrics, naming cleanup, constraints, ADRs
- `01-foundation`: shared infra, contracts, tooling, scaffolding, rules, metrics scripts
- `02-pilot`: first slice or first environment proving the direction
- `03-bulk-migration` or domain slices: repeated delivery batches
- `90-ops-handoff`: docs or checklists for cloud, gateway, deployment, or coordination steps that stay outside the automated runner
- `99-verification`: definition-of-done evidence and regression sweep

## Batch Kinds

- `analysis`: scanning, inventories, metrics baselines, compatibility reports
- `code`: implementation work with deterministic verification commands
- `docs`: specs, ADRs, migration guides, runbooks, checklists
- `verification`: end-to-end validation, evidence capture, CI or smoke checks

## What Belongs In Prose

- rationale and tradeoffs
- target architecture explanation
- migration sequencing narrative
- external coordination details
- rollback discussion
- unresolved questions

## What Must Be In YAML

- phase order and dependencies
- batch order and explicit `depends_on` edges
- deliverables and acceptance criteria
- verification commands
- files likely to change
- prompt context the runner must always inject

## Manual Or External Work

When a step touches cloud consoles, Kubernetes clusters, gateways, DNS, third-party platforms, or another team:

- keep it in prose or a separate ops checklist instead of a generated runner batch
- record owner, execution system, verification step, and rollback note
- do not generate an automated runner until those prerequisites are already satisfied or moved fully in-repo
- avoid pretending Claude Code can finish the real-world action if it cannot

## Metrics And Evidence

For large initiatives, include both:

- baseline metrics: current counts, scan outputs, inventory snapshots
- end-state evidence: commands, dashboards, screenshots, or validation docs proving the rollout worked

## Verification Command Hygiene

- Runner verification commands must exit on their own in a normal developer shell, not only under CI.
- Package-manager argument forwarding is part of the command contract. Test exact commands before baking them into YAML.
- For Vitest under pnpm, use `pnpm test --run` or `pnpm exec vitest --run`. Avoid `pnpm test -- --run`; it can execute as `vitest -- --run`, ignore the run flag, and wait for file changes.

## Claude Code Specific Considerations

- The runner runs `claude -p` in non-interactive mode. Anything that requires
  Claude Code to stop and ask the user (auth, permission, confirmation) will
  hang the batch — keep batches free of those triggers.
- `--dangerously-skip-permissions` is set by default. Audit `hard_rules` and
  `batch_prompt_suffix` for guardrails because Claude Code will not be prompting.
- Tool allowlists/denylists can be appended to `rollout.claude_cmd` (for
  example `--allowed-tools "Read,Edit,Bash"`). The runner passes the template
  through verbatim except for the model injection.
- The repo working directory is the cwd of the subprocess; you do not need
  `--add-dir` for the repo root itself.
