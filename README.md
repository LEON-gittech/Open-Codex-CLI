<p align="center">
  <img src="https://raw.githubusercontent.com/LEON-gittech/Open-Codex-CLI/main/.github/open-codexcli-icon.png" alt="Open Codex CLI icon" width="180" />
</p>

<h1 align="center">Open Codex CLI</h1>

<p align="center">
  A community-maintained Codex CLI fork that stays close to upstream while making room for openly developed CLI improvements.
</p>

<p align="center">
  Install with <code>npm install -g @leonw24/open-codex</code>, then run <code>open-codex</code>.
</p>

<p align="center">
  <a href="https://www.npmjs.com/package/@leonw24/open-codex"><img alt="npm weekly downloads" src="https://img.shields.io/npm/dw/%40leonw24%2Fopen-codex?label=weekly%20downloads" /></a>
  <a href="https://www.npmjs.com/package/@leonw24/open-codex"><img alt="npm total downloads" src="https://img.shields.io/npm/dt/%40leonw24%2Fopen-codex?label=total%20downloads" /></a>
</p>

<p align="center">
  <a href="https://www.npmjs.com/package/@leonw24/open-codex"><img src="https://raw.githubusercontent.com/LEON-gittech/Open-Codex-CLI/main/.github/npm-weekly-downloads.svg" alt="Open Codex CLI weekly npm downloads chart" width="960" /></a>
</p>

<p align="center">
  <a href="#english"><img alt="English" src="https://img.shields.io/badge/English-default-111111?style=for-the-badge" /></a>
  <a href="#简体中文"><img alt="简体中文" src="https://img.shields.io/badge/简体中文-switch-444444?style=for-the-badge" /></a>
</p>

<p align="center">
  <img src="https://raw.githubusercontent.com/LEON-gittech/Open-Codex-CLI/main/.github/open-codex-cli-unleashed.png" alt="Open Codex CLI unleashed banner" width="960" />
</p>

<p align="center">
  GitHub README does not allow JavaScript-based language toggles, so this page uses collapsible language sections as the practical equivalent.
</p>

---

<details open>
<summary><strong id="english">English</strong></summary>

## Motivation

Codex CLI is open source, but upstream code contributions are currently invitation-only. The upstream repository states this clearly in [docs/contributing.md](./docs/contributing.md): external pull requests that have not been explicitly invited will be closed without review.

That policy is understandable from the perspective of the upstream maintainers, but it also leaves a gap for developers who want to iterate in public, ship focused CLI improvements, and maintain a fork that can accept normal community collaboration. This repository exists to fill that gap.

The goal of **Open Codex CLI** is not to diverge for the sake of divergence. The goal is to keep a small, intentional delta on top of upstream Codex CLI, make that delta easy to understand, and keep the fork mergeable as upstream evolves.

## Current Goals

- solve real Codex CLI usage problems I run into, whether they are bugs or features worth borrowing from Claude Code
- keep improving the Codex CLI experience under `zellij` (**Fuck Off Tmux!**)

## Current Delta and Roadmap vs. Latest Upstream Codex CLI

This fork is currently based on the latest upstream `openai/codex` and adds a small set of focused CLI improvements from recent fork-specific commits:

### 1. General user-query highlighting in the TUI

From commit `c652bb8db1`:

- adds a shared `user_message_style()` for user-authored query surfaces instead of a Zellij-only transcript contrast path
- applies the same terminal-background-aware user-query highlight to the chat composer panel and to committed `UserHistoryCell` messages in chat history
- wraps historical user queries with top/bottom divider lines and keeps the `User ›` prefix visually distinct, so user prompts are easier to scan in long transcripts
- keeps the styling adaptive across light and dark terminal backgrounds rather than hardcoding a single foreground/border color

This is a general TUI readability improvement: the active user query in the chat box and previous user queries in history now share one consistent highlight language, making user-authored turns easier to identify without tying the behavior to `zellij`.

### 2. Stale turn output protection in the TUI

From commit `67b06fd086`:

- adds turn-aware filtering at the live TUI notification/render boundary
- drops stale assistant message, plan, and reasoning deltas when they belong to an older turn
- drops stale completed assistant/plan/reasoning items from live thread-item notifications
- ignores stale turn-completion notifications so an old completion cannot end the current turn
- keeps replay explicitly separate, so resumed thread snapshots can still render historical content
- adds regression coverage for stale deltas, stale completed items, and stale completions

