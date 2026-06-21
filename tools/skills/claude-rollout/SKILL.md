---
name: claude-rollout
description: >-
  Create spec-first rollout packages for large engineering initiatives, where
  the generated runner drives Claude Code through the terminal: a
  human-readable spec, a phase-and-batch execution plan, and an optional
  generated `rollout.py` runner. Use when Claude Code needs to orchestrate
  refactors, migrations, convergence work, or cross-system delivery with hard
  rules, verification commands, external dependencies, operational constraints,
  and resumable batches.
---

# Claude Rollout

Use this skill when the work looks like a real program of delivery rather than
a single coding task:

- multi-phase refactors
- architecture convergence
- platform or infrastructure migrations
- cross-repo integrations
- rollouts that mix code changes, docs, verification, and external coordination

This skill can produce two package shapes:

1. Planning package: upstream spec + derived plan only.
2. Execution package: upstream spec + derived plan + generated `rollout.py`.

Choose the planning package when the work is still exploratory, heavily
approval-driven, or not yet safe for unattended execution. Choose the execution
package only when the rollout batches are in-repo, non-interactive, fully
automatable, and verifiable.

## Output Directory

Each rollout owns its own subdirectory under the project root's
`.agents/rollouts/`, named after the rollout's slug (`<runner-name>`,
identical to `rollout.name` in the plan YAML). Use a slug-friendly name
(lowercase, hyphenated, no spaces) so it can be used directly as a directory
name. Create the rollout directory before writing artifacts. All generated
artifacts MUST live under that tree:

- Spec: `.agents/rollouts/<runner-name>/spec.md`
- Plan: `.agents/rollouts/<runner-name>/plan.md`
- Runner: `.agents/rollouts/<runner-name>/rollout.py`

The runner's runtime workdir (state, prompts, logs) defaults to
`.agents/rollouts/<runner-name>/logs` so every generated file for one rollout
stays under one tree. Override only via `rollout.workdir` in the plan YAML,
and keep overrides inside `.agents/rollouts/<runner-name>/`.

## Workflow

1. Read these references before drafting:
   - [references/spec-template.md](references/spec-template.md)
   - [references/plan-template.md](references/plan-template.md)
   - [references/orchestration-patterns.md](references/orchestration-patterns.md)

2. Draft a project-specific spec at `.agents/rollouts/<runner-name>/spec.md`.
   Capture current state, target state, goals, non-goals, principles, technical
   boundaries, rollout and rollback notes, external dependencies, verification
   strategy, risks, and definition of done.
   Keep the spec human-readable. It is the upstream decision source for the
   plan.

3. Draft a project-specific implementation plan at `.agents/rollouts/<runner-name>/plan.md`.
   Keep prose concise but useful for humans.
   Keep the YAML block between `<!-- rollout-plan:start -->` and
   `<!-- rollout-plan:end -->` complete and valid because
   [scripts/generate_rollout.py](scripts/generate_rollout.py) parses that
   block.
   Derive the phase goals, batch boundaries, risks, acceptance criteria, and
   verification strategy from the spec. Set `rollout.spec_path` to the spec
   file; the generator verifies that it exists and injects it into every batch
   prompt's sources of truth.
   Model each phase as a milestone and each batch as the smallest end-to-end
   unit one Claude Code invocation should finish safely.
   Use prose for rationale, architecture, and coordination notes. Use YAML for
   anything the runner must obey.

4. Decide whether a runner should be generated.
   Generate a runner only when the plan is stable enough and every runnable
   batch is local, deterministic, non-interactive, and fully automatable.
   If the initiative is hybrid, keep manual or out-of-repo steps in prose or in
   an explicit ops checklist. Do not encode them as runner batches.

5. Generate the runner when appropriate. By default it writes
   `rollout.py` next to the plan, so it lands at
   `.agents/rollouts/<runner-name>/rollout.py`. Pass `--output` only to
   override, and keep any override inside `.agents/rollouts/<runner-name>/`:

   ```bash
   python3 scripts/generate_rollout.py --plan .agents/rollouts/<runner-name>/plan.md
   ```

6. Review the generated runner before execution.
   Confirm `repo_root`, `spec_path`, `workdir`, `sources_of_truth`, planning
   notes, hard rules, and batch verification commands.
   Prefer short, idempotent verify commands.
   Use batch-local verification whenever possible.
   Confirm that no batch depends on hidden manual work and that the full
   execution path is automatable end to end.

7. Execute or resume the rollout:

   ```bash
   python3 .agents/rollouts/<runner-name>/rollout.py --list
   python3 .agents/rollouts/<runner-name>/rollout.py
   python3 .agents/rollouts/<runner-name>/rollout.py --from-phase 02-contract
   python3 .agents/rollouts/<runner-name>/rollout.py --from-batch 02-02-handlers
   python3 .agents/rollouts/<runner-name>/rollout.py --only-batch 03-01-tests
   python3 .agents/rollouts/<runner-name>/rollout.py --dry-run
   ```

## Planning Rules

- Keep phases coarse and batches fine. A phase is a milestone. A batch is one
  safe work packet.
- Give every phase and batch a stable numeric prefix such as `01-foundation` or
  `02-03-api-client`.
- Start with current-state evidence. For plan-heavy work, the spec should say
  what exists today, what is broken, and how progress will be measured.
- Treat the spec as upstream of the plan. Do not invent plan phases or batches
  that contradict the spec; update the spec first when the intended direction
  changes.
- Separate three kinds of work clearly:
  - in-repo changes Claude Code can perform
  - read-only references in sibling repos or external systems
  - manual or external actions that require people, cloud consoles, or
    approvals, which stay outside the generated runner
