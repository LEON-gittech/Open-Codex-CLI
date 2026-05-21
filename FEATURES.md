# Open Codex Feature Inventory

Last updated: 2026-05-21

This file tracks fork-specific features and fixes that should remain visible during README, release-note, and roadmap updates. It is an engineering inventory, not a marketing page. Keep it scoped to behavior implemented in this fork.

## Scope

Open Codex CLI is a native Codex fork. Features listed here should generally require one of these layers:

- Rust runtime changes under `codex-rs`
- launcher or npm distribution changes under `codex-cli`
- app-server, protocol, state, or TUI contract changes
- repository-level instruction or release process changes that affect how this fork is operated

Fast-moving prompt packs, hooks, setup flows, and project policies are better handled by wrapper/workflow projects unless they need native runtime guarantees.

## Implemented Features

### TUI readability and turn correctness

| Feature | User-facing behavior | Main entry points | Notes |
| --- | --- | --- | --- |
| User-query highlighting | User-authored queries are visually easier to scan in composer and history. | Chat composer, `UserHistoryCell` | Shared styling keeps light/dark terminal behavior consistent. |
| Zellij pane scroll compatibility | Running Open Codex inside Zellij no longer steals pane wheel scroll through xterm alternate-scroll mode. | Alt-screen enter/leave, Ctrl-Z suspend/resume | Disables `CSI ?1007 h/l` only when Zellij environment variables are present, preserving existing terminal behavior elsewhere. |
| Stale turn output guard | Old assistant, plan, reasoning deltas, and stale completions are dropped when they belong to an older turn. | Live TUI notification/render path | Replay remains separate so historical resumed content still renders. |
| Esc steer priority restoration | Pending steers and rejected steers prevent double-Esc rewind from stealing single-Esc steering/interrupt behavior. | App Esc routing, `has_pending_or_queued_input` | Restores the 0.130.x pending-input semantics after the 0.131 merge split input state. |
| Resume latest-response restoration | Resuming or rejoining a running session preserves already-streamed assistant deltas instead of showing a truncated latest response. | App-server thread history builder, active thread snapshot | Final `AgentMessage` replaces the delta-backed item to avoid duplication. Added in `d587d950bc`. |

### Memory UX

| Feature | User-facing behavior | Main entry points | Notes |
| --- | --- | --- | --- |
| Active memory staging | `memory_stage_update` writes an ad-hoc note and immediately stages the same content into the current session overlay. | `memory_stage_update` tool | Durable storage still follows upstream memory consolidation. |
| Explicit memory write feedback | Memory staging returns staged content, reason, overlay revision, and ad-hoc note path. | Tool result | Lets the assistant tell the user exactly what changed. |
| Global memory overlay diagnostics | `/memory-overlay` shows session overlays plus global ad-hoc staged notes. | `/memory-overlay` | Rows show `matched` or `pending` exact-match status against durable files. |
| Durable memory browser | `/memory` shows durable memory grouped by `Summary`, `Overall index`, and `Topics`. | `/memory` | Designed to show what future sessions can actually read. |

### Reasoning and speed controls

| Feature | User-facing behavior | Main entry points | Notes |
| --- | --- | --- | --- |
| `/effort` | Users can change active reasoning effort from the TUI. | `/effort` | Persistent session/default reasoning control. |
| Persistent `Shift+Tab` speed toggle | `Shift+Tab` toggles between `high` + Fast mode and `xhigh` + standard tier for fast-capable models. | Composer key handling, service tier config | Persists both reasoning effort and service tier defaults. Added in `81f61c9aa4`, documented in `0505d0528a`. |
| One-turn high-effort markers | Standalone `ulw`, `ultra`, or `xhigh` submits only that turn with `xhigh` reasoning. | User query parser | Does not mutate persistent session defaults. |
| Per-turn effort status visibility | Status line can show submitted per-turn effort while the foreground turn is pending or running. | `model-with-reasoning` status-line item | Restores to session/default effort after completion, failure, or interrupt. |
| Resume reasoning and fast-mode restoration | Resumed sessions restore their own model, reasoning effort, and fast/standard service tier instead of inheriting another session. | `TurnContext`, state metadata, `thread/resume` | Includes `service_tier` DB migration and resume merge fixes. Added in `d587d950bc`. |

### Session management and rollback