This is a correctness-focused patch: the UI should not render output from the wrong turn, even when retry, replay, or stream timing gets messy.

### 3. Active memory staging on upstream memories

The earlier fork-only memory subsystem has been removed so the fork stays aligned with upstream's memory architecture. The current memory change is intentionally narrow:

- **Upstream memory remains the source of truth** — Codex still uses upstream `memory_summary.md`, `MEMORY.md`, topic files, skills, and `extensions/ad_hoc` consolidation. The `memories` feature keeps the upstream experimental gate and is disabled by default.

- **Active staging tool** — When the memory feature is enabled, Codex exposes `memory_stage_update`. It writes a small ad-hoc note under `~/.codex/memories/extensions/ad_hoc/notes/` for upstream consolidation and stages the same content into the current session.

- **Session Memory Overlay** — Staged entries are injected as a bounded developer-context overlay for the current session, so newly saved information can affect the next turn without waiting for a new session or background consolidation. The overlay is emitted only when its revision changes.

- **Removed conflicting fork behavior** — The old direct durable write tools, notepad file, topic frontmatter priority system, merge-write path, and custom AGENTS.md hierarchy are no longer part of the memory implementation.

This addresses the active-update gap while keeping durable memory storage and consolidation compatible with upstream.

### 4. Open Codex-aware update detection and upgrade prompts

From commit `e1e88af89d`:

- switches npm/bun update detection from upstream Codex metadata to the fork package `@leonw24/open-codex`
- updates upgrade commands and release notes links so prompts point to Open Codex instead of `@openai/codex`
- keeps the runtime check aligned with the actual package users install from npm

This is a fork-correctness patch: update notifications should describe the Open Codex release a user can actually install, not the upstream Codex CLI release stream.

### 5. Session export inside the TUI

From recent fork-specific changes:

- adds `/export <path>` for the current session transcript
- supports user-chosen filenames like `/export talk.md` or `/export talk.txt`
- writes a markdown transcript suitable for debugging, archival, and sharing

This brings a Claude Code-style export flow into the TUI without requiring external scripts or manual transcript scraping.

### 6. Reasoning effort controls and per-turn status visibility

From recent fork-specific changes:

- adds `/effort` for changing the active reasoning effort directly from the TUI
- supports one-turn high-effort markers in the user query: standalone `ulw`, `ultra`, or `xhigh` submit that turn with `xhigh` reasoning
- keeps marker-based `xhigh` current-turn-only, so it does not mutate the session's default effort
- shows the submitted per-turn effort in the `model-with-reasoning` status-line item while that foreground turn is pending or running
- restores the status line to the session/default effort when the turn completes, fails, or is interrupted

This closes the UX gap between "the request was submitted with xhigh" and "the footer still looks like high": the status line now describes the active foreground turn without making a temporary marker look like a persistent configuration change.

### 7. Parallel-first subagent planning policy

Implemented through the user-scope `~/.codex/AGENTS.md` instruction layer, with an extracted repo example in [`docs/parallel-first-agent-execution.md`](docs/parallel-first-agent-execution.md):

- overrides Codex's default conservative stance against automatic agent spawning with an explicit, more aggressive subagent policy for complex work
- classifies non-trivial work by independent investigation, review, test, docs, and validation axes before editing
- prefers read-only subagents for evidence gathering, with one final implementation lane unless edit boundaries are clearly disjoint
- sets concrete lane-count guidance, prompt requirements, stop conditions, and final-response evidence requirements

This is an instruction-policy feature rather than a hardcoded scheduler: it enables a more aggressive subagent mechanism while keeping shared-file edits coordinated.

### 8. Nonblocking background execution

From recent fork-specific TUI changes:

The feature is centered on two background lanes:

- **Terminal commands** — long-running terminal sessions continue in the background instead of keeping the main turn blocked by foreground waiting or polling. Empty `command/exec/write` interactions keep the terminal backgrounded, so normal chat input can be submitted while the shell process continues.
- **Subagents** — long-running agent work is tracked as background activity after spawn/wait tool calls complete, including agent nickname, role, status, and progress lines when available.

