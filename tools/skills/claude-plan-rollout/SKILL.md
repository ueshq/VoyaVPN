---
name: claude-plan-rollout
description: >-
  Create spec-first rollout packages for large engineering initiatives, where
  the runner drives Claude Code (`claude -p`) through the terminal: a
  human-readable spec, a phase-and-batch execution plan, and an optional
  generated `rollout.py` runner. Use when Claude Code needs to orchestrate
  refactors, migrations, convergence work, or cross-system delivery with hard
  rules, verification commands, external dependencies, operational constraints,
  and resumable batches.
---

# Claude Plan Rollout

This skill is the Claude Code variant of the spec-plan-rollout skill. The
planning surface (spec + plan) is identical, but the generated runner shells
out to the Claude Code CLI (`claude -p`) instead of `codex exec`.

Use this skill when the work looks like a real program of delivery rather than
a single coding task:

- multi-phase refactors
- architecture convergence
- platform or infrastructure migrations
- cross-repo integrations
- rollouts that mix code changes, docs, verification, and external coordination

This skill can produce two package shapes:

1. Planning package: spec + plan only.
2. Execution package: spec + plan + generated `rollout.py` driven by Claude Code.

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
   Keep the spec human-readable. It is the planning source, not the runtime
   source.

3. Draft a project-specific implementation plan at `.agents/rollouts/<runner-name>/plan.md`.
   Keep prose concise but useful for humans.
   Keep the YAML block between `<!-- rollout-plan:start -->` and
   `<!-- rollout-plan:end -->` complete and valid because
   [scripts/generate_rollout.py](scripts/generate_rollout.py) parses that
   block.
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
   Confirm `repo_root`, `workdir`, `sources_of_truth`, planning notes, hard
   rules, and batch verification commands.
   Prefer short, idempotent verify commands.
   Use batch-local verification whenever possible.
   Run any ambiguous package-manager test command once outside the runner and
   confirm that it exits by itself.
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
- Be precise with package-manager argument forwarding. For Vitest under pnpm,
  use `pnpm test --run` or `pnpm exec vitest --run`; do not use
  `pnpm test -- --run`, which can become `vitest -- --run` and leave Vitest
  waiting for file changes.
- Keep external checkpoints evidence-oriented in prose or checklist form so
  humans can execute them outside the runner.
- Make later batches depend on earlier work by ordering them in the same
  phase. Use explicit dependencies only when needed.
- Do not ask the runner to infer the plan from prose. The YAML block is
  authoritative.

## No Human Intervention In Batches

The runner is fully automated end-to-end. A batch must always decide and
execute; it must never pause for a human, never intentionally exit non-zero to
"surface a diff", and never delegate decisions back to the operator via prompts
or sentinel files. If a batch's verification can only be satisfied by human
judgment, the batch belongs outside the runner — keep it in prose or an ops
checklist, not in the plan YAML.

When drafting a batch, watch for these anti-patterns and rewrite them before
generating the runner:

- A `verify_commands` entry whose failure message tells the user to "review",
  "decide", "approve", or "intervene" (e.g.
  `echo "files differ; human review required"; exit 1`). Verify commands check
  whether the batch's automated action succeeded — they are not a place to
  request escalation.
- A `goal` or `prompt_context` instruction of the form "if X is ambiguous,
  stop and write a decision file for the human". Replace it with a default
  action the batch will take in the ambiguous case (e.g. rename with a
  documented basename) so the rollout can proceed unattended.
- Batches that hinge on out-of-band approval, external review, or cloud
  console clicks. Those steps live in prose or in `references/`-style
  checklists, never as a batch with `execution: claude`.
- Conditional acceptance criteria like "if decision is X, FAIL verification
  so the human can intervene". Acceptance and verification only describe what
  success looks like for the automated path the batch chose.

When a batch must make a genuine runtime decision (e.g. two files would
collide), give Claude Code the authority and the default in the prompt
context — for example, "if functionally identical, delete one; otherwise
rename to `<documented-basename>` and record the choice in
`evidence/<decision>.txt` so downstream batches can read it". Downstream
batches that depend on the decision should read that evidence file rather
than re-deciding.

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
  and writes a standalone runner that drives Claude Code.

## Runner Behavior

The generated `rollout.py`:

- persists progress in a state file under the configured workdir
- writes batch prompts and logs under the configured workdir
- calls `claude -p --dangerously-skip-permissions` for each batch, piping the
  rendered prompt to Claude Code on stdin
- verifies each batch with shell commands
- feeds failed verification output back into a retry prompt
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
- Set a model with `rollout.model` (e.g. `claude-opus-4-7`) or `--model`; the
  runner appends `--model <id>` automatically.
- `--dangerously-skip-permissions` is required for unattended execution
  because Claude Code otherwise prompts on every tool use. Only run the
  generated `rollout.py` in repositories where that is acceptable.
- The working tree must be clean unless `allow_dirty` (plan) or
  `--allow-dirty` (CLI) is set.
- A failed Claude Code invocation or failed verification is fed back as a
  retry prompt up to `rollout.max_fix_attempts` (default `1`) or
  `--max-fix-attempts N` times; set it to `0` to fail fast on the first
  error.

## Serializing Concurrent Rollouts

Each `rollout.py` is single-threaded — one batch invokes one `claude.exe` at a
time and waits for it to exit. But the runner does **not** coordinate with
other rollouts running on the same machine. If you launch several rollouts in
parallel (or accidentally double-launch the same one), every batch boundary
spawns yet another `claude.exe`. The usual symptom is that the first few
batches succeed and then a later batch silently hangs: `claude.exe` is alive
with near-zero CPU, the batch log has only the header the runner wrote, and
the 30-second heartbeats are the only sign the runner itself is fine. Root
cause is contention on the Claude account (rate / TPM limits, 5-hour token
budget) or on local `~/.claude/` session state — `claude.exe` waits without
emitting output, so the runner can't tell it apart from a normal slow batch.

When multiple rollouts must run on the same machine, serialize the
`claude.exe` invocations with a wrapper and point `rollout.claude_cmd` at it.
A global named mutex (Windows) or `flock` (POSIX) is enough; the wrapper must
forward stdin and the exit code unchanged so the runner's prompt piping and
retry logic keep working.

Windows (`tools/scripts/claude-serial.ps1`):

```powershell
$mutex = New-Object System.Threading.Mutex($false, 'Global\ClaudeRolloutMutex')
try {
  $null = $mutex.WaitOne()
  & 'C:\Users\<you>\.local\bin\claude.EXE' @args
  exit $LASTEXITCODE
} finally {
  try { $mutex.ReleaseMutex() } catch {}
  $mutex.Dispose()
}
```

Then in the plan YAML:

```yaml
rollout:
  claude_cmd: >-
    pwsh -NoProfile -ExecutionPolicy Bypass -File
    {repo}/tools/scripts/claude-serial.ps1 -p
    --dangerously-skip-permissions --output-format text
```

POSIX (`tools/scripts/claude-serial.sh`):

```bash
#!/usr/bin/env bash
exec flock /tmp/claude-rollout.lock claude "$@"
```

```yaml
rollout:
  claude_cmd: "{repo}/tools/scripts/claude-serial.sh -p --dangerously-skip-permissions --output-format text"
```

Notes when adopting this:

- The lock is process-wide, not user-wide. Pick a name (`Global\…` /
  lock-file path) that every rollout on the box shares so they actually
  queue against each other.
- The wrapper must `exec`/forward stdin — the runner pipes the prompt file
  into the child's stdin. PowerShell's `&` and bash's `exec` both inherit
  stdin correctly; do not interpose `cmd /c` or `Start-Process`.
- Heartbeats become more useful with serialization: a stuck rollout now
  reflects a genuine `claude.exe` problem rather than contention, so a
  silent batch past the first 30s heartbeat is a real signal to
  investigate.
- Serializing trades wall-clock throughput for reliability. If you need
  parallelism, run each rollout under a separate Claude account and skip
  the wrapper.