| Feature | User-facing behavior | Main entry points | Notes |
| --- | --- | --- | --- |
| `/export` | Export the current session transcript to a user-chosen `.md` or `.txt` path. | `/export <path>` | Useful for debugging, archival, and sharing. |
| Claude Code-style rewind UX | Double-Esc rewind supports code + conversation restore and a cleaner picker. | Double-Esc backtrack flow | Recent fixes tightened session scoping and ordering. |
| Revoke restore scope fix | Conversation-only restore no longer rolls back files. | Revoke/rewind restore logic | Fixed by `b6f62e4867`. |
| Esc interrupt routing | Single Esc interrupt and double-Esc rewind detection both work without blocking each other. | Composer key handling | Fixed by `0d20c120d8`. |

### Background tasks and subagents

| Feature | User-facing behavior | Main entry points | Notes |
| --- | --- | --- | --- |
| Nonblocking terminal tasks | Long-running terminal commands can continue in the background while chat continues. | `Ctrl+B`, task panel | Completed output remains available in history. |
| Background subagent tracking | Spawned agents appear as background activity with role, status, runtime, progress, and task context. | Task panel, spawn/wait tool handling | Foreground `Working` state is separate from background counts. |
| `/agent` and `/subagents` split | `/agent` lists agent profiles, while `/subagents` opens the subagent thread picker for live, resumable, and reviewable threads. | Slash commands | Closed review rows do not imply spawn quota is still held; quota availability is determined by active/interrupted runtime handles. |
| Subagent completion wakeups | Completed subagent work wakes the parent turn without requiring manual wait/close. | Core completion events | Added/fixed around `8670c2c842`. |
| Subagent quota reclamation | Completed subagents are reclaimed from spawn quota, and spawn performs opportunistic cleanup when quota is exhausted. | Subagent runtime, spawn path | Interrupted subagents remain active/resumable quota holders. Fixed by `5348fb6fcd`. |
| Parallel-first subagent policy | Complex tasks are encouraged to use independent read-only exploration, review, validation, and release-check lanes. | `~/.codex/AGENTS.md`, `docs/parallel-first-agent-execution.md` | Instruction-policy feature, not a hardcoded scheduler. |

### Update and release experience

| Feature | User-facing behavior | Main entry points | Notes |
| --- | --- | --- | --- |
| Open Codex-aware npm update detection | Update prompts check `@leonw24/open-codex`, not upstream `@openai/codex`. | npm/bun update check | Keeps prompts aligned with the package users can install. |
| Fork-correct upgrade commands | Update actions install `@leonw24/open-codex@latest`. | update prompt/action | Avoids upstream package identity leaks. |
| Inline release notes in update prompt | Startup update prompt shows concrete release-note bullets, with a full release link as fallback. | update prompt, GitHub release fetch | Added in `2615364e34`. Release bodies must contain concrete markdown bullets. |
| Release-fast local publish path | Local release builds use `release-fast` and publish Linux x64 npm payload directly. | `cargo build --profile release-fast`, npm packaging scripts | Current direct publish flow includes GitHub release body, npm publish, and install/version smoke test. |
| npm download chart refresh | README badges/chart track npm download visibility. | `.github/npm-weekly-downloads.svg` | Maintained by periodic chart update commits. |

### Status-line and situational awareness

| Feature | User-facing behavior | Main entry points | Notes |
| --- | --- | --- | --- |
| Token throughput status item | Optional status-line item shows coarse session-average input/output token throughput. | status line config | Beta quality, useful as rough responsiveness signal. |
| Workspace git diff status item | Status line can show tracked `+added/-deleted` and untracked file count. | `workspace-changes` item | Helps notice local changes without opening `/status` or shell. |

### Side-channel workflows

| Feature | User-facing behavior | Main entry points | Notes |
| --- | --- | --- | --- |
| `/btw` side questions | Ask quick side questions without taking over the primary chat thread. | `/btw <question>` | Runs as an inline hidden thread with expected model, effort, permission, and tool behavior. |
| Default commit attribution | Open Codex can apply the fork's commit attribution identity by default. | git attribution extension/config | Implemented before the recent 0.130.x release series. |

## Release Notes

### 0.131.4 - 2026-05-21

