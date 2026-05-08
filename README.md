<p align="center">
  <img src="./.github/open-codexcli-icon.png" alt="Open Codex CLI icon" width="180" />
</p>

<h1 align="center">Open Codex CLI</h1>

<p align="center">
  A community-maintained Codex CLI fork that stays close to upstream while making room for openly developed CLI improvements.
</p>

<p align="center">
  Install with <code>npm install -g @leonw24/open-codex</code>, then run <code>open-codex</code>.
</p>

<p align="center">
  <a href="#english"><img alt="English" src="https://img.shields.io/badge/English-default-111111?style=for-the-badge" /></a>
  <a href="#简体中文"><img alt="简体中文" src="https://img.shields.io/badge/简体中文-switch-444444?style=for-the-badge" /></a>
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

## Targets

- solve real Codex CLI usage problems I run into, whether they are bugs or features worth borrowing from Claude Code
- keep improving the Codex CLI experience under `zellij` (**Fuck Off Tmux!**)

## Current Delta vs. Latest Upstream Codex CLI

This fork is currently based on the latest upstream `openai/codex` and adds a small set of focused CLI improvements from recent fork-specific commits:

### 1. Better transcript contrast in the TUI for Zellij

From commit `598bebc6b`:

- improves visual distinction between user-authored content and assistant-rendered content when Codex CLI is used inside `zellij`
- adjusts the TUI styling path used by user message rendering for the `zellij` case
- targets a real readability issue in `zellij`; this is not the same problem in a normal terminal session or in `tmux`

This is a usability-focused patch for the `zellij` environment: the goal is to reduce ambiguity in the chat history without changing the underlying interaction model.

For context: [Zellij](https://github.com/zellij-org/zellij) is a terminal workspace / terminal multiplexer. Compared with `tmux`, it puts more emphasis on a batteries-included user experience, richer pane behavior, built-in layouts, and more discoverable interaction patterns out of the box.

### 2. Stale turn output protection in the TUI

From commits `5800f4e9f` and `0b299d9bd`:

- adds turn-aware filtering for streamed assistant output
- prevents stale deltas from older turns from leaking into the currently active turn
- hardens replay and status handling around message deltas, reasoning deltas, and turn completion events
- adds regression coverage for the stale-turn cases that motivated the fix

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

## Roadmap

The near-term roadmap is intentionally focused on a few CLI-facing improvements:

### 1. Status line throughput visibility

Improve the Codex CLI status line so it can surface token throughput directly, instead of only showing coarse task state. The aim is to make model responsiveness easier to judge in real time.

### 2. ~~Session export~~ ✅ Completed

Implemented as a Claude Code-style `/export` flow for the current session, with user-defined filenames like `/export talk.txt` or `/export talk.md`. This now covers the debugging, sharing, and archival use case directly inside the TUI.

### 3. Better memory mechanics

Active memory staging is implemented on top of upstream memories (see Current Delta section 3 above). Broader durable-memory behavior should continue to follow upstream so fork changes stay additive.

### 4. Better Zellij ergonomics

Continue improving the Codex CLI experience under `zellij`, especially around rendering, layout, contrast, and other interaction details that behave differently from plain terminal sessions or `tmux`.

### Next focus areas

- **Background AutoDream-style consolidation** — move consolidation fully off the startup path and replace it with a background 3-gate consolidator (time ≥ 24h, ≥ 5 new sessions, no lock), using a 4-phase pipeline: Orient → Gather → Consolidate → Prune.
- **Memory staging UI** — surface staged session-memory entries in the TUI so the user can inspect what is currently overlaid before upstream consolidation picks it up.
- **Memory versioning** — keep a lightweight changelog for topic edits so agents can reason about what changed and when.
- **More proactive subagent parallel planning** — let the agent split work and dispatch parallel subagents more aggressively instead of stepping through tasks strictly serially.
- **Claude Code-style background execution** — automatically send long-running commands and agent work to the background rather than keeping the main process occupied by foreground waiting and polling.

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

## 当前相对最新 Upstream Codex CLI 的差异

这个 fork 目前基于最新的 `openai/codex`，并在最近几条 fork 自有 commit 的基础上增加了几项聚焦的 CLI 改进：

### 1. 面向 Zellij 的 TUI transcript 对比度优化

来自 commit `598bebc6b`：

- 改善了在 `zellij` 环境下用户消息与 assistant 输出之间的视觉区分度
- 调整了 `zellij` 场景下用户消息渲染路径的样式策略
- 解决的是 `zellij` 下真实存在的可读性问题，而不是普通 terminal 或 `tmux` 下的通用问题

这是一个面向 `zellij` 使用环境的可用性优化，目标是在不改变交互模型的前提下，降低 transcript 阅读时的歧义。

补充说明：[Zellij](https://github.com/zellij-org/zellij) 是一个 terminal workspace / terminal multiplexer。相比 `tmux`，它更强调开箱即用的体验、更丰富的 pane 行为、内建布局能力，以及更容易发现的交互方式。

### 2. TUI 中的 stale turn output 保护

来自 commits `642d306a7` 和 `6c27de579`：

- 为流式 assistant 输出增加了 turn-aware 过滤
- 防止旧 turn 的 delta 混入当前 active turn
- 强化了 replay、reasoning delta、turn completion 等路径下的状态处理
- 增加了针对 stale-turn 场景的回归测试覆盖

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

## 路线图

接下来会优先推进几项面向 CLI 的改进：

### 1. Status line 中增加 token throughput 可见性

改进 Codex CLI 的 status line，让它可以直接展示 token 吞吐，而不只是显示比较粗粒度的任务状态，便于更直观判断模型响应效率。

### 2. ~~Session export~~ ✅ 已完成

已实现类似 Claude Code 的 `/export` 会话导出能力，支持用户自定义文件名，例如 `/export talk.txt` 或 `/export talk.md`。当前已经覆盖调试、归档、分享这一类核心使用场景。

### 3. 更好的 memory 机制

当前已在 upstream memories 之上实现主动 memory staging（见上方当前差异第 3 项）。更广义的 durable-memory 行为应继续跟随 upstream，fork 侧只保留增量补充。

### 4. 更好的 Zellij 使用体验

继续针对 `zellij` 下的 Codex CLI 使用体验做优化，包括渲染、布局、对比度，以及其他与普通 terminal 或 `tmux` 表现不同的交互细节。

### 下一阶段重点

- **后台化的 AutoDream 式 consolidation** — 把 consolidation 完整移出启动路径，改为后台 3-gate 合并器（时间 ≥ 24h、≥ 5 个新 session、无锁），并使用 4 阶段管线：Orient → Gather → Consolidate → Prune。
- **Memory staging UI** — 在 TUI 中展示当前 staged session-memory entries，让用户能检查当前 overlay 中有什么，再等待 upstream consolidation 吸收。
- **Memory 版本管理** — 为 topic 编辑维护轻量级变更日志，让 agent 能推理内容何时发生了什么变化。
- **更主动的 subagent 并行规划** — 让 agent 能更积极地拆分任务并并行派发 subagent，而不是严格串行地一步步推进。
- **Claude Code 风格的后台执行** — 自动把长时间运行的命令和 agent 工作放到后台，而不是长时间占用主进程做前台等待或轮询。

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
