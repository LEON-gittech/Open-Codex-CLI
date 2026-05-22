//! Full-screen "agent view" overlay listing past Codex sessions discovered on
//! disk. Modelled after [`BackgroundTasksView`] but specialised for top-level
//! session rollouts loaded via `codex_agent_view::list_sessions`.
//!
//! Interaction:
//! - `↑`/`↓` (and `k`/`j`) move the selection.
//! - `Enter` (or `→`) emits `AppEvent::ResumeThreadFromBrowser` for the
//!   selected session so the outer loop can re-launch into resume mode.
//! - `Esc`, `Left`, or `q` close the view.

use codex_agent_view::SessionStatus;
use codex_agent_view::SessionSummary;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use time::OffsetDateTime;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::bottom_pane_view::BottomPaneView;
use crate::bottom_pane::bottom_pane_view::ViewCompletion;
use crate::render::renderable::Renderable;

use super::CancellationEvent;
use super::selection_popup_common;

pub(crate) const SESSION_BROWSER_VIEW_ID: &str = "session_browser";
const MENU_SURFACE_HORIZONTAL_PADDING: u16 = 4;

#[derive(Debug)]
pub(crate) struct SessionBrowserView {
    state: BrowserState,
    selected_index: usize,
    app_event_tx: AppEventSender,
    completion: Option<ViewCompletion>,
    status_message: Option<String>,
}

#[derive(Debug)]
enum BrowserState {
    Loading,
    Loaded(Vec<SessionSummary>),
    Failed(String),
}

impl SessionBrowserView {
    pub(crate) fn new_loading(app_event_tx: AppEventSender) -> Self {
        Self {
            state: BrowserState::Loading,
            selected_index: 0,
            app_event_tx,
            completion: None,
            status_message: None,
        }
    }

    pub(crate) fn set_sessions(&mut self, sessions: Vec<SessionSummary>) {
        self.state = BrowserState::Loaded(sessions);
        if let BrowserState::Loaded(list) = &self.state
            && self.selected_index >= list.len()
        {
            self.selected_index = list.len().saturating_sub(1);
        }
    }

    pub(crate) fn set_error(&mut self, message: String) {
        self.state = BrowserState::Failed(message);
    }

    fn sessions(&self) -> Option<&[SessionSummary]> {
        match &self.state {
            BrowserState::Loaded(list) => Some(list),
            _ => None,
        }
    }

    fn move_selection(&mut self, delta: isize) {
        let Some(list) = self.sessions() else {
            return;
        };
        let count = list.len();
        if count == 0 {
            self.selected_index = 0;
            return;
        }
        let next = (self.selected_index as isize + delta).rem_euclid(count as isize);
        self.selected_index = next as usize;
    }

    fn activate_selected(&mut self) {
        let Some(list) = self.sessions() else {
            return;
        };
        let Some(summary) = list.get(self.selected_index) else {
            return;
        };
        let Some(thread_id) = summary.thread_id.as_deref() else {
            self.status_message = Some("selected session has no thread id".to_string());
            return;
        };
        self.app_event_tx
            .send(AppEvent::ResumeThreadFromBrowser {
                thread_id: thread_id.to_string(),
            });
        self.completion = Some(ViewCompletion::Accepted);
    }

    fn render_lines(&self, width: u16) -> Vec<Line<'static>> {
        let mut lines: Vec<Line<'static>> = Vec::new();
        lines.push(header_line(self.sessions().map(<[SessionSummary]>::len)));
        lines.push("".into());

        match &self.state {
            BrowserState::Loading => {
                lines.push("Loading sessions from $CODEX_HOME/sessions/…".italic().into());
            }
            BrowserState::Failed(err) => {
                lines.push(Line::from(vec![
                    "Failed to load sessions: ".red().bold(),
                    err.clone().into(),
                ]));
            }
            BrowserState::Loaded(list) if list.is_empty() => {
                lines.push(
                    "No sessions found. Start one with `codex` to populate this view."
                        .italic()
                        .into(),
                );
            }
            BrowserState::Loaded(list) => {
                let now = OffsetDateTime::now_utc();
                let mut current_status: Option<SessionStatus> = None;
                for (idx, summary) in list.iter().enumerate() {
                    if current_status != Some(summary.status) {
                        if current_status.is_some() {
                            lines.push("".into());
                        }
                        lines.push(section_header(summary.status, count_with_status(list, summary.status)));
                        current_status = Some(summary.status);
                    }
                    lines.push(format_row(summary, idx == self.selected_index, now, width));
                }
            }
        }

        lines.push("".into());
        if let Some(message) = &self.status_message {
            lines.push(Line::from(vec![message.clone().yellow()]));
        }
        let footer = match &self.state {
            BrowserState::Loaded(list) if !list.is_empty() => {
                "↑/↓ select · Enter/→ resume · Esc/← close"
            }
            _ => "Esc/← close",
        };
        lines.push(footer.dim().into());
        lines
    }
}

