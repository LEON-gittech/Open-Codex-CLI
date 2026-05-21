# Open Codex × upstream openai/codex 真实 merge — Handoff

## TL;DR

正在 `/home/admin/zzw/tmp/codex-main-merge`（branch `merge-upstream-2026-05-20`）执行真实 git merge，目的是建立 fork 与 `origin/main`（openai/codex）的祖先关系并吸收 upstream 0.131.0 之后的改动，让 GitHub 不再显示 "271 commits behind"。

当前 merge 进行中（未 commit）。Phase 1（49 个 AA 冲突）已解决 41 个，剩 **8 个 chatwidget AA 文件**。还有 **164 个 UU + 4 个 UD/DU** 冲突没碰。

继续工作直接在这个 worktree 里跑：merge 状态保留在 index 中，所有已解决文件都已 staged。

## 关键路径与上下文

- **Worktree**: `/home/admin/zzw/tmp/codex-main-merge`
- **Branch**: `merge-upstream-2026-05-20`（未 push）
- **Backup branch**: `backup/before-real-merge-20260520`（main 上 merge 之前的备份点 = aad25d9a50）
- **Main worktree**: `/home/admin/zzw/tmp/codex-main` 上 main 分支保持干净，0.131.2 已发布
- **Remote 命名**:
  - `origin` → `https://github.com/openai/codex`
  - `fork` → `https://github.com/LEON-gittech/Open-Codex-CLI`

## 现状快照

```
Total conflicts:  217
- AA (add/add):    49 → 解决 41，剩 8
- UU (content):   164 → 0 解决
- UD/DU:            4 → 0 解决

Staged files:     748 (含 AA 已解决的 + upstream 新增的非冲突文件)
```

## 决策记录（已应用）

### 总体策略

1. **Fork-specific 功能必须保留**（按 commit memory + 实际代码审查）：
   - Open Codex URLs（`OPEN_CODEX_REPO_URL`, `OPEN_CODEX_RELEASE_NOTES_URL`）
   - `CODEX_RESUME_COMMAND_NAME` env var（fork 0.131.2 加的）
   - Stale-turn output guard（fork 关键安全特性）
   - Background subagent activity tracking（`background_agents::sync_collab_agent_background_activity`）
   - Foreground turn running 状态（`restore_foreground_turn_activity`, `foreground_turn_running`）
   - Persistent constructor fields（`environment_manager`, `task_backgrounded`, `background_activities`, `last_plan_update_items`, `status_line_workspace_changes*`）
   - Memory overlay/browser、`/btw`、`/effort`、`/subagents`（这些大概率在 UU 阶段才会冲突）

2. **Upstream API 改名/重构默认 take theirs**：
   - `set_permission_profile` → `set_permission_profile_from_session_snapshot(PermissionProfileSnapshot)`
   - `permission_profile` 字段 → `active_permission_profile`
   - `Constrained::allow_only(...)` 包装：upstream 直接传 snapshot，不需要 wrap
   - `resume_command` API 多了 `resume_hint`：手工合并保留 fork env var 支持 + 新增 hint 函数
   - `PermissionProfile::Managed { network: NetworkSandboxPolicy::Enabled, file_system: ManagedFileSystemPermissions::Unrestricted }` (替代 fork 旧 `AppServerPermissionProfile::Managed { network: PermissionProfileNetworkPermissions { enabled: true }, ... }.into()`)

3. **Visibility 调整 take theirs**：upstream 有时把 `pub(crate)` 收窄为 `pub(super)` 或 `fn`，按 upstream 的 module boundary 走

### 已解决文件清单（已 staged）

**Take theirs（41 个）— 安全 upstream feature 或 API rename**：
```
codex-rs/cli/src/doctor.rs
codex-rs/cli/src/doctor/output.rs
codex-rs/cli/src/doctor/output/detail.rs
codex-rs/cli/src/doctor/runtime.rs
codex-rs/cli/src/doctor/updates.rs
codex-rs/cli/src/state_db_recovery.rs
codex-rs/core-plugins/src/remote/share/checkout.rs
codex-rs/core/src/config/resolved_permission_profile.rs
codex-rs/core/src/tasks/lifecycle.rs
codex-rs/core/tests/suite/compact_remote_parity.rs
codex-rs/core/tests/suite/mcp_turn_metadata.rs
codex-rs/exec-server/src/relay.rs
codex-rs/exec-server/tests/relay.rs
codex-rs/ext/extension-api/src/capabilities/mod.rs
codex-rs/ext/guardian/Cargo.toml
codex-rs/ext/guardian/src/lib.rs
codex-rs/ext/memories/src/extension.rs           # 改 enable build_memory_tool_developer_instructions; fork 自己的 memory overlay 在 TUI 层另外实现
codex-rs/thread-store/src/thread_metadata_sync.rs
codex-rs/tui/src/app/pets.rs
codex-rs/tui/src/pets/image_protocol.rs
codex-rs/tui/src/bottom_pane/snapshots/.../slash_popup_pet.snap
codex-rs/tui/src/app/tests/session_summary.rs    # API 改名 resume_command → resume_hint
codex-rs/tui/src/history_cell/mod.rs             # import 调整
codex-rs/tui/src/history_cell/patches.rs
codex-rs/tui/src/history_cell/separators.rs
codex-rs/tui/src/history_cell/session.rs
codex-rs/tui/src/history_cell/snapshots/.../session_info_availability_nux_tooltip_snapshot.snap
codex-rs/tui/src/history_cell/tests.rs           # PermissionProfile API rename
codex-rs/tui/src/chatwidget/input_flow.rs        # 仅 visibility pub(crate) → pub(super)
codex-rs/tui/src/chatwidget/settings_popups.rs   # 仅 doc comment
codex-rs/tui/src/chatwidget/tool_requests.rs     # ThreadId 解析逻辑改进
codex-rs/tui/src/chatwidget/input_restore.rs     # restore flow 改进
codex-rs/tui/src/chatwidget/model_popups.rs      # visibility 收窄
codex-rs/tui/src/chatwidget/settings.rs          # set_permission_profile_from_session_snapshot
codex-rs/tui/src/chatwidget/session_flow.rs      # PermissionProfileSnapshot wrapping
```

