# Parallel-First Agent Execution Policy Example

This file is an extracted example from `~/.codex/AGENTS.md`. It documents the user-scope instruction policy that makes Codex plan with subagents more proactively on complex work.

The policy intentionally overrides Codex's default conservative stance against automatic agent spawning. It makes subagent usage more aggressive for complex, multi-axis work while keeping edits coordinated through explicit ownership boundaries.

## High-Priority Parallelism Rule

For non-trivial tasks, first decide whether the work has independent axes. If yes, use parallel read-only subagents before editing.

Prefer:

- parallel exploration, review, docs checking, test discovery, and validation;
- one final implementation lane;
- parallel implementation only when edit boundaries are clearly disjoint.

Do not serialize independent investigation.
Do not parallelize tightly coupled edits.
Do not let multiple agents edit the same files.
Spawn the smallest useful number of subagents: usually 2-3 for complex tasks, 4-6 for broad audits/reviews, 0-1 for local tasks.

If independent investigation axes exist and no subagents are spawned, explicitly justify why sequential execution is better.

## Purpose

Use a parallel-first workflow for complex software engineering tasks.

The goal is not to parallelize everything. The goal is to let Codex deliberately decide when independent investigation, review, testing, or implementation lanes can run concurrently, while keeping shared-file edits and dependency-heavy work coordinated.

Default assumption:

- simple, tightly coupled, or single-file tasks should stay local and sequential;
- complex, multi-axis, multi-module, review-heavy, test-heavy, or research-heavy tasks should be evaluated for parallel subagents;
- parallel exploration is preferred over parallel editing;
- only one agent should usually apply the final patch unless edit boundaries are clearly disjoint.

## Core Rule

Before doing substantial work, classify the task by dependency shape.

Use parallel subagents when:

- subtasks are independent or mostly independent;
- different agents can inspect different modules, layers, files, test suites, logs, or documentation without blocking each other;
- the task benefits from independent evidence before implementation;
- the task involves PR review, bug triage, regression analysis, large-codebase exploration, migration planning, test discovery, API/docs verification, or multi-module auditing.

Do not use parallel subagents when:

- the task is small and local;
- the next step depends on immediate local context;
- the work is mostly a single sequential reasoning chain;
- multiple agents would edit the same files;
- the acceptance target is unclear;
- the cost of coordination exceeds the benefit of parallelism.

## Decision Procedure

For every non-trivial task, do the following internally before acting:

1. Identify the task intent.
2. Identify constraints, likely touchpoints, and unknowns.
3. Define acceptance criteria:
   - What must be true at the end?
   - Which command, test, artifact, or manual check proves success?
4. Classify the dependency structure:
   - independent axes -> parallel lanes;
   - shared-file or prerequisite-heavy work -> staged/sequential execution;
   - unclear dependency graph -> do a short focused exploration first.
5. Decide whether to spawn subagents.
6. If spawning subagents, choose the smallest useful number of lanes.
7. Wait for the relevant subagent results before making final implementation decisions.
8. Synthesize evidence.
9. Apply the smallest safe change.
10. Validate against the acceptance criteria.

## How Many Subagents to Spawn

Choose the number of subagents based on the number of genuinely independent axes, not based on task size alone.

Default limits:

- 0 subagents: trivial/local task.
- 1 subagent: one bounded evidence lane useful while the main agent works.
- 2-3 subagents: normal complex task with several independent investigation axes.
- 4-6 subagents: broad review, migration, audit, or multi-module investigation.
- More than 6 only when the task is explicitly batch-like and the work items are independent.

Prefer fewer high-quality lanes over many vague lanes.

Spawn subagents by role, not by arbitrary count.

Good lane types:

- code-path explorer
- test-discovery agent
- regression-history agent
- API/docs researcher
- security reviewer
- performance reviewer
- compatibility reviewer
- frontend/backend/module-specific explorer
- implementation worker for a clearly disjoint edit boundary
- validation/test runner

Bad lane types:

- "figure it out"
- "implement whatever is needed"
- "check everything"
- multiple agents editing the same file without coordination

## Editing Policy

Parallel exploration is safe by default.
Parallel editing is not.

Use this hierarchy:

1. Read-only subagents for evidence.
2. Main agent synthesizes.
3. One implementation lane edits.
4. Additional implementation lanes only if file/module ownership is clearly disjoint.
5. Main agent integrates and validates.

Never allow multiple agents to independently rewrite the same files.

If two lanes propose conflicting changes:

- stop parallel implementation;
- compare evidence;
- choose one minimal patch;
- explain the tradeoff if relevant.

## Subagent Prompt Requirements

When spawning a subagent, give it a bounded task.

Every subagent prompt should include:

- role;
- scope;
- files/modules/tests to inspect if known;
- whether it may edit files;
- expected output format;
- confidence/risk requirement;
- stop condition.

Preferred output format:

```text
Summary:
Evidence:
Relevant files/symbols:
Recommendation:
Risk/uncertainty:
Validation suggestion:
```

For read-only lanes, explicitly say:

```text
Do not edit files. Return evidence and recommendations only.
```

For implementation lanes, explicitly say:

```text
Only edit within this boundary: <files/modules>.
Avoid unrelated refactors.
Return changed files, rationale, and validation performed.
```

## Concurrency Heuristics

Use parallelism aggressively for:

- read-only investigation;
- review;
- grep/search across independent concepts;
- docs/API verification;
- test discovery;
- independent test commands;
- multi-module audits.

Use parallelism cautiously for:

- implementation;
- refactors;
- formatting large areas;
- dependency upgrades;
- generated code;
- shared configuration files;
- lockfiles.

Avoid parallelism for:

- single-file fixes;
- tightly coupled algorithm changes;
- tasks requiring a single coherent design decision;
- changes where one result determines the next step.

## Final Response Requirements

When parallel lanes were used, summarize:

- which lanes ran;
- what each found;
- what decision was made from their evidence;
- what changed;
- what validation passed or could not be run.

Keep the final answer concise but evidence-backed.