The shared interaction model is:

- **Manual backgrounding** — `Ctrl+B` sends the current active exec/terminal activity to the background without submitting a core op, clearing the foreground task-running UI while preserving streamed output.
- **Foreground state** — foreground model activity still drives the normal `Working` status, while background work is counted separately in the status line as `bg <n> subagent / <m> terminal`.
- **Down task panel** — pressing `Down` opens a persistent bottom-pane task panel instead of inserting a chat-stream summary. The panel updates in place and separates `Tasks`, `Subagents`, and `Terminals`.
- **List navigation** — `Up`/`Down` moves selection, `Enter` opens details, `x` stops the selected stoppable terminal, and `Esc`/`Left` closes the panel.
- **Terminal details** — terminal detail view shows status, elapsed runtime, wrapped command/task metadata, and the recent output tail. `x` stops the terminal through the same cleanup path as `/stop` when the terminal is stoppable.
- **Slash commands** — `/ps` prints a chat-history summary of running background terminals, while `/stop` terminates all running background terminal processes for the thread.
- **Subagent details** — subagent detail view shows agent title, role, task prompt, status, elapsed runtime, and progress lines. Subagents are inspectable in the background picker and switchable through the agent-thread workflow; non-stoppable subagents do not expose `x stop`.
- **Plan tasks** — the same `Down` panel also surfaces the latest visible plan/task list, including completed tasks, so task history is inspectable without relying only on the compact status-line count.
- **Completed background work** — completed background exec cells are flushed back into history once they finish, preserving the command and captured output without re-foregrounding the task.

This is the essential interaction change behind the Claude Code-style behavior: background work stays visible and controllable, but it no longer blocks normal chat flow.

### 9. Status line token throughput visibility (Beta)

From commit `85e937b855`:

- adds a configurable status-line item for session-average input/output token throughput
- renders as `in <rate> / out <rate> tok/s` when enough token-usage and turn-duration data has been observed
- falls back to `in -- / out -- tok/s` before a usable sample exists
- computes a coarse session average from completed turn usage and duration, including interval merging for overlapping active windows

This is intentionally marked **Beta**: the current value is useful as a rough responsiveness signal, but it is not yet an exact real-time throughput metric.

### 10. Workspace git status in the status line

The status line can now surface the current workspace diff through the configurable `workspace-changes` item:

- renders tracked changes as `+<added>/-<deleted>` using Git numstat data from the active workspace
- appends `?<count>` when untracked files are present, so a dirty workspace is still visible even before files are staged or modified in tracked paths
- hides itself for clean workspaces, keeping the status line compact when there is nothing to review

This is intended as a lightweight situational-awareness signal: it answers "did this session leave local changes behind?" without opening a separate shell or `/status` view.

### Near-term roadmap

The near-term roadmap is intentionally focused on a few CLI-facing improvements:

#### 1. ~~Status line throughput visibility~~ ✅ Beta

Initial support is implemented as a beta status-line item. The remaining work is accuracy: the current estimator is session-average and timing-derived, so it should not be treated as precise real-time token throughput yet.

#### 2. ~~Session export~~ ✅ Completed

Implemented as a Claude Code-style `/export` flow for the current session, with user-defined filenames like `/export talk.txt` or `/export talk.md`. This now covers the debugging, sharing, and archival use case directly inside the TUI.

#### 3. Better memory mechanics

Active memory staging is implemented on top of upstream memories (see Current Delta section 3 above). Broader durable-memory behavior should continue to follow upstream so fork changes stay additive.

#### 4. Better Zellij ergonomics

Continue improving the Codex CLI experience under `zellij`, especially around rendering, layout, contrast, and other interaction details that behave differently from plain terminal sessions or `tmux`.

#### Next focus areas

- **Better task management experience**

## Maintenance Philosophy

This fork is maintained with a conservative strategy:

- keep the fork close to upstream `openai/codex`
- merge upstream regularly rather than carrying a long-lived reimplementation
- keep fork-specific patches small, testable, and easy to reason about
- prefer user-facing CLI quality improvements over broad architectural churn
- document motivation, tradeoffs, and intended maintenance cost in the repo itself

In practice, maintenance will follow a straightforward loop:

