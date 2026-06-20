# Orchestration Patterns

Use this reference when the initiative is more like a delivery program than a single implementation ticket.

## What Complex Plans Usually Need

- current-state assessment or inventory
- target state or target architecture
- goals, non-goals, and explicit design principles
- phase gates instead of a flat task list
- baseline metrics and post-change success metrics
- risk, rollback, compatibility, or cutover notes
- external dependencies such as gateways, cloud resources, vendor systems, or sibling repos
- a clear split between automated code work and external or operational work

## Spec To Plan Handoff

- write or update `spec.md` before drafting `plan.md`
- derive phase goals from spec goals, target state, and rollout strategy
- derive batch boundaries from independently verifiable slices in the spec
- derive acceptance and verification from the spec's definition of done
- update the spec first when planning reveals a changed goal, non-goal, risk, or external dependency

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
- avoid pretending Codex can finish the real-world action if it cannot

## Metrics And Evidence

For large initiatives, include both:

- baseline metrics: current counts, scan outputs, inventory snapshots
- end-state evidence: commands, dashboards, screenshots, or validation docs proving the rollout worked

## Codex Specific Considerations

- The runner runs `codex exec` in non-interactive mode. Anything that requires
  Codex to stop and ask the user for approval or input will block the batch;
  keep batches free of those triggers.
- `--dangerously-bypass-approvals-and-sandbox` is set by default. Audit
  `hard_rules` and `batch_prompt_suffix` for guardrails because Codex will not
  be prompting.
- Custom command flags can be appended to `rollout.codex_cmd`. The runner
  passes the template through verbatim except for model injection and ensuring
  the stdin marker (`-`) is present.
- The default command sets `--cd {repo}`. If you override `rollout.codex_cmd`,
  keep the command rooted in the rollout repo and preserve stdin piping.
