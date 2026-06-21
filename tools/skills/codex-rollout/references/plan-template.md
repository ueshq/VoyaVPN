# <Initiative Name> Implementation Plan

This document has two layers:

- prose for humans
- a machine-readable YAML block used by `scripts/generate_rollout.py`

If prose and YAML disagree, the YAML block wins.

## Planning Heuristics

- One phase = one milestone.
- One batch = one Codex invocation worth of end-to-end work.
- Prefer 2-6 batches per phase.
- Prefer local verification commands for runnable batches.
- Order batches exactly as they should run.
- Use prose for rationale and coordination.
- Use YAML for anything the runner must obey.
- Model manual or out-of-repo checkpoints explicitly in prose or an external checklist instead of hiding them in runner batches.

<!-- rollout-plan:start -->

```yaml
rollout:
  name: "<initiative-name>"
  repo_root: "/absolute/path/to/repo"
  spec_path: ".agents/rollouts/<initiative-name>/spec.md"
  # workdir defaults to .agents/rollouts/{rollout.name}/logs; uncomment to override.
  # workdir: ".agents/rollouts/<initiative-name>/logs"
  codex_cmd: null
  model: null
  max_fix_attempts: 1
  allow_dirty: false
  commit_per_batch: true
  sources_of_truth:
    - "docs/architecture.md"
  planning_notes:
    - "Mention plan-wide context such as baseline metrics, migration shape, or scope boundaries."
  success_metrics:
    - "Document one measurable outcome the rollout should improve or hold constant."
  global_context:
    - "Summarize project-wide context that every batch should see."
    - "Mention any important repo conventions."
  hard_rules:
    - "Do not edit external systems or sibling repositories."
    - "Keep diffs focused on the current batch."
    - "Run the listed verification commands before reporting success."
  batch_prompt_suffix:
    - "Finish only this batch and anything strictly required to make it pass."
phases:
  - id: "01-foundation"
    title: "Foundation"
    goal: "Create the baseline needed for later work."
    depends_on: []
    summary: "Shared context for every batch in this phase."
    entry_criteria:
      - "Baseline context is clear enough to start this phase."
    exit_criteria:
      - "Shared scaffolding or rules are in place for later phases."
    risks:
      - "Call out the main risk of this phase."
    batches:
      - id: "01-01-scaffold"
        title: "Scaffold"
        kind: "code"
        execution: "codex"
        goal: "Create the initial structure and make the baseline checks pass."
        depends_on: []
        deliverables:
          - "List the concrete files or outcomes expected from this batch."
        acceptance:
          - "List human-readable acceptance criteria for this batch."
        evidence_to_capture:
          - "Describe logs, docs, or metrics that prove the batch is complete."
        verify_commands:
          - "pnpm typecheck"
        files_to_touch:
          - "apps/example/**"
        prompt_context:
          - "Add batch-specific notes, tradeoffs, or constraints."
      - id: "01-02-follow-up"
        title: "Follow-up"
        kind: "docs"
        execution: "codex"
        goal: "Capture the next phase gate in-repo and verify that the baseline still holds."
        depends_on:
          - "01-01-scaffold"
        deliverables:
          - "List the concrete files or outcomes expected from this batch."
        acceptance:
          - "List human-readable acceptance criteria for this batch."
        evidence_to_capture:
          - "Describe the evidence needed before marking this batch complete."
        verify_commands:
          - "pnpm lint"
        files_to_touch:
          - "docs/runbook.md"
        prompt_context:
          - "Explain why this batch exists and what later work depends on it."
  - id: "02-product-slice"
    title: "Product Slice"
    goal: "Deliver the first user-visible slice."
    depends_on:
      - "01-foundation"
    summary: "Shared context for every batch in this phase."
    entry_criteria:
      - "The foundation phase has completed and the key decision points are stable."
    exit_criteria:
      - "The first slice is implemented and verified."
    risks:
      - "Note the most likely integration or migration risk."
    batches:
      - id: "02-01-feature"
        title: "Feature"
        kind: "code"
        execution: "codex"
        goal: "Implement the first user-visible slice."
        depends_on: []
        deliverables:
          - "List the concrete files or outcomes expected from this batch."
        acceptance:
          - "List human-readable acceptance criteria for this batch."
        evidence_to_capture:
          - "Describe logs, docs, or metrics that prove the batch is complete."
        verify_commands:
          - "pnpm test"
        files_to_touch:
          - "apps/web/**"
        prompt_context:
          - "Add batch-specific notes, tradeoffs, or constraints."
```