1. track the latest upstream Codex CLI changes
2. merge upstream into this fork on a regular basis
3. re-validate the fork-specific delta
4. keep or refine only the patches that still provide clear value

The standard for changes here is simple: if a patch is not worth carrying across upstream merges, it does not belong in the fork.

## Community

Issues and pull requests are welcome in this fork.

If you have a bug report, a CLI usability problem, a design idea, or a concrete patch, please open an issue or submit a PR. Small, focused, well-explained changes are preferred over broad, unrelated edits.

The intent of this repository is to keep development open and reviewable in public, even while the upstream repository remains invitation-only for external code contributions.

## Compatibility Notes

This fork keeps the native Codex CLI implementation close to upstream, but the npm distribution uses fork-specific names so it can coexist with the official package:

- npm package: `@leonw24/open-codex`
- npm command: `open-codex`
- native binary identity: `codex-cli`

That means installing this fork from npm does not overwrite the official `codex` command from `@openai/codex`.

## Quickstart

### Option A: install from npm

The npm package is published under the `leonw24` scope:

```shell
npm install -g @leonw24/open-codex
open-codex --version
```

The current npm payload is published for Linux x64. For other platforms, build from source until this fork publishes platform artifacts for macOS, Windows, and Linux arm64.

### Option B: build from source

If you want to use this fork from source, build the Rust workspace and install the resulting binary locally.

```shell
# Clone the fork and build the CLI
git clone https://github.com/LEON-gittech/codex.git
cd codex/codex-rs
cargo build --release
```

Then choose one of these install modes:

#### Replace your local `codex`

```shell
mkdir -p ~/.local/bin
install -m 755 target/release/codex ~/.local/bin/codex
```

#### Install this fork as `open-codex`

```shell
mkdir -p ~/.local/bin
install -m 755 target/release/codex ~/.local/bin/open-codex
```

After that, run either `codex` or `open-codex`, depending on which install path you chose.

## Docs

- [Contributing](./docs/contributing.md)
- [Installing & building](./docs/install.md)
- [Open source fund](./docs/open-source-fund.md)

## License

This repository is licensed under the [Apache-2.0 License](./LICENSE).

</details>

<details>
<summary><strong id="简体中文">简体中文</strong></summary>

## 背景动机

Codex CLI 是开源的，但上游仓库当前对外部代码贡献采用 invitation-only 策略。上游仓库在 [docs/contributing.md](./docs/contributing.md) 中写得很明确：没有被明确邀请的外部 PR 会被直接关闭，不进入正常 review 流程。

从上游维护者的角度，这个策略是可以理解的；但对于想要公开迭代、持续提交 CLI 改进、并让社区可以正常协作的人来说，这中间就出现了一个空白。这也是这个 fork 存在的原因。

**Open Codex CLI** 的目标不是为了分叉而分叉，而是在尽量贴近 upstream Codex CLI 的前提下，保留一层小而明确、容易理解、也容易持续维护的公开改动。

## 当前目标

- 解决我在实际使用 Codex CLI 时遇到的体验问题，不管它们是 bug，还是值得从 Claude Code 借鉴过来的 feature
- 持续优化 Codex CLI 在 `zellij` 下的使用体验（**Fuck Off Tmux!**）

## 当前相对最新 Upstream Codex CLI 的差异与路线图

这个 fork 目前基于最新的 `openai/codex`，并在最近几条 fork 自有 commit 的基础上增加了几项聚焦的 CLI 改进：

### 1. TUI 中通用的 user query 高亮优化

来自 commit `c652bb8db1`：

- 增加共享的 `user_message_style()`，用于 user-authored query surfaces，而不是继续保留 Zellij-only 的 transcript contrast 路径
- 把同一套 terminal-background-aware user-query highlight 同时用于聊天输入框 panel 和历史聊天里的 `UserHistoryCell`
- 历史 user query 会带 top/bottom divider lines，并保留醒目的 `User ›` prefix，让长 transcript 里的用户提问更容易扫描
- 样式会根据 light/dark terminal background 自适应，而不是硬编码单一 foreground/border color

这是一个通用 TUI 可读性优化：聊天框中的当前 user query 和历史聊天中的旧 user query 使用一致的高亮语言，让用户输入回合更容易识别，而不再绑定到 `zellij` 场景。

