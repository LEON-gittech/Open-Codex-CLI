# `~/.codex/AGENTS.md` — Drop-in snippet

This is the user-scope instruction layer the fork expects to see at runtime.
Paste the block below into your `~/.codex/AGENTS.md` (create the file if it
does not exist) so the model starts the session with the contract-first
subagent policy already loaded.

The same content is referenced and unpacked at length in
[`parallel-first-agent-execution.md`](parallel-first-agent-execution.md) —
keep that doc handy when you want to understand *why* a given rule fires.

---

## Snippet

```markdown
# Codex user-scope agent instructions

## Contract-first subagent delegation

For non-trivial work, before editing or spawning anything, decide whether
subagents would produce independent results the main lane will actually
consume. Do not spawn simply because the task is large.

Spawn only when every gate passes:

- **Independent** — the work can proceed without sharing mutable state with
  your current main-lane task.
- **Consumer decision** — name the decision, test, edit, or release gate
  that will consume the result before spawning.
- **Bounded** — the subagent has a clear stop condition and compact output
  contract.
- **Worth it** — the expected evidence is more valuable than running the
  same search/check locally.

Spawn the smallest useful number of lanes:

- 0 for trivial / local tasks.
- 1 bounded evidence lane for one independent axis you can keep working
  next to.
- 2–3 for normal complex tasks with several independent investigation
  axes.
- 4–6 only for broad review / migration / audit work with cleanly disjoint
  lanes.

Prefer read-only investigation lanes for evidence. Keep editing centralised
in one implementation lane unless edit boundaries are clearly disjoint.

After spawning, do not cross the named consumer decision until you have
consumed the result or explicitly cancelled the lane. The main lane stays
responsible for synthesis, verification, and the final user-facing answer.

## Per-query reasoning markers

When the user includes one of `ulw`, `ultra`, or `xhigh` as a standalone
word in their query, treat that turn (and only that turn) as if reasoning
effort were set to `xhigh`. Do not mutate the persistent session default.

## Side conversations

- `/btw <question>` opens an inline lightweight side thread that does NOT
  take over the primary chat — keep its answers compact and return focus
  to the main lane.
- `/side <prompt>` forks a regular side conversation; use it when the user
  asks for a longer-running parallel investigation.
```

---

## How the runtime enforces this

The same four-gate text is also injected directly into the model's
`spawn_agent` tool description by
`codex-rs/core/src/tools/handlers/multi_agents_spec.rs::SPAWN_AGENT_CONTRACT_GUIDANCE_V2`,
so a model can see the contract even if `~/.codex/AGENTS.md` was not
configured. The user-scope snippet above stays useful because it primes the
*main-lane* behavior (when to think about spawning, how to handle `/btw` vs
`/side`, how to react to `xhigh` markers) before any tool call happens.
