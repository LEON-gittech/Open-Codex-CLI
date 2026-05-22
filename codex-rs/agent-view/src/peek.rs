//! Read the tail of a rollout file and reduce it to a flat list of display
//! lines for the peek panel.

use std::path::Path;
use std::path::PathBuf;

use anyhow::Context as _;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::Turn;
use codex_app_server_protocol::UserInput;
use codex_app_server_protocol::build_turns_from_rollout_items;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::RolloutLine;
use tokio::fs;

/// What the peek panel renders. One row per visible item.
#[derive(Debug, Clone)]
pub struct PeekContent {
    pub path: PathBuf,
    pub turns: Vec<Turn>,
    pub rendered: Vec<PeekLine>,
    pub truncated: bool,
}

#[derive(Debug, Clone)]
pub struct PeekLine {
    /// Short label for the role / item kind (e.g. "user", "agent").
    pub kind: String,
    /// Single-line preview of the item.
    pub text: String,
}

/// Default cap on items shown so peeking a huge rollout stays cheap.
pub const DEFAULT_PEEK_LIMIT: usize = 60;

/// Load the rollout at `path` and reduce it to a [`PeekContent`] suitable for
/// the panel. Limits the returned rendered lines to `max_items` (most recent).
pub async fn load_peek(path: &Path, max_items: usize) -> anyhow::Result<PeekContent> {
    let bytes = fs::read(path)
        .await
        .with_context(|| format!("reading {}", path.display()))?;
    let items = parse_rollout(&bytes)?;
    let turns = build_turns_from_rollout_items(&items);
    let mut rendered = Vec::new();
    for turn in &turns {
        for item in &turn.items {
            if let Some(line) = render_item(item) {
                rendered.push(line);
            }
        }
    }
    let truncated = rendered.len() > max_items;
    if truncated {
        let drop = rendered.len() - max_items;
        rendered.drain(0..drop);
    }
    Ok(PeekContent {
        path: path.to_path_buf(),
        turns,
        rendered,
        truncated,
    })
}

fn parse_rollout(bytes: &[u8]) -> anyhow::Result<Vec<RolloutItem>> {
    let text = std::str::from_utf8(bytes).context("rollout file is not utf-8")?;
    let mut items = Vec::new();
    for (line_no, raw) in text.lines().enumerate() {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        match serde_json::from_str::<RolloutLine>(trimmed) {
            Ok(line) => items.push(line.item),
            Err(err) => {
                tracing::debug!(
                    line_no = line_no + 1,
                    error = %err,
                    "skipping unparseable rollout line"
                );
            }
        }
    }
    Ok(items)
}

fn render_item(item: &ThreadItem) -> Option<PeekLine> {
    match item {
        ThreadItem::UserMessage { content, .. } => Some(PeekLine {
            kind: "user".to_string(),
            text: render_user_input(content),
        }),
        ThreadItem::AgentMessage { text, .. } => Some(PeekLine {
            kind: "agent".to_string(),
            text: trim_one_line(text, 200),
        }),
        ThreadItem::Reasoning {
            summary, content, ..
        } => {
            let mut buf = String::new();
            for part in summary.iter().chain(content.iter()) {
                if !buf.is_empty() {
                    buf.push(' ');
                }
                buf.push_str(part);
            }
            if buf.is_empty() {
                return None;
            }
            Some(PeekLine {
                kind: "reasoning".to_string(),
                text: trim_one_line(&buf, 160),
            })
        }
        ThreadItem::CommandExecution { command, .. } => Some(PeekLine {
            kind: "exec".to_string(),
            text: trim_one_line(command, 160),
        }),
        ThreadItem::FileChange { changes, .. } => Some(PeekLine {
            kind: "edit".to_string(),
            text: changes
                .iter()
                .map(|c| c.path.as_str())
                .collect::<Vec<_>>()
                .join(", "),
        }),
        ThreadItem::McpToolCall { server, tool, .. } => Some(PeekLine {
            kind: "mcp".to_string(),
            text: format!("{server}/{tool}"),
        }),
        ThreadItem::DynamicToolCall {
            tool, namespace, ..
        } => Some(PeekLine {
            kind: "tool".to_string(),
            text: match namespace {
                Some(ns) => format!("{ns}/{tool}"),
                None => tool.clone(),
            },
        }),
        ThreadItem::CollabAgentToolCall {
            tool,
            receiver_thread_ids,
            ..
        } => Some(PeekLine {
            kind: "subagent".to_string(),
            text: format!(
                "{tool:?} → {}",
                receiver_thread_ids
                    .first()
                    .map(String::as_str)
                    .unwrap_or("-")
            ),
        }),
        ThreadItem::Plan { text, .. } => Some(PeekLine {
            kind: "plan".to_string(),
            text: trim_one_line(text, 160),
        }),
        ThreadItem::WebSearch { query, .. } => Some(PeekLine {
            kind: "search".to_string(),
            text: trim_one_line(query, 160),
        }),
        ThreadItem::HookPrompt { .. }
        | ThreadItem::ImageView { .. }
        | ThreadItem::ImageGeneration { .. }
        | ThreadItem::EnteredReviewMode { .. }
        | ThreadItem::ExitedReviewMode { .. }
        | ThreadItem::ContextCompaction { .. } => None,
    }
}

fn render_user_input(content: &[UserInput]) -> String {
    let mut buf = String::new();
    for piece in content {
        match piece {
            UserInput::Text { text, .. } => {
                if !buf.is_empty() {
                    buf.push(' ');
                }
                buf.push_str(text);
            }
            UserInput::Image { url, .. } => {
                if !buf.is_empty() {
                    buf.push(' ');
                }
                buf.push_str(&format!("[image {url}]"));
            }
            UserInput::LocalImage { path, .. } => {
                if !buf.is_empty() {
                    buf.push(' ');
                }
                buf.push_str(&format!("[image {}]", path.display()));
            }
            UserInput::Skill { name, .. } => {
                if !buf.is_empty() {
                    buf.push(' ');
                }
                buf.push_str(&format!("/{name}"));
            }
            UserInput::Mention { name, .. } => {
                if !buf.is_empty() {
                    buf.push(' ');
                }
                buf.push_str(&format!("@{name}"));
            }
        }
    }
    trim_one_line(&buf, 200)
}

fn trim_one_line(input: &str, max: usize) -> String {
    let single = input.replace(['\n', '\r'], " ");
    let trimmed: String = single.chars().take(max).collect();
    if input.chars().count() > max {
        format!("{trimmed}…")
    } else {
        trimmed
    }
}