- Put the most important constraints in `hard_rules`. They are injected into
  every batch prompt.
- Put batch-specific constraints, deliverables, and acceptance inside the
  batch.
- Prefer an explicit baseline or foundation phase before bulk migration phases.
- Every phase should state entry criteria, exit criteria, and top risks when
  the initiative is large enough to need them.
- Use `batch.depends_on` when simple phase ordering is not enough.
- Use `batch.kind` to signal the work shape such as `analysis`, `code`,
  `docs`, or `verification`.
- Keep `execution` set to `claude`; the generated runner is fully automatic and
  does not support manual pause points.
- Keep verify commands non-interactive and deterministic.
- Keep external checkpoints evidence-oriented in prose or checklist form so
  humans can execute them outside the runner.
- Make later batches depend on earlier work by ordering them in the same
  phase. Use explicit dependencies only when needed.
- Do not ask the runner to infer the plan from prose. The YAML block is
  authoritative.
- Keep `rollout.spec_path` pointed at the upstream spec. The generator rejects a
  plan when that file does not exist.

## No Human Intervention In Batches

The runner is fully automated end to end. A batch must always decide and
execute; it must never pause for a human, intentionally exit non-zero to
surface a diff, delegate decisions back to the operator via prompts, or write
sentinel files that require human action. If a batch's verification can only be
satisfied by human judgment, the batch belongs outside the runner; keep it in
prose or an ops checklist, not in the plan YAML.

When drafting a batch, rewrite these anti-patterns before generating a runner:

- A `verify_commands` entry whose failure message tells the user to review,
  decide, approve, or intervene. Verify commands check whether the automated
  action succeeded; they are not a place to request escalation.
- A `goal` or `prompt_context` instruction of the form "if X is ambiguous,
  stop and write a decision file for the human". Replace it with a default
  action the batch will take in the ambiguous case.
- Batches that hinge on out-of-band approval, external review, or cloud console
  clicks. Those steps live in prose or in checklists, never as a batch with
  `execution: claude`.
- Conditional acceptance criteria that fail verification so the human can
  intervene. Acceptance and verification only describe what success looks like
  for the automated path.

When a batch must make a genuine runtime decision, give Claude Code the
authority and the default in the prompt context. For example, if two files
would collide, tell it to delete one when they are functionally identical or
rename to a documented basename and record the choice in evidence that
downstream batches can read.

## When To Split

Split a new batch when any of these are true:

- The work touches more than one subsystem and can be verified separately.
- The prompt would require multiple distinct acceptance checkpoints.
- A failed verification should not force rerunning unrelated work.
- You would want an isolated commit boundary.

## When Not To Generate A Runner

Skip runner generation for now when any of these are true:

- Most work is still discovery, decision-making, or stakeholder alignment.
- Success depends on many manual cloud or vendor actions.
- Verification is mostly human judgment and not yet scriptable.
- The plan shape is expected to change daily while the design is still moving.

## References

- [references/spec-template.md](references/spec-template.md): project spec
  scaffold.
- [references/plan-template.md](references/plan-template.md): batch-oriented
  execution plan scaffold and YAML schema.
- [references/orchestration-patterns.md](references/orchestration-patterns.md):
  patterns for large refactors, migrations, convergence work, and execution
  plans that separate automated repo work from external ops.
- [scripts/generate_rollout.py](scripts/generate_rollout.py): reads the plan
  YAML, verifies the upstream spec path, and writes a standalone runner.

## Runner Behavior

The generated `rollout.py`:

- persists progress in a state file under the configured workdir
- writes batch prompts and logs under the configured workdir
- injects `rollout.spec_path` into every batch prompt's sources of truth
- calls `claude -p ...` for each batch, piping the rendered prompt to Claude
  Code on stdin
- verifies each batch with shell commands
- feeds failed Claude Code or verification output back into a retry prompt
- runs only fully automated batches and rejects manual pause points at
  generation time
- resumes from unfinished work
- supports phase and batch selection flags

## Claude Code CLI Notes

- Default command template:
  `claude -p --dangerously-skip-permissions --output-format text`, run with
  the repo root as the working directory. Override per project via
  `rollout.claude_cmd` in the plan YAML or `--claude-cmd` at runtime; use
  `{repo}` for the absolute repo root.
- Set a model with `rollout.model` or `--model`; the runner appends
  `--model <id>` automatically when set.
- `--dangerously-skip-permissions` is required for unattended execution because
  Claude Code otherwise prompts on tool use. Only run the generated
  `rollout.py` in repositories where that is acceptable.
- The working tree must be clean unless `allow_dirty` (plan) or
  `--allow-dirty` (CLI) is set.
- A failed Claude Code invocation or failed verification is fed back as a retry
  prompt up to `rollout.max_fix_attempts` (default `1`) or
  `--max-fix-attempts N` times. Set it to `0` to fail fast on the first error.

## Concurrent Rollout Notes

Each `rollout.py` is single-threaded: one batch invokes one Claude Code process
and waits for it to exit. The runner does not coordinate with other rollouts
running on the same machine. If multiple rollouts share the same account,
rate-limit budget, or local CLI state, run them serially or point
`rollout.claude_cmd` at a wrapper that queues invocations.

Any wrapper must forward stdin and preserve the child exit code so prompt
piping and retry logic keep working. Use one lock name or lock file shared by
every rollout on the machine. Serialization trades wall-clock throughput for
predictability; if reliable parallelism is required, run separate rollouts
under isolated accounts and CLI state.