### 2. TUI 中的 stale turn output 保护

来自 commit `67b06fd086`：

- 在 live TUI notification/render boundary 增加 turn-aware filtering
- 丢弃属于旧 turn 的 assistant message、plan、reasoning deltas
- 丢弃 live thread-item notifications 中属于旧 turn 的 completed assistant/plan/reasoning items
- 忽略旧 turn 的 completion notification，避免旧 completion 结束当前 turn
- replay 路径保持独立，resumed thread snapshot 仍然可以渲染历史内容
- 增加针对 stale deltas、stale completed items、stale completions 的回归测试覆盖

这是一个偏正确性的修复：即使在 retry、replay、stream 时序比较复杂的情况下，UI 也不应该把错误 turn 的输出渲染出来。

### 3. 基于 upstream memories 的主动 memory staging

之前 fork-only 的 memory 子系统已经移除，以保持和 upstream memory 架构对齐。当前 memory 改动刻意收窄：

- **upstream memory 仍是事实来源** — Codex 继续使用 upstream 的 `memory_summary.md`、`MEMORY.md`、topic 文件、skills 和 `extensions/ad_hoc` consolidation。`memories` feature 保留 upstream 的 experimental gate，默认不启用。

- **主动 staging tool** — 启用 memory feature 后，Codex 暴露 `memory_stage_update`。它会把小型 ad-hoc note 写到 `~/.codex/memories/extensions/ad_hoc/notes/`，供 upstream consolidation 后续吸收，同时把同一内容 staged 到当前 session。

- **Session Memory Overlay** — staged entries 会作为有预算上限的 developer-context overlay 注入当前 session，让新保存的信息不用等新 session 或后台 consolidation 就能影响下一轮。overlay 只在 revision 变化时发出。

- **已移除冲突 fork 行为** — 旧的直接 durable write tools、notepad 文件、topic frontmatter priority、merge-write 路径、自定义 AGENTS.md hierarchy 都不再属于当前 memory 实现。

这补上了主动更新缺口，同时保持 durable memory 存储和 consolidation 与 upstream 兼容。

### 4. 面向 Open Codex 的版本检测与升级提示

来自 commit `e1e88af89d`：

- 把 npm/bun 更新检测从 upstream Codex 元数据切换到 fork 的 `@leonw24/open-codex`
- 更新升级命令与 release notes 链接，使提示指向 Open Codex，而不是 `@openai/codex`
- 让运行时版本提醒与用户实际通过 npm 安装的包保持一致

这是一个 fork 正确性修复：版本提醒应该描述用户真正能安装的 Open Codex 版本，而不是 upstream Codex CLI 的发布流。

### 5. TUI 内建 session 导出

来自最近几条 fork 自有改动：

- 为当前 session 增加 `/export <path>`
- 支持用户自定义文件名，例如 `/export talk.md` 或 `/export talk.txt`
- 导出 markdown transcript，便于调试、归档和分享

这让类似 Claude Code 的会话导出能力直接进入 TUI，而不需要额外脚本或手工抓 transcript。

### 6. Reasoning effort 控制与单 turn 状态可见性

来自最近几条 fork 自有改动：

- 增加 `/effort`，可以直接在 TUI 中切换当前 reasoning effort
- 支持在用户 query 中用 standalone `ulw`、`ultra` 或 `xhigh` 触发单 turn `xhigh` reasoning
- marker 触发的 `xhigh` 保持 current-turn-only，不会修改 session 默认 effort
- 当前 foreground turn 处于 pending/running 时，`model-with-reasoning` status-line item 会显示这次提交实际使用的 per-turn effort
- turn 完成、失败或被打断后，status line 会恢复为 session/default effort

这补上了“请求实际按 xhigh 提交，但底部仍显示 high”的 UX 缺口：status line 会描述当前前台 turn，但不会把一次性的 marker 伪装成持久配置变更。

### 7. Parallel-first subagent planning policy

通过 user-scope `~/.codex/AGENTS.md` 指令层实现，并在 repo 中抽取了示例文件：[`docs/parallel-first-agent-execution.md`](docs/parallel-first-agent-execution.md)。