- Fix Zellij pane scroll compatibility by disabling xterm alternate-scroll mode when Open Codex detects `ZELLIJ`, `ZELLIJ_SESSION_NAME`, or `ZELLIJ_VERSION`.
- Preserve normal terminal behavior outside Zellij, so non-Zellij sessions still get alternate-scroll handling while inside the TUI alt screen.
- Apply the same Zellij guard across Ctrl-Z suspend/resume so pane scroll remains usable after returning to Open Codex.
- Remove the redundant background-terminal footer/status text (`/ps to view · /stop to close`) now that background terminals are managed through the down panel.
- Restore 0.130.x Esc routing semantics for pending steers and rejected steers, so single Esc can steer/interrupt instead of being intercepted by double-Esc rewind detection.
- Add focused regression coverage for alternate-scroll enablement inside and outside Zellij.

### 0.131.3 - 2026-05-21

- Merge upstream `openai/codex` through `cfa16fcc2e`, bringing in 271 upstream commits after the fork point.
- Preserve Open Codex fork behavior across the merge, including memory overlay/browser, rewind/revoke UX, persistent `Shift+Tab` speed toggle, `/btw`, `/effort`, subagent tracking, and git attribution.
- Reconcile upstream ThreadSettings, `Op::UserInput`, MCP runtime environment, permission profile, rate-limit, status-surface, and package-layout changes with fork-specific runtime state.
- Keep stale-turn output guards, foreground/background task state, and fork-correct package/update identity intact after the upstream merge.
- Restore the 0.130.x user-query history highlighting with visible divider rows and the cyan `User ›` label after the upstream history-cell split.
- Restore the Rust workspace release version so built binaries report `0.131.3` instead of upstream source-build `0.0.0`.

### 0.131.2 - 2026-05-20

- Fix slash popup navigation: use keymap `list.move_up/down` instead of hardcoded `Ctrl-P/N`, fixing popup only moving upward.
- Open slash popup immediately on bare `/` without paste-burst flush delay.
- Deduplicate service tier commands that collide with builtin slash command names.
- Restore top/bottom border on chat composer to visually separate history from input.
- Clarify `/subagents` description: closed rows are reviewable, not quota holders.
- Support `CODEX_RESUME_COMMAND_NAME` env var for fork-specific resume command formatting.

### 0.131.1 - 2026-05-19

- Merge upstream Codex 0.131.0 into Open Codex.
- Preserve fork runtime features across the merge: memory overlay/browser, persistent `Shift+Tab` speed toggle, Claude-style rewind/revoke, `/btw`, `/effort`, subagent quota reclamation, background subagent tracking, git attribution, and inline update release notes.
- Restore fork-correct npm packaging and update diagnostics so install/update guidance targets `@leonw24/open-codex`.
- Keep the release publishable through the local `release-fast` path with concrete GitHub release notes for the startup update prompt.

## Recently Important Fixes

| Commit | Date | Summary |
| --- | --- | --- |
| `d587d950bc` | 2026-05-19 | Fix resume state restoration for streamed assistant deltas, reasoning effort, and fast/standard service tier. |
| `2615364e34` | 2026-05-18 | Show inline release notes in the update prompt. |
| `b6f62e4867` | 2026-05-17 | Fix revoke restore scope and release notes link. |
| `0d20c120d8` | 2026-05-16 | Fix rewind restore and Esc interrupt routing. |
| `81f61c9aa4` | 2026-05-15 | Add persistent `Shift+Tab` reasoning speed toggle. |
| `0505d0528a` | 2026-05-15 | Document persistent `Shift+Tab` speed toggle. |
| `df5d9799de` | 2026-05-14 | Fix rewind picker session scope. |
| `eb47a50088` | 2026-05-14 | Improve rewind code restore UX. |
| `11a5ff07e4` | 2026-05-14 | Document memory overlay metrics and subagent GC. |
| `bfa6fa9890` | 2026-05-14 | Add memory browser and preserve resume effort. |
| `5348fb6fcd` | 2026-05-14 | Fix completed subagent quota reclamation. |
| `ba54960a86` | 2026-05-14 | Add memory overlay status diagnostics. |
| `8670c2c842` | 2026-05-13 | Fix subagent completion wakeups. |
| `4cf2052d1c` | 2026-05-12 | Clarify Open Codex wrapper positioning. |
| `e42cbb9d66` | 2026-05-12 | Update README for BTW and background task UX. |

## README Sync Checklist

When adding or changing a feature:

1. Add or update the feature row in this file.
2. If the feature is user-facing and stable, mirror it in README `Current Delta and Roadmap`.
3. If the feature is part of a release, include concrete bullets in the GitHub release body.
4. If the feature changes npm/package identity, verify update prompt copy and install smoke tests.
5. If the feature changes session, memory, or subagent behavior, include a focused regression test and note the slash command or keybinding entry point.
