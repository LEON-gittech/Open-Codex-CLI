//! Scan `$CODEX_HOME/sessions/` for rollout files and reduce each one to a
//! display-friendly summary.

use std::path::Path;
use std::path::PathBuf;

use codex_protocol::protocol::SessionSource;
use codex_rollout::ThreadListConfig;
use codex_rollout::ThreadListLayout;
use codex_rollout::ThreadSortKey;
use codex_rollout::ThreadsPage;
use codex_rollout::get_threads_in_root;
use serde::Serialize;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

const SESSIONS_SUBDIR: &str = "sessions";
const STALE_AFTER_SECS: i64 = 5 * 60;

/// Coarse status derived from rollout metadata alone. Without a live daemon we
/// cannot tell `Running` from `Idle`, so we treat anything recently updated as
/// `Active`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    /// File mtime is newer than the staleness threshold.
    Active,
    /// File mtime is older than the staleness threshold.
    Idle,
    /// Could not derive a timestamp from the rollout file.
    Unknown,
}

impl SessionStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Idle => "idle",
            Self::Unknown => "unknown",
        }
    }
}

/// One row in the agent-view list. Built from `codex_rollout::ThreadItem` plus
/// a couple of derived fields.
#[derive(Debug, Clone, Serialize)]
pub struct SessionSummary {
    pub thread_id: Option<String>,
    pub path: PathBuf,
    pub title: String,
    pub cwd: Option<PathBuf>,
    pub git_branch: Option<String>,
    pub agent_nickname: Option<String>,
    pub agent_role: Option<String>,
    pub source: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub status: SessionStatus,
}

impl SessionSummary {
    pub fn updated_at_offset(&self) -> Option<OffsetDateTime> {
        self.updated_at
            .as_deref()
            .and_then(|s| OffsetDateTime::parse(s, &Rfc3339).ok())
    }
}

/// Walk `<codex_home>/sessions/` and return the newest `limit` sessions.
pub async fn list_sessions(
    codex_home: &Path,
    limit: usize,
) -> std::io::Result<Vec<SessionSummary>> {
    let root = codex_home.join(SESSIONS_SUBDIR);
    list_sessions_in_root(root, limit).await
}

/// Same as [`list_sessions`] but takes the explicit sessions root. Used by tests.
pub async fn list_sessions_in_root(
    root: PathBuf,
    limit: usize,
) -> std::io::Result<Vec<SessionSummary>> {
    let allowed_sources: &[SessionSource] = &[];
    let config = ThreadListConfig {
        allowed_sources,
        model_providers: None,
        cwd_filters: None,
        default_provider: "openai",
        layout: ThreadListLayout::NestedByDate,
    };
    let page: ThreadsPage =
        get_threads_in_root(root, limit.max(1), None, ThreadSortKey::UpdatedAt, config).await?;
    let now = OffsetDateTime::now_utc();
    Ok(page
        .items
        .into_iter()
        .map(|item| build_summary(item, now))
        .collect())
}

fn build_summary(item: codex_rollout::ThreadItem, now: OffsetDateTime) -> SessionSummary {
    let title = derive_title(&item);
    let source = item.source.as_ref().map(serialize_source);
    let status = derive_status(item.updated_at.as_deref(), now);
    SessionSummary {
        thread_id: item.thread_id.map(|id| id.to_string()),
        path: item.path,
        title,
        cwd: item.cwd,
        git_branch: item.git_branch,
        agent_nickname: item.agent_nickname,
        agent_role: item.agent_role,
        source,
        created_at: item.created_at,
        updated_at: item.updated_at,
        status,
    }
}

fn derive_title(item: &codex_rollout::ThreadItem) -> String {
    if let Some(preview) = item
        .preview
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        return truncate(preview, 80);
    }
    if let Some(first) = item
        .first_user_message
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        return truncate(first, 80);
    }
    if let Some(nick) = item.agent_nickname.as_deref() {
        return nick.to_string();
    }
    "(untitled session)".to_string()
}

fn truncate(input: &str, max: usize) -> String {
    let mut out = String::with_capacity(max + 1);
    for ch in input.chars().take(max) {
        if ch == '\n' || ch == '\r' {
            out.push(' ');
        } else {
            out.push(ch);
        }
    }
    if input.chars().count() > max {
        out.push('…');
    }
    out
}