- 显式覆盖 Codex 原本对 automatic agent spawning 的保守/禁止姿态，为复杂任务启用更激进的 subagent policy
- 在编辑前先按 independent investigation、review、test、docs、validation 等轴判断任务是否适合拆分
- 默认优先使用 read-only subagents 收集证据，除非 edit boundary 明确 disjoint，否则保留一个最终 implementation lane
- 明确 lane 数量建议、subagent prompt 要求、stop conditions，以及 final response 的 evidence 要求

这不是硬编码 scheduler，而是 instruction-policy feature：它启用了更激进的 subagent 机制，同时避免多个执行 lane 争抢同一批文件。

### 8. 非阻塞后台执行

来自最近几条 fork 自有 TUI 改动：

这个能力围绕两个 background lanes 展开：

- **Terminal commands** — 长时间运行的 terminal session 会进入后台继续执行，不再通过前台 waiting / polling 阻塞主 turn。空的 `command/exec/write` 交互会保持 terminal backgrounded，因此 shell process 继续运行时也可以正常提交新的聊天输入。
- **Subagents** — 长时间运行的 agent work 会在 spawn/wait tool call 完成后作为 background activity 跟踪，能带上 agent nickname、role、status，以及可用的 progress lines。

共享交互模型是：

- **Manual backgrounding** — `Ctrl+B` 会把当前 active exec/terminal activity 送到后台，不提交 core op，并清掉前台 task-running UI，同时保留后续 streamed output。
- **Foreground state** — 前台模型活动仍然驱动正常的 `Working` 状态，后台工作则在 status line 中单独计数为 `bg <n> subagent / <m> terminal`。
- **Down task panel** — 按 `Down` 会打开一个持久的底部 task panel，而不是往 chat stream 插入 summary。这个 panel 会原地更新，并区分 `Tasks`、`Subagents`、`Terminals`。
- **List navigation** — `Up`/`Down` 移动选择，`Enter` 打开详情，`x` 停止当前选中的 stoppable terminal，`Esc`/`Left` 关闭 panel。
- **Terminal details** — terminal detail view 会显示 status、elapsed runtime、自动换行的 command/task metadata，以及最近的 output tail。当 terminal 是 stoppable 时，`x` 会通过和 `/stop` 相同的 cleanup path 停止它。
- **Slash commands** — `/ps` 会把 running background terminals 的摘要打印到 chat history，`/stop` 会终止当前 thread 下所有 running background terminal processes。
- **Subagent details** — subagent detail view 会显示 agent title、role、task prompt、status、elapsed runtime 和 progress lines。Subagents 可以在 background picker 中查看，并通过 agent-thread workflow 切换；不可停止的 subagents 不会暴露 `x stop`。
- **Plan tasks** — 同一个 `Down` panel 也会展示最近可见的 plan/task list，包括已经完成的 tasks，因此不用只依赖 status line 上的 compact task count 才能了解任务历史。
- **Completed background work** — background exec 完成后会把对应 cell 刷回 history，保留 command 和已捕获 output，但不会把任务重新拉回前台。

这是 Claude Code 风格体验背后的本质交互变化：后台工作仍然可见、可管理，但不会阻塞正常聊天流。

### 9. Status line token throughput visibility（Beta）

来自 commit `85e937b855`：

- 增加可配置的 status-line item，用于显示 session-average input/output token throughput
- 当已经观察到足够的 token usage 和 turn duration 数据时，显示为 `in <rate> / out <rate> tok/s`
- 在还没有可用 sample 前，回退显示为 `in -- / out -- tok/s`
- 当前用 completed turn usage 和 duration 计算粗略 session average，并对 overlapping active windows 做 interval merging

这个能力目前标记为 **Beta**：它可以作为粗略 responsiveness signal，但还不是准确的 real-time throughput metric。

### 10. Status line 中显示 workspace git status

status line 现在可以通过可配置的 `workspace-changes` item 显示当前 workspace diff：

- 使用当前 workspace 的 Git numstat 数据，把 tracked changes 显示为 `+<added>/-<deleted>`
- 当存在 untracked files 时追加 `?<count>`，因此即使文件还没 stage、也还没进入 tracked path 修改，也能看出 workspace 已经变脏
- clean workspace 下自动隐藏，避免没有改动时占用 status line 空间

这个能力是轻量级的上下文提示：它回答的是“当前 session 有没有留下本地改动”，不需要额外打开 shell 或 `/status` view。

