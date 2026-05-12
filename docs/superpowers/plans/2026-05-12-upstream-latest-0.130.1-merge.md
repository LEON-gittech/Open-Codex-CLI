# Upstream Latest 0.130.1 Merge Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Merge latest `openai/codex` upstream `origin/main` into Open-Codex, preserve fork UX invariants, then release Open-Codex `0.130.1` to GitHub and npm.

**Architecture:** Use the isolated worktree `/home/admin/zzw/tmp/codex-upstream-latest-0.130.1` as the integration lane. Prefer upstream architecture for broad platform changes, but preserve fork-owned TUI behavior where upstream still lacks equivalent non-blocking background execution, Down panel task management, `/btw`, `/effort`, statusline additions, and npm Open-Codex branding. Do not merge partial fixes back to `/home/admin/zzw/tmp/codex-main` until the integration worktree passes focused gates.

**Tech Stack:** Rust workspace under `codex-rs`, TUI `ratatui`, Cargo `release-fast`, `sccache`, npm packages `@leonw24/open-codex` and platform payload package.

---

## Current State

- Worktree: `/home/admin/zzw/tmp/codex-upstream-latest-0.130.1`
- Integration branch: `integration/upstream-latest-0.130.1`
- Merge source: `origin/main`
- Merge target after validation: `/home/admin/zzw/tmp/codex-main` branch `main`
- Current merge status: conflicts are resolved in the index, but TUI compile/test compatibility is still in progress.
- Known deliberate merge decision: `codex-rs/tui/src/chatwidget.rs`, `slash_dispatch.rs`, `status_surfaces.rs`, and high-risk TUI behavior files preserve fork semantics first, then receive minimal upstream API compatibility patches.

## Non-Negotiable Fork Invariants

- Background terminal and subagent execution must remain non-blocking for the foreground composer.
- Down panel must remain the management surface for subagents, terminals, and tasks.
- Background task completion notices must remain visible in foreground history where the fork already provides them.
- Statusline must keep model/effort, background counts, task progress, git workspace changes, token throughput beta, and configured custom items.
- `/btw` must remain lightweight and interactive enough for follow-up work, not a hidden `/side` clone.
- `/effort` and inline `ulw` / `ultra` / `xhigh` query markers must keep working.
- npm package identity remains Open-Codex: bin command `open-codex`, package `@leonw24/open-codex`, GitHub links point to `LEON-gittech/Open-Codex-CLI`.
- Commit attribution default remains `Open Codex <hff582580@gmail.com>`.

## Task 1: Stabilize Merge Compile Boundary

**Files:**
- Modify: `codex-rs/tui/src/chatwidget.rs`
- Modify: `codex-rs/tui/src/chatwidget/slash_dispatch.rs`
- Modify: `codex-rs/tui/src/chatwidget/status_surfaces.rs`
- Modify: `codex-rs/tui/src/app/thread_session_state.rs`
- Modify: `codex-rs/tui/src/app/session_lifecycle.rs`
- Modify: `codex-rs/tui/src/app/side.rs`
- Modify: `codex-rs/tui/src/app/event_dispatch.rs`
- Modify only if required by compiler: `codex-rs/tui/src/chatwidget/tests/**`, `codex-rs/tui/src/app/tests.rs`

- [x] **Step 1: Resolve mechanical merge conflicts**

Completed before this spec:

```bash
git merge --no-ff origin/main
git diff --name-only --diff-filter=U
```

Expected current output:

```text
<no unresolved paths>
```

- [x] **Step 2: Preserve upstream moved commit attribution crate with fork default**

Completed before this spec:

```rust
const DEFAULT_ATTRIBUTION_VALUE: &str = "Open Codex <hff582580@gmail.com>";
```

Location:

```text
codex-rs/ext/git-attribution/src/lib.rs
```

- [x] **Step 3: Re-run focused compile gate**

Run:

```bash
cd /home/admin/zzw/tmp/codex-upstream-latest-0.130.1/codex-rs
RUSTC_WRAPPER=sccache CC=clang cargo test -p codex-tui --lib slash_btw_requests_lightweight_side_question_while_task_running
```

Expected: compile succeeds and test passes. If it fails, fix only the first coherent compile class, then re-run this exact command.

- [x] **Step 4: Fix TUI test drift without weakening fork behavior**

If compile failures are only test-only references to upstream split fields such as `input_queue`, `status_state`, `connectors`, `review`, or `turn_lifecycle`, prefer reverting the affected upstream-only test files to the fork side instead of adding dead fields to production `ChatWidget`.

