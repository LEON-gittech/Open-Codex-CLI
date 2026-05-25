# Open Codex Subagent Spawn Policy Guide

User-scope subagent spawn policy docs live here so they stay out of the upstream `docs/*.md` namespace and are easy to point at from the README's nonblocking background execution section.

The policy is not "spawn only when the user explicitly asked for subagents."
Explicit parallel/subagent requests are strong positive triggers, but normal
spawning is contract-based: independent, consumable, bounded, and worth the
coordination cost.

The same ownership rule applies to background terminals and processes: do not
stop them merely because they look unrelated or consume CPU. Stop only work that
belongs to the active task, blocks required resources, risks corrupting the
active artifact, or was explicitly requested to be stopped.

## What's here

| File | Purpose | Where it normally lives in a user environment |
| --- | --- | --- |
| [`agents-md-snippet.md`](agents-md-snippet.md) | Drop-in snippet for `~/.codex/AGENTS.md` that wires up the fork's contract-first subagent spawn gates, stable profile set, per-query reasoning markers, and `/btw` / `/side` etiquette. | `~/.codex/AGENTS.md` (user-scope, lives in your home directory once installed) |
| [`parallel-first-agent-execution.md`](parallel-first-agent-execution.md) | The full contract-first subagent execution policy with its rationale, decision procedure, lane counts, edit ownership rules, and final-response checklist. Used as the canonical reference behind the snippet above. | This repo only — extracted from a real `~/.codex/AGENTS.md` for community visibility. |

## Why split this from `docs/`

The root `docs/` directory tracks upstream `openai/codex` documentation (install, auth, sandbox, etc.) and is kept close to upstream during merges. Open Codex user policy guides go in `docs/open-codex/` so:

- they survive `git merge rust-vX.Y.Z` without touching upstream files,
- the README has a single dedicated entry point to point users at, and
- users can clone or vendor `docs/open-codex/` alone if they only need the subagent spawn policy.

## Related code-side material

The fork's runtime behavior for subagents (spawn-tool description, gate text, status-line plumbing) lives in:

- `codex-rs/core/src/tools/handlers/multi_agents_spec.rs` — `SPAWN_AGENT_CONTRACT_GUIDANCE_V2` ships the four-gate (Independent / Consumer decision / Bounded / Worth it) text directly into the model's tool description.
- `codex-rs/app-server-protocol/src/protocol/v2/item.rs` — `CollabAgentState` carries the per-subagent receipt fields (`ownership`, `output_contract`, `spawn_reason`, `phase`, `lane`) surfaced into the TUI background-task panel.
- `codex-rs/tui/src/chatwidget/background_agents.rs` — TUI sync that wires the protocol fields into the down panel.

The docs in this directory are the *operator-facing* policy that should accompany those runtime guarantees.