### 近期路线图

接下来会优先推进几项面向 CLI 的改进：

#### 1. ~~Status line 中增加 token throughput 可见性~~ ✅ Beta

初步能力已经实现为 beta status-line item。剩余工作是准确性：当前 estimator 是 session-average 且依赖 turn timing，不应该当作精确的 real-time token throughput。

#### 2. ~~Session export~~ ✅ 已完成

已实现类似 Claude Code 的 `/export` 会话导出能力，支持用户自定义文件名，例如 `/export talk.txt` 或 `/export talk.md`。当前已经覆盖调试、归档、分享这一类核心使用场景。

#### 3. 更好的 memory 机制

当前已在 upstream memories 之上实现主动 memory staging（见上方当前差异第 3 项）。更广义的 durable-memory 行为应继续跟随 upstream，fork 侧只保留增量补充。

#### 4. 更好的 Zellij 使用体验

继续针对 `zellij` 下的 Codex CLI 使用体验做优化，包括渲染、布局、对比度，以及其他与普通 terminal 或 `tmux` 表现不同的交互细节。

#### 下一阶段重点

- **更好的 task management 体验**

## 维护思路

这个 fork 的维护策略是偏保守的：

- 尽量保持与 upstream `openai/codex` 接近
- 通过持续 merge upstream，而不是长期走大幅重写路线
- 保持 fork 自有 patch 小而清晰、可测试、可解释
- 优先关注面向 CLI 用户的真实体验改进，而不是无边界扩张
- 在仓库内直接记录动机、取舍和维护成本

实际维护会遵循一个比较直接的循环：

1. 跟踪 upstream Codex CLI 的最新变化
2. 定期把 upstream merge 进这个 fork
3. 重新验证 fork 的自有差异是否仍然成立
4. 只保留那些在持续 merge 成本下仍然值得维护的 patch

标准很简单：如果一个 patch 不值得长期跟随 upstream 一起维护，它就不应该存在于这个 fork 中。

## 社区协作

这个 fork 欢迎 issue 和 pull request。

如果你有 bug report、CLI 可用性问题、设计想法，或者已经有一个清晰的小 patch，都欢迎直接提 issue 或 PR。相比大而杂的改动，这里更偏好小范围、聚焦、说明充分的提交。

这个仓库的目标之一，就是在 upstream 仍然对外部代码贡献采用 invitation-only 策略的情况下，继续保持公开、可 review、可协作的开发方式。

## 兼容性说明

这个 fork 的原生 CLI 实现尽量贴近 upstream，但 npm 分发使用 fork 自己的命名，这样可以和官方包共存：

- npm 包名：`@leonw24/open-codex`
- npm 命令名：`open-codex`
- 原生二进制身份：`codex-cli`

也就是说，通过 npm 安装这个 fork 不会覆盖官方 `@openai/codex` 提供的 `codex` 命令。

## Quickstart

### 方式 A：通过 npm 安装

npm 包发布在 `leonw24` scope 下：

```shell
npm install -g @leonw24/open-codex
open-codex --version
```

当前 npm payload 已发布 Linux x64 版本。macOS、Windows 和 Linux arm64 在发布对应平台 artifact 之前，请先使用源码构建方式安装。

### 方式 B：从源码构建

如果你想从源码使用这个 fork，可以先构建 Rust workspace，再把产出的二进制安装到本地。

```shell
# 克隆仓库并构建 CLI
git clone https://github.com/LEON-gittech/codex.git
cd codex/codex-rs
cargo build --release
```

然后在下面两种安装方式中选一个：

#### 直接覆盖你本地的 `codex`

```shell
mkdir -p ~/.local/bin
install -m 755 target/release/codex ~/.local/bin/codex
```

#### 并行安装为 `open-codex`

```shell
mkdir -p ~/.local/bin
install -m 755 target/release/codex ~/.local/bin/open-codex
```

之后根据你的安装方式，运行 `codex` 或 `open-codex` 即可。

## 文档

- [Contributing](./docs/contributing.md)
- [Installing & building](./docs/install.md)
- [Open source fund](./docs/open-source-fund.md)

## 许可证

本仓库使用 [Apache-2.0 License](./LICENSE)。

</details>