impl BottomPaneView for SessionBrowserView {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        if !matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
            return;
        }

        match key_event.code {
            KeyCode::Up | KeyCode::Char('k') => self.move_selection(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_selection(1),
            KeyCode::Enter | KeyCode::Right | KeyCode::Char(' ') => self.activate_selected(),
            KeyCode::Esc | KeyCode::Left | KeyCode::Char('q') => {
                self.completion = Some(ViewCompletion::Cancelled);
            }
            _ => {}
        }
    }

    fn is_complete(&self) -> bool {
        self.completion.is_some()
    }

    fn completion(&self) -> Option<ViewCompletion> {
        self.completion
    }

    fn view_id(&self) -> Option<&'static str> {
        Some(SESSION_BROWSER_VIEW_ID)
    }

    fn selected_index(&self) -> Option<usize> {
        Some(self.selected_index)
    }

    fn update_session_browser_sessions(&mut self, sessions: Vec<SessionSummary>) -> bool {
        self.set_sessions(sessions);
        true
    }

    fn update_session_browser_error(&mut self, message: String) -> bool {
        self.set_error(message);
        true
    }

    fn on_ctrl_c(&mut self) -> CancellationEvent {
        self.completion = Some(ViewCompletion::Cancelled);
        CancellationEvent::Handled
    }

    fn prefer_esc_to_handle_key_event(&self) -> bool {
        true
    }
}

impl Renderable for SessionBrowserView {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let content_area = selection_popup_common::render_menu_surface(area, buf);
        Paragraph::new(self.render_lines(content_area.width)).render(content_area, buf);
    }

    fn desired_height(&self, width: u16) -> u16 {
        self.render_lines(width.saturating_sub(MENU_SURFACE_HORIZONTAL_PADDING))
            .len()
            .try_into()
            .unwrap_or(u16::MAX)
            .saturating_add(selection_popup_common::menu_surface_padding_height())
    }
}

fn header_line(count: Option<usize>) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = vec!["Agent view · past sessions".bold()];
    if let Some(n) = count {
        spans.push("  ".into());
        spans.push(format!("({n})").dim());
    }
    Line::from(spans)
}

fn section_header(status: SessionStatus, count: usize) -> Line<'static> {
    let (label, color) = match status {
        SessionStatus::Active => ("Active", ratatui::style::Color::Green),
        SessionStatus::Idle => ("Idle", ratatui::style::Color::Yellow),
        SessionStatus::Unknown => ("Unknown", ratatui::style::Color::DarkGray),
    };
    Line::from(vec![format!("{label} ({count})").fg(color).bold()])
}

fn count_with_status(list: &[SessionSummary], status: SessionStatus) -> usize {
    list.iter().filter(|s| s.status == status).count()
}

fn format_row(
    summary: &SessionSummary,
    selected: bool,
    now: OffsetDateTime,
    width: u16,
) -> Line<'static> {
    let caret = if selected { "› ".cyan() } else { "  ".into() };
    let title_span = if selected {
        summary.title.clone().bold()
    } else {
        summary.title.clone().into()
    };
    let elapsed = elapsed_label(summary.updated_at_offset(), now);
    let cwd = summary
        .cwd
        .as_ref()
        .map(|p| compact_path(p, 32))
        .unwrap_or_else(|| "(no cwd)".to_string());
    let mut tail_parts = vec![cwd];
    if let Some(role) = &summary.agent_role {
        tail_parts.push(format!("[{role}]"));
    }
    if let Some(branch) = &summary.git_branch {
        tail_parts.push(format!("⎇ {branch}"));
    }
    tail_parts.push(elapsed);
    let tail = tail_parts.join(" · ");

    let used_caret = 2usize;
    let used_title = summary.title.chars().count();
    let available = (width as usize).saturating_sub(used_caret + used_title + 4);
    let tail_text = if tail.chars().count() > available {
        truncate_to_chars(&tail, available)
    } else {
        let pad = available.saturating_sub(tail.chars().count());
        format!("{}{tail}", " ".repeat(pad))
    };
    Line::from(vec![caret, title_span, "  ".into(), tail_text.dim()])
}

fn elapsed_label(ts: Option<OffsetDateTime>, now: OffsetDateTime) -> String {
    let Some(ts) = ts else {
        return "—".to_string();
    };
    let elapsed = now - ts;
    let secs = elapsed.whole_seconds();
    if secs < 0 {
        return "just now".to_string();
    }
    if secs < 60 {
        return format!("{secs}s ago");
    }
    let mins = secs / 60;
    if mins < 60 {
        return format!("{mins}m ago");
    }
    let hours = mins / 60;
    if hours < 24 {
        return format!("{hours}h ago");
    }
    let days = hours / 24;
    format!("{days}d ago")
}

fn compact_path(path: &std::path::Path, max: usize) -> String {
    let display = path.display().to_string();
    if display.chars().count() <= max {
        return display;
    }
    let suffix: String = display.chars().rev().take(max - 1).collect();
    let suffix: String = suffix.chars().rev().collect();
    format!("…{suffix}")
}

fn truncate_to_chars(text: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    let mut out = String::new();
    let limit = max.saturating_sub(1);
    let mut taken = 0usize;
    for ch in text.chars() {
        if taken >= limit {
            out.push('…');
            return out;
        }
        out.push(ch);
        taken += 1;
    }
    out
}