fn derive_status(updated_at: Option<&str>, now: OffsetDateTime) -> SessionStatus {
    let Some(updated_at) = updated_at else {
        return SessionStatus::Unknown;
    };
    let Ok(ts) = OffsetDateTime::parse(updated_at, &Rfc3339) else {
        return SessionStatus::Unknown;
    };
    let elapsed = now - ts;
    if elapsed.whole_seconds() <= STALE_AFTER_SECS {
        SessionStatus::Active
    } else {
        SessionStatus::Idle
    }
}

fn serialize_source(source: &SessionSource) -> String {
    serde_json::to_value(source)
        .ok()
        .and_then(|v| match v {
            serde_json::Value::String(s) => Some(s),
            serde_json::Value::Object(map) => map.keys().next().cloned(),
            _ => None,
        })
        .unwrap_or_else(|| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::Duration;

    fn write_rollout(dir: &Path, ts: &str, uuid: &str, body: &str) -> PathBuf {
        let filename = format!("rollout-{ts}-{uuid}.jsonl");
        let path = dir.join(filename);
        fs::write(&path, body).expect("write rollout");
        path
    }

    fn rollout_lines() -> String {
        let meta = serde_json::json!({
            "timestamp": "2026-05-21T12:34:56Z",
            "type": "session_meta",
            "payload": {
                "id": "11111111-1111-1111-1111-111111111111",
                "timestamp": "2026-05-21T12:34:56Z",
                "instructions": null,
                "cwd": "/tmp/proj",
                "originator": "codex_cli_rs",
                "cli_version": "0.131.5",
                "source": "cli"
            }
        });
        let user = serde_json::json!({
            "timestamp": "2026-05-21T12:34:57Z",
            "type": "event_msg",
            "payload": {
                "type": "user_message",
                "message": "investigate the auth bug"
            }
        });
        format!("{meta}\n{user}\n")
    }

    #[tokio::test]
    async fn scans_sessions_and_extracts_titles() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let day = tmp.path().join("2026").join("05").join("21");
        fs::create_dir_all(&day).expect("mkdir");
        let path = write_rollout(
            &day,
            "2026-05-21T12-34-56",
            "11111111-1111-1111-1111-111111111111",
            &rollout_lines(),
        );

        let summaries = list_sessions_in_root(tmp.path().to_path_buf(), 50)
            .await
            .expect("scan");
        assert_eq!(summaries.len(), 1, "summaries: {summaries:?}");
        let s = &summaries[0];
        assert_eq!(s.path, path);
        assert_eq!(s.cwd.as_deref(), Some(Path::new("/tmp/proj")));
        assert_eq!(s.title, "investigate the auth bug");
        assert_eq!(s.source.as_deref(), Some("cli"));
        assert!(s.updated_at.is_some());
    }

    #[tokio::test]
    async fn empty_sessions_root_returns_empty_list() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let result = list_sessions_in_root(tmp.path().to_path_buf(), 10)
            .await
            .expect("scan empty");
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn missing_root_returns_empty_list() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let result = list_sessions_in_root(tmp.path().join("nope"), 10)
            .await
            .expect("scan missing");
        assert!(result.is_empty());
    }

    #[test]
    fn idle_status_when_updated_long_ago() {
        let now = OffsetDateTime::now_utc();
        let stale = now - Duration::from_secs(STALE_AFTER_SECS as u64 + 60);
        let stale_str = stale.format(&Rfc3339).unwrap();
        assert_eq!(derive_status(Some(&stale_str), now), SessionStatus::Idle);
    }

    #[test]
    fn active_status_when_updated_recently() {
        let now = OffsetDateTime::now_utc();
        let fresh = now - Duration::from_secs(10);
        let fresh_str = fresh.format(&Rfc3339).unwrap();
        assert_eq!(derive_status(Some(&fresh_str), now), SessionStatus::Active);
    }

    #[test]
    fn unknown_status_when_timestamp_missing_or_bad() {
        let now = OffsetDateTime::now_utc();
        assert_eq!(derive_status(None, now), SessionStatus::Unknown);
        assert_eq!(
            derive_status(Some("not-a-date"), now),
            SessionStatus::Unknown
        );
    }
}