Allowed command:

```bash
git checkout HEAD -- codex-rs/tui/src/chatwidget/tests.rs codex-rs/tui/src/chatwidget/tests codex-rs/tui/src/app/tests.rs
```

Then re-apply only test updates needed for fork features and rerun the focused gate.

## Task 2: Preserve Fork Feature Gates

**Files:**
- Modify: `codex-rs/tui/src/chatwidget.rs`
- Modify: `codex-rs/tui/src/chatwidget/slash_dispatch.rs`
- Modify: `codex-rs/tui/src/bottom_pane/**`
- Modify: `codex-rs/core/src/tools/handlers/multi_agents/**`
- Modify: `codex-rs/core/src/tools/handlers/unified_exec/**`
- Modify: `codex-rs/core/tests/suite/unified_exec.rs`

- [x] **Step 1: Run fork TUI feature tests**

Run:

```bash
cd /home/admin/zzw/tmp/codex-upstream-latest-0.130.1/codex-rs
RUSTC_WRAPPER=sccache CC=clang cargo test -p codex-tui --lib slash_btw_requests_lightweight_side_question_while_task_running
RUSTC_WRAPPER=sccache CC=clang cargo test -p codex-tui --lib btw_notifications_collect_answer_and_emit_completion
RUSTC_WRAPPER=sccache CC=clang cargo test -p codex-tui --lib down_panel_lists_latest_plan_tasks
RUSTC_WRAPPER=sccache CC=clang cargo test -p codex-tui --lib status_line_setup_popup
RUSTC_WRAPPER=sccache CC=clang cargo test -p codex-tui --lib npm_registry
```

Expected: all pass. Failures in user-visible TUI output must be handled with snapshot review, not blind acceptance.

- [x] **Step 2: Run non-blocking background core tests**

Run:

```bash
cd /home/admin/zzw/tmp/codex-upstream-latest-0.130.1/codex-rs
RUSTC_WRAPPER=sccache CC=clang cargo test -p codex-core unified_exec_keeps_long_running_session_after_turn_end
RUSTC_WRAPPER=sccache CC=clang cargo test -p codex-core unified_exec_interrupt_preserves_long_running_session
```

Expected: both pass. If upstream unified exec changes conflict with fork behavior, keep fork non-blocking semantics.

## Task 3: Validate Upstream Compatibility Gates

**Files:**
- Modify only if tests identify real integration issues.

- [x] **Step 1: App-server protocol schema compatibility**

Run:

```bash
cd /home/admin/zzw/tmp/codex-upstream-latest-0.130.1/codex-rs
RUSTC_WRAPPER=sccache CC=clang cargo test -p codex-app-server-protocol
```

Expected: pass.

- [x] **Step 2: TUI focused module coverage**

Run:

```bash
cd /home/admin/zzw/tmp/codex-upstream-latest-0.130.1/codex-rs
RUSTC_WRAPPER=sccache CC=clang cargo test -p codex-tui --lib down_x
```

Expected: pass or produce only pre-existing drift that is documented before release.

- [x] **Step 3: Format Rust code**

Run:

```bash
cd /home/admin/zzw/tmp/codex-upstream-latest-0.130.1/codex-rs
just fmt
```

Expected: no unreviewed broad formatting churn outside merged upstream + fork compatibility files.

Note: `just fmt` was updated to run Python SDK formatting through `uv tool run ruff` instead of syncing the full SDK project environment. The upstream SDK pins `openai-codex-cli-bin==0.131.0a4`, which has no manylinux x86_64 wheel on this host, and formatting does not require installing that runtime payload.

## Task 4: Version and Documentation Gate

**Files:**
- Modify: `codex-rs/Cargo.toml`
- Modify: `codex-cli/package.json`
- Modify if present/required: npm platform package metadata under `codex-cli/`
- Review/modify: `README.md`
- Review/modify: `README.zh.md` or Chinese README file if present

- [x] **Step 1: Set Open-Codex version to 0.130.1**

Check:

```bash
rg -n '"version": "0\\.|^version = "0\\.|workspace.package' codex-rs/Cargo.toml codex-cli package.json
```

Edit every Open-Codex release version that controls `open-codex --version` and npm package version to `0.130.1`.

- [x] **Step 2: README review before commit**

Run:

```bash
git log --oneline -10
rg -n "btw|effort|statusline|status line|non-blocking|background|download|npm|0\\.130" README.md README.zh.md 2>/dev/null || true
```

Expected: README remains user-oriented and concise. Add or update only user-visible feature notes if upstream merge or release changes require it.

## Task 5: Commit and Merge Back to Main

**Files:**
- All files intentionally changed by upstream merge and fork compatibility patches.

- [ ] **Step 1: Inspect final diff**

Run:

```bash
git status --short
git diff --stat
git diff --check
rg -n "<<<<<<<|=======|>>>>>>>" codex-rs docs README.md codex-cli package.json || true
```

Expected:

```text
git diff --check has no whitespace errors
no conflict markers
```

- [ ] **Step 2: Commit integration branch with required attribution**

Run:

```bash
GIT_AUTHOR_EMAIL=zzw.cs@smail.nju.edu.cn \
GIT_COMMITTER_EMAIL=zzw.cs@smail.nju.edu.cn \
git commit -m "$(cat <<'EOF'
merge upstream latest for Open-Codex 0.130.1

Merge latest upstream Codex changes while preserving Open-Codex foreground UX:
non-blocking background terminal/subagent behavior, Down panel task management,
statusline additions, lightweight /btw, /effort controls, npm identity, and
Open-Codex commit attribution.

Co-authored-by: Open Codex <hff582580@gmail.com>
EOF
)"
```

Then verify:

```bash
git log -1 --format=fuller
```

Expected author and committer emails are both `zzw.cs@smail.nju.edu.cn`, and the trailer appears exactly once.

- [ ] **Step 3: Merge integration branch into main worktree**

Run:

```bash
cd /home/admin/zzw/tmp/codex-main
git status --short
git merge --no-ff integration/upstream-latest-0.130.1
```

Expected: no unexpected dirty main-worktree files are included. Existing untracked local files such as `.serena/` remain untouched.

- [ ] **Step 4: Push GitHub**

Run:

```bash
cd /home/admin/zzw/tmp/codex-main
GIT_AUTHOR_EMAIL=zzw.cs@smail.nju.edu.cn \
GIT_COMMITTER_EMAIL=zzw.cs@smail.nju.edu.cn \
git push fork main
```

Expected: push succeeds to `LEON-gittech/Open-Codex-CLI`.

## Task 6: Release 0.130.1 to npm

**Files:**
- Build output under `codex-rs/target/release-fast`
- npm vendor payload under `codex-cli/`
- generated npm tarballs

- [ ] **Step 1: Build release-fast binary**

Run:

```bash
cd /home/admin/zzw/tmp/codex-main/codex-rs
RUSTC_WRAPPER=sccache CC=clang cargo build --profile release-fast --bin codex
./target/release-fast/codex --version
```

Expected:

```text
codex-cli 0.130.1
```

- [ ] **Step 2: Package npm payload**

Use the repo's existing npm packaging scripts. Before publishing, verify tarball metadata and embedded binary:

```bash
npm pack --json --registry=https://registry.npmjs.org
```

Expected: meta package and linux payload package versions are `0.130.1` / `0.130.1-linux-x64` as applicable, and the platform package contains the release-fast binary.

- [ ] **Step 3: Publish npm packages**

Publish platform payload first, then meta package:

```bash
npm publish <linux-payload-tarball> --tag linux-x64 --registry=https://registry.npmjs.org
npm publish <meta-tarball> --tag latest --registry=https://registry.npmjs.org
```

Expected: both publish commands succeed.

- [ ] **Step 4: Fresh install smoke test**

Run in a temp directory:

```bash
npm install @leonw24/open-codex@0.130.1 --registry=https://registry.npmjs.org
./node_modules/.bin/open-codex --version
```

Expected:

```text
codex-cli 0.130.1
```

## Acceptance Criteria

- `git diff --name-only --diff-filter=U` is empty.
- No conflict markers remain in `codex-rs`, `docs`, `README*`, or `codex-cli`.
- Focused fork tests pass:
  - `/btw`
  - Down panel task visibility
  - statusline setup
  - npm registry behavior
  - non-blocking unified exec
- App-server protocol tests pass.
- `open-codex --version` from built release-fast binary reports `codex-cli 0.130.1`.
- GitHub main branch contains the merge commit with required email and `Co-authored-by` trailer.
- npm `latest` resolves to `@leonw24/open-codex@0.130.1`.
- Fresh install from npm runs `open-codex --version` successfully and reports `0.130.1`.