**Take ours（fork 关键功能保留，3 个）**：
```
codex-rs/tui/src/history_cell/notices.rs         # Open Codex URLs + 测试
codex-rs/tui/src/chatwidget/constructor.rs       # fork-specific fields
codex-rs/tui/src/chatwidget/streaming.rs         # restore_foreground_turn_activity / foreground_turn_running
```

**手工 merge（3 个）**：
```
codex-rs/utils/cli/src/resume_command.rs         # 保留 CODEX_RESUME_COMMAND_NAME + 加 resume_hint
codex-rs/tui/src/chatwidget/tool_lifecycle.rs    # take theirs + 补 background_agents::sync_collab_agent_background_activity
codex-rs/tui/src/chatwidget/replay.rs            # take ours + 补 McpToolCallStatus::InProgress arm
```

## 还要做的事

### Phase 1 残余（8 个 chatwidget AA）

按 diff 大小升序：

1. **`windows_sandbox_prompts.rs`** (16 行 diff) — 全是 `permission_profile` → `active_permission_profile` 改名。**Take theirs**。
2. **`status_controls.rs`** (23 行) — 待查。
3. **`permission_popups.rs`** (22 行) — 待查（很可能 PermissionProfile API 改名）。
4. **`input_submission.rs`** (31 行) — 待查（fork 可能有 paste-burst 改动）。
5. **`turn_runtime.rs`** (49 行) — **高风险**：fork 的 turn lifecycle 关键，必须查 fork-specific 改动（多半涉及 stale-turn guard / background tracking）。
6. **`rate_limits.rs`** (59 行) — 待查。
7. **`interaction.rs`** (107 行) — **高风险**：fork 在 0.131.2 改过键盘处理（`Down` 键和 background activity 交互）。原 working tree 上还有未提交改动 stash 过：`pre-merge-stash 20260520`，已经 pop 进 main worktree。
8. **`command_lifecycle.rs`** (152 行) — **高风险**：command 执行链路核心，fork 可能有自定义 hook。

**操作模板**：
```bash
# 看真实 diff
diff <(git show ":2:<file>") <(git show ":3:<file>") --normal | head -80

# 如果是纯 API rename 或 import 调整 → take theirs
git checkout --theirs -- <file>; git add <file>

# 如果 fork 有特殊功能 → take ours，再补 upstream 真正的新功能
git checkout --ours -- <file>; # 然后 Edit 加 upstream 新东西
git add <file>

# 手工 merge：take ours 后基于 :3 的 diff 补改动；或者 take theirs 后补 fork 关键行
```

### Phase 2: 164 个 UU 冲突 + 4 个 UD/DU

UU 冲突分布大头（按目录估算）：
- `codex-rs/core/src/*` 和 `codex-rs/core/tests/suite/*` — 协议、session、tools handlers
- `codex-rs/app-server/*` — 一堆 request_processors
- `codex-rs/app-server-protocol/*` — schema 改动
- `codex-rs/tui/src/*` — chatwidget.rs（剩下的整文件）、bottom_pane/、status/、keymap、resume_picker、slash_command、debug_config、main、lib
- `codex-rs/utils/cli/src/lib.rs`
- `codex-rs/cli/src/main.rs`
- `codex-rs/Cargo.toml` 和 `Cargo.lock`（最后处理）
- `docs/config.md`

**UD/DU**（一边删一边改）：
```
UD codex-rs/core/src/arc_monitor.rs                  # upstream 删了
UD codex-rs/core/src/tools/spec_plan_model_tests.rs  # upstream 删了
UD codex-rs/ext/git-attribution/src/lib.rs           # upstream 删了；fork 必须保留 git attribution feature
UD codex-rs/tui/src/bottom_pane/snapshots/...zellij_empty_composer.snap  # 看是否还需要
```

