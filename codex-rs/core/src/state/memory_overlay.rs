use codex_utils_output_truncation::TruncationPolicy;
use codex_utils_output_truncation::truncate_text;

const MAX_ENTRY_TOKENS: usize = 600;
const MAX_OVERLAY_TOKENS: usize = 1_500;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct SessionMemoryOverlay {
    revision: u64,
    entries: Vec<SessionMemoryOverlayEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SessionMemoryOverlayEntry {
    pub(crate) content: String,
    pub(crate) reason: Option<String>,
    pub(crate) created_at_unix_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SessionMemoryOverlaySnapshot {
    pub(crate) revision: u64,
    pub(crate) rendered: Option<String>,
    pub(crate) entries: Vec<SessionMemoryOverlayEntry>,
}

impl SessionMemoryOverlay {
    pub(crate) fn stage(
        &mut self,
        content: String,
        reason: Option<String>,
        created_at_unix_ms: i64,
    ) -> SessionMemoryOverlaySnapshot {
        let content = truncate_text(content.trim(), TruncationPolicy::Tokens(MAX_ENTRY_TOKENS));
        let reason = reason
            .map(|reason| reason.trim().to_string())
            .filter(|reason| !reason.is_empty());
        self.entries.push(SessionMemoryOverlayEntry {
            content,
            reason,
            created_at_unix_ms,
        });
        self.revision = self.revision.saturating_add(1);
        self.snapshot()
    }

    pub(crate) fn snapshot(&self) -> SessionMemoryOverlaySnapshot {
        SessionMemoryOverlaySnapshot {
            revision: self.revision,
            rendered: self.render(),
            entries: self.entries.clone(),
        }
    }

    fn render(&self) -> Option<String> {
        if self.entries.is_empty() {
            return None;
        }
        let mut rendered = String::from(
            "These entries were actively staged during this session. Treat them as current session memory until durable memory consolidation incorporates them.",
        );
        for entry in &self.entries {
            rendered.push_str("\n\n- ");
            rendered.push_str(entry.content.trim());
            if let Some(reason) = &entry.reason {
                rendered.push_str("\n  reason: ");
                rendered.push_str(reason.trim());
            }
        }
        Some(truncate_text(
            &rendered,
            TruncationPolicy::Tokens(MAX_OVERLAY_TOKENS),
        ))
    }
}