<!-- rollout-plan:end -->

## Field Notes

- `rollout.repo_root`: Prefer an absolute path because the generated `rollout.py` is standalone.
- `rollout.spec_path`: Required upstream spec for this plan. Relative paths are resolved from `repo_root`; the generator verifies that the file exists and injects it into every batch prompt's sources of truth.
- `rollout.workdir`: Relative paths are resolved from `repo_root`. Defaults to `.agents/rollouts/{rollout.name}/logs` so runtime state lives alongside that rollout's spec, plan, and runner.
- `rollout.codex_cmd`: Optional override for the Codex CLI command template. Use `{repo}` as a placeholder. Defaults to `codex exec --dangerously-bypass-approvals-and-sandbox --cd {repo} -`.
- `rollout.model`: Optional Codex model id. The runner inserts `--model <id>` before the stdin marker (`-`) automatically when set.
- `rollout.max_fix_attempts`: Auto-fix budget for each batch. When Codex exits non-zero or a verification command fails, the runner feeds the error output back into a follow-up Codex invocation up to this many times before declaring the batch `failed`. Defaults to `1`. Set to `0` to fail fast on the first error.
- `rollout.commit_per_batch`: Commits after each successful batch. Defaults to `true`; set to `false` to keep all changes uncommitted.
- `rollout.sources_of_truth`: Additional references injected into every batch prompt after `rollout.spec_path`.
- `rollout.planning_notes`: Shared planning context such as baseline metrics, migration shape, or cross-team constraints.
- `rollout.success_metrics`: Reminders about what the initiative must measurably improve or preserve.
- `rollout.global_context`: Shared context injected into every batch prompt.
- `rollout.hard_rules`: Non-negotiable rules injected into every batch prompt.
- `rollout.batch_prompt_suffix`: Common closing reminders appended to every batch prompt.
- `phase.summary`: Optional context shared by all batches in that phase.
- `phase.entry_criteria` / `phase.exit_criteria`: Use these for complex migrations or convergence work where gating matters.
- `phase.risks`: Short list of the main things that could invalidate the plan.
- `batch.kind`: Freeform label such as `analysis`, `code`, `docs`, or `verification`.
- `batch.execution`: Keep this as `codex`. The generated runner is fully automatic and does not support manual pause points.
- `batch.depends_on`: Optional extra dependencies on specific batch ids. Batch order is still authoritative.
- `batch.evidence_to_capture`: Evidence that should exist when the batch is complete.
- `batch.files_to_touch`: Optional hint list. It does not restrict the runner.
- `batch.prompt_context`: Optional batch-only notes.

## Authoring Checklist

- Use stable, numeric IDs such as `01-foundation` and `01-02-api`.
- Draft the spec first, then derive phase goals, risks, acceptance, and verification from that spec.
- Make batch goals action-oriented and end-to-end.
- Keep verification commands small, deterministic, and non-interactive for runnable batches.
- Do not let one batch depend on work from a later batch.
- Add an explicit baseline phase when the initiative depends on inventories or current-state scans.
- Separate repo work from external or manual work.
- Keep non-automated work in prose or an external checklist, not in the generated runner's YAML batches.
- If a batch would be painful to rerun after a failed verify step, split it.
- Confirm the repo is safe to run with `--dangerously-bypass-approvals-and-sandbox`. The runner sets that flag by default so Codex can complete each batch without prompting.

## Phase Notes

### 01-foundation

- Why this phase exists:
- Likely files:
- Risks:

### 02-product-slice

- Why this phase exists:
- Likely files:
- Risks:
