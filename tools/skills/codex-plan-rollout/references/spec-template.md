# <Initiative Name> Specification

## 1. Executive Summary

- Initiative type: `<refactor | migration | convergence | integration | rollout>`
- Primary repo or workspace:
- Primary decision:
- Definition of success in one paragraph:

## 2. Problem And Current State

### 2.1 Problem Statement

Describe the concrete engineering or product problem to solve.

### 2.2 Current State

- Existing code paths or systems:
- Known pain points:
- External systems or dependencies:
- Existing contracts that must be preserved:
- Known organizational or operational constraints:

### 2.3 Baseline Evidence

- Inventory or scan source:
- Current metrics:
- Known hot spots:
- Assumptions that still need validation:

## 3. Goals, Non-Goals, And Success Metrics

### 3.1 Goals

- Must-have outcome:
- Must-have outcome:
- Measurable expectation:

### 3.2 Non-Goals

- Explicitly out of scope:
- Deferred follow-up work:

### 3.3 Success Metrics

- Baseline metric:
- Target metric:
- Verification source:

## 4. Principles And Target State

### 4.1 Design Principles

- Principle:
- Principle:
- Rule that should win when tradeoffs appear:

### 4.2 Target State

- Target architecture or operating model:
- What will be unified, removed, or standardized:
- Compatibility or migration stance:

## 5. Workflows Or Capability Slices

### 5.1 <Workflow Or Capability Name>

- Trigger:
- Happy path:
- Important edge cases:
- Acceptance notes:

### 5.2 <Workflow Or Capability Name>

- Trigger:
- Happy path:
- Important edge cases:
- Acceptance notes:

## 6. Functional Requirements

### 6.1 Required Capability: <Name>

- Description:
- Inputs:
- Outputs:
- Business rules:
- Error handling:

### 6.2 Required Capability: <Name>

- Description:
- Inputs:
- Outputs:
- Business rules:
- Error handling:

## 7. Delivery Strategy

### 7.1 Proposed Phase Shape

- Phase 0 or baseline work:
- Foundation work:
- Pilot or first vertical slice:
- Bulk rollout or migration slices:
- Final verification or handoff:

### 7.2 Rollout And Rollback Notes

- How rollout will be staged:
- What must happen before irreversible changes:
- Rollback or containment strategy:

## 8. Technical Boundaries

### 8.1 Likely Repo Areas Touched

- `path/to/module`
- `path/to/module`

### 8.2 Interfaces, Data, Or Contracts

- API contracts:
- Schemas or storage:
- External services:
- Generated artifacts:

### 8.3 Runtime And Environment Assumptions

- Required runtimes:
- Required env vars:
- Local development assumptions:
- Deployment constraints:

## 9. External Dependencies And Coordination

- Sibling repos or read-only references:
- Manual cloud or platform actions:
- Teams or owners to coordinate with:
- Delivery sequencing outside this repo:

## 10. Hard Rules

- Do not edit out-of-scope systems.
- Prefer focused diffs over broad refactors.
- Do not change public contracts unless this spec says so.
- Add project-specific rules here.

## 11. Verification And Evidence

### 11.1 Global Verification Commands

- `<typecheck command>`
- `<lint command>`
- `<test command>`
- `<metrics or inventory command>`

### 11.2 Evidence To Capture

- Verification document or report:
- Logs, screenshots, or dashboards:
- External checks or follow-up steps that stay outside the automated runner:

### 11.3 Batch-Level Verification Guidance

- Prefer commands that can pass inside a single batch.
- Keep commands non-interactive where possible.
- Prefer verification that exercises the exact touched surface.
- For external checkpoints, define the evidence that proves completion and keep them outside the generated runner.

## 12. Risks, Assumptions, And Open Questions

- Dependency or migration risks:
- Key implementation assumption:
- Key implementation assumption:
- Open question needing an explicit decision:

## 13. Definition Of Done

- All required capabilities are implemented.
- Verification commands pass.
- Relevant contracts or generated artifacts are updated.
- Baseline and target metrics are documented where relevant.
- External follow-up work is explicitly listed outside the automated rollout runner.
- Remaining follow-up work is explicitly documented.