特别注意 **`codex-rs/ext/git-attribution/src/lib.rs`**：upstream 删了，但 fork 默认开启了 git attribution feature。要保留 ours 然后看 upstream 是把功能搬到别处还是真删了。

### Phase 3-4: 收尾 + Final commit

按推荐 phase order：
1. core/protocol UU（架构层）
2. tui UU（保留 fork features）
3. app-server / cli / docs UU
4. UD/DU（4 个）
5. `cargo build --profile release-fast` 跑全量编译
6. `cargo test -p codex-tui -p codex-core` 跑核心测试
7. 解 snapshot 冲突：`cargo insta accept -p codex-tui` 仅在 review 后
8. `git commit` 形成真正 merge commit
9. Push to fork → 验证 GitHub "0 commits behind"

## 预估剩余工作量

- 8 个 chatwidget AA：1-2h（已经有模式，但 turn_runtime/interaction/command_lifecycle 是高风险）
- 164 UU：3-6h（大部分是 API rename / import；少部分需要保留 fork features）
- 4 UD/DU：30min（git-attribution 要 careful）
- 编译 + 测试 + snapshot：1-2h
- **总计**：6-12h，建议分 2-3 个 session

## Fork-specific 功能 cheat sheet

下面这些任何冲突里出现都要保留 ours：

```
# 关键字检查 grep
grep -E "OPEN_CODEX|CODEX_RESUME_COMMAND_NAME|background_agents|sync_collab_agent_background|restore_foreground_turn_activity|foreground_turn_running|stale_turn_id|should_accept_live_turn|task_backgrounded|status_line_workspace_changes|environment_manager"

# 已知 fork-specific module
codex-rs/tui/src/chatwidget/background_agents.rs
codex-rs/tui/src/chatwidget/btw_*.rs (如果有)
codex-rs/tui/src/history_cell/btw.rs
codex-rs/ext/git-attribution/

# Slash commands fork-specific
SlashCommand::Btw, SlashCommand::Effort, SlashCommand::MultiAgents, SlashCommand::Side, SlashCommand::Goal
```

## 测试 / 编译命令

```bash
# 在 worktree 内
cd /home/admin/zzw/tmp/codex-main-merge

# 编译（必须 clang，磁盘吃紧用 TMPDIR）
export TMPDIR=/home/admin/zzw/tmp/
CC=clang CXX=clang++ cargo build --profile release-fast --bin codex 2>&1 | tail -20

# 测试（仅核心 crate；先不跑 --all-features）
cargo test -p codex-tui 2>&1 | tail -30
cargo test -p codex-core 2>&1 | tail -30

# Snapshot review
cargo insta pending-snapshots -p codex-tui

# 验证 merge 是否真正建立祖先关系（commit 后跑）
git merge-base --is-ancestor origin/main HEAD && echo "ancestor OK"
```

## 回滚

如果 merge 出问题想重来：

```bash
# 在 codex-main-merge worktree 内
git merge --abort

# 或者整个 worktree 弃掉，从 main worktree 重做
cd /home/admin/zzw/tmp/codex-main
git worktree remove --force /home/admin/zzw/tmp/codex-main-merge
git branch -D merge-upstream-2026-05-20
git worktree add -b merge-upstream-2026-05-20-take2 /home/admin/zzw/tmp/codex-main-merge HEAD
```

main 分支始终是 aad25d9a50（0.131.2 已发布），`backup/before-real-merge-20260520` 提供了第二层保险。

## 发版（merge 完成后）

按 `~/.claude/projects/-home-admin-zzw-tmp-codex-main/memory/project_open_codex_release.md`：

1. Bump 到 **0.132.0**（major upstream catch-up，建议跨 minor）
2. `release-fast` build with `CC=clang`
3. npm pack + publish（platform `--tag linux-x64`，meta `--access public`）
4. 用 granular access token（`[REDACTED_SECRET]` bypass 2FA）
5. `gh release create v0.132.0 --repo LEON-gittech/codex` with concrete bullets
6. 更新 `FEATURES.md` Release Notes
7. Install smoke test: `npm install -g @leonw24/open-codex@0.132.0 && open-codex --version`

GitHub release body 应该至少包含：
- "Catch up with upstream openai/codex through cfa16fcc2e (X commits)"
- 用户面向的功能变化（看 271 个 upstream commits 找重要的）
- Fork features 保留确认列表

## 联系点

- 主 worktree 上有未跟踪文件：`.qoder/`, `.serena/`, `.tmp/`, `dario_agi_notes.txt`, `package-lock.json` — merge 不影响
- 之前 stash 过的 `interaction.rs` 改动已经 pop 回 main worktree（不在 merge worktree 里）
- npm token 已更新为 granular bypass-2FA token
