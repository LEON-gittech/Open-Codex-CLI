//! Full-screen "agent view" overlay listing past Codex sessions discovered on
//! disk. Modelled after Claude Code's `claude agents` screen — it takes over
//! the entire frame, groups sessions by recency, and supports peek/resume/
//! delete from the same surface.
//!
//! Interaction:
//! - `↑`/`↓` (and `k`/`j`) move the selection. Page Up/Down jumps a screen.
//! - `Space` opens an inline peek of the selected rollout.
//! - `Enter` / `→` emits `AppEvent::ResumeThreadFromBrowser` so the outer loop
//!   re-launches into resume mode for that thread.
//! - `Ctrl+X` deletes the selected rollout file (double-press confirms).
//! - `Esc`, `Left`, or `q` close the view.

use std::path::PathBuf;
use std::time::Instant;

use codex_agent_view::PeekContent;
use codex_agent_view::PeekLine;
use codex_agent_view::SessionStatus;
use codex_agent_view::SessionSummary;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Constraint;
use ratatui::layout::Direction;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Clear;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use ratatui::widgets::Wrap;
use time::OffsetDateTime;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::bottom_pane_view::BottomPaneView;
use crate::bottom_pane::bottom_pane_view::ViewCompletion;
use crate::render::renderable::Renderable;

use super::CancellationEvent;

pub(crate) const SESSION_BROWSER_VIEW_ID: &str = "session_browser";
const DELETE_CONFIRM_WINDOW: std::time::Duration = std::time::Duration::from_secs(2);
const HEADER_HEIGHT: u16 = 3;
const FOOTER_HEIGHT: u16 = 2;
const COMPOSER_HEIGHT: u16 = 3;
const PEEK_DEFAULT_LIMIT: usize = 80;

#[derive(Debug)]
pub(crate) struct SessionBrowserView {
    state: BrowserState,
    selected_index: usize,
    view_offset: usize,
    last_visible_height: std::cell::Cell<usize>,
    last_screen_size: std::cell::Cell<(u16, u16)>,
    app_event_tx: AppEventSender,
    completion: Option<ViewCompletion>,
    status_message: Option<(String, Instant)>,
    pending_delete: Option<(usize, Instant)>,
    peek: PeekState,
    /// Free-form text the user has typed into the inline composer at the
    /// bottom of the agent view. When non-empty, Enter sends it to the
    /// selected (resume) or fresh session.
    compose: String,
}

#[derive(Debug)]
enum BrowserState {
    Loading,
    Loaded(Vec<SessionSummary>),
    Failed(String),
}

#[derive(Debug)]
enum PeekState {
    Closed,
    Loading { path: PathBuf },
    Ready(PeekContent),
    Failed(String),
}

impl SessionBrowserView {
    pub(crate) fn new_loading(app_event_tx: AppEventSender) -> Self {
        Self {
            state: BrowserState::Loading,
            selected_index: 0,
            view_offset: 0,
            last_visible_height: std::cell::Cell::new(0),
            last_screen_size: std::cell::Cell::new((0, 0)),
            app_event_tx,
            completion: None,
            status_message: None,
            pending_delete: None,
            peek: PeekState::Closed,
            compose: String::new(),
        }
    }

    pub(crate) fn set_sessions(&mut self, sessions: Vec<SessionSummary>) {
        self.state = BrowserState::Loaded(sessions);
        if let BrowserState::Loaded(list) = &self.state
            && self.selected_index >= list.len()
        {
            self.selected_index = list.len().saturating_sub(1);
        }
        self.ensure_visible();
    }

    pub(crate) fn set_error(&mut self, message: String) {
        self.state = BrowserState::Failed(message);
    }

    pub(crate) fn set_peek_loading(&mut self, path: PathBuf) {
        self.peek = PeekState::Loading { path };
    }

    pub(crate) fn set_peek(&mut self, content: PeekContent) {
        self.peek = PeekState::Ready(content);
    }

    pub(crate) fn set_peek_error(&mut self, message: String) {
        self.peek = PeekState::Failed(message);
    }

    fn sessions(&self) -> Option<&[SessionSummary]> {
        match &self.state {
            BrowserState::Loaded(list) => Some(list),
            _ => None,
        }
    }

    fn selected_summary(&self) -> Option<&SessionSummary> {
        self.sessions().and_then(|list| list.get(self.selected_index))
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
        self.pending_delete = None;
        self.ensure_visible();
    }

    fn ensure_visible(&mut self) {
        let visible = self.last_visible_height.get().max(1);
        if self.selected_index < self.view_offset {
            self.view_offset = self.selected_index;
        } else if self.selected_index >= self.view_offset + visible {
            self.view_offset = self.selected_index + 1 - visible;
        }
    }

    fn activate_selected(&mut self) {
        let thread_id_opt = self
            .selected_summary()
            .and_then(|s| s.thread_id.clone());
        let Some(thread_id) = thread_id_opt else {
            self.set_status("Selected session has no thread id");
            return;
        };
        let initial_prompt = self.take_compose_text();
        self.app_event_tx
            .send(AppEvent::ResumeThreadFromBrowser {
                thread_id,
                initial_prompt,
            });
        self.completion = Some(ViewCompletion::Accepted);
    }

    fn start_new_session(&mut self) {
        let initial_prompt = self.take_compose_text();
        self.app_event_tx
            .send(AppEvent::StartNewSessionFromBrowser { initial_prompt });
        self.completion = Some(ViewCompletion::Accepted);
    }

    fn take_compose_text(&mut self) -> Option<String> {
        let trimmed = self.compose.trim().to_string();
        self.compose.clear();
        if trimmed.is_empty() { None } else { Some(trimmed) }
    }

    fn open_peek(&mut self) {
        let Some(summary) = self.selected_summary() else {
            return;
        };
        let path = summary.path.clone();
        let tx = self.app_event_tx.clone();
        self.set_peek_loading(path.clone());
        tokio::spawn(async move {
            match codex_agent_view::load_peek(&path, PEEK_DEFAULT_LIMIT).await {
                Ok(content) => tx.send(AppEvent::SessionBrowserPeekLoaded(Box::new(content))),
                Err(err) => tx.send(AppEvent::SessionBrowserPeekFailed(format!("{err:#}"))),
            }
        });
    }

    fn close_peek(&mut self) {
        self.peek = PeekState::Closed;
    }

    fn request_delete(&mut self) {
        let Some(summary) = self.selected_summary() else {
            return;
        };
        let path = summary.path.clone();
        let now = Instant::now();
        match self.pending_delete {
            Some((idx, when))
                if idx == self.selected_index && now.duration_since(when) <= DELETE_CONFIRM_WINDOW =>
            {
                let tx = self.app_event_tx.clone();
                tokio::spawn(async move {
                    match tokio::fs::remove_file(&path).await {
                        Ok(()) => tx.send(AppEvent::SessionBrowserDeleted {
                            path: path.clone(),
                            error: None,
                        }),
                        Err(err) => tx.send(AppEvent::SessionBrowserDeleted {
                            path: path.clone(),
                            error: Some(format!("{err:#}")),
                        }),
                    }
                });
                self.pending_delete = None;
                self.set_status("Deleting…");
            }
            _ => {
                self.pending_delete = Some((self.selected_index, now));
                self.set_status("Press Ctrl+X again within 2s to delete this session.");
            }
        }
    }

    pub(crate) fn after_delete(&mut self, path: &std::path::Path, error: Option<String>) {
        if let Some(err) = error {
            self.set_status(&format!("Delete failed: {err}"));
            return;
        }
        let new_selected = if let BrowserState::Loaded(list) = &mut self.state {
            list.retain(|s| s.path != path);
            self.selected_index.min(list.len().saturating_sub(1))
        } else {
            0
        };
        self.selected_index = new_selected;
        self.set_status("Session deleted.");
        self.ensure_visible();
    }

    fn set_status(&mut self, message: impl Into<String>) {
        self.status_message = Some((message.into(), Instant::now()));
    }

    fn status_text(&self) -> Option<String> {
        self.status_message
            .as_ref()
            .filter(|(_, when)| when.elapsed() < std::time::Duration::from_secs(5))
            .map(|(text, _)| text.clone())
    }

    fn render_header(&self, area: Rect, buf: &mut Buffer) {
        let counts = self.sessions().map(group_counts).unwrap_or_default();
        let total = self.sessions().map(<[SessionSummary]>::len).unwrap_or(0);
        let title = Line::from(vec![
            Span::styled(
                "Open Codex · agent view",
                Style::default().add_modifier(Modifier::BOLD).fg(Color::Cyan),
            ),
            Span::raw("  "),
            Span::styled(format!("({total} sessions)"), Style::default().fg(Color::DarkGray)),
        ]);
        let mut summary_parts: Vec<Span<'static>> = Vec::new();
        for (label, count, color) in [
            ("active", counts.active, Color::Green),
            ("idle", counts.idle, Color::Yellow),
            ("unknown", counts.unknown, Color::DarkGray),
        ] {
            if count == 0 {
                continue;
            }
            if !summary_parts.is_empty() {
                summary_parts.push(Span::raw(" · "));
            }
            summary_parts.push(Span::styled(
                format!("{count} {label}"),
                Style::default().fg(color),
            ));
        }
        let summary = Line::from(summary_parts);
        Paragraph::new(vec![title, summary, Line::raw("")])
            .render(area, buf);
    }

    fn render_footer(&self, area: Rect, buf: &mut Buffer) {
        let hint = match (&self.state, &self.peek) {
            (_, PeekState::Closed) if !self.compose.is_empty() => {
                "Enter send · Ctrl+N new session · Backspace delete · Esc clear"
            }
            (BrowserState::Loaded(list), PeekState::Closed) if !list.is_empty() => {
                "↑/↓ select · Space peek · Enter/→ resume · type to send · Ctrl+N new · Ctrl+X delete · Esc/← close"
            }
            (_, PeekState::Closed) => "Esc/← close",
            _ => "Esc/Space close peek",
        };
        let mut lines = vec![
            Line::raw(""),
            Line::from(Span::styled(hint, Style::default().fg(Color::DarkGray))),
        ];
        if let Some(msg) = self.status_text() {
            lines.push(Line::from(Span::styled(msg, Style::default().fg(Color::Yellow))));
        }
        let para = Paragraph::new(lines).wrap(Wrap { trim: true });
        para.render(area, buf);
    }

    fn render_composer(&self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }
        let selected_label = self
            .selected_summary()
            .map(|s| {
                let title = s.title.chars().take(40).collect::<String>();
                if s.title.chars().count() > 40 {
                    format!("{title}…")
                } else {
                    title
                }
            })
            .unwrap_or_else(|| "no session selected".to_string());
        let title = format!(" Reply to: {selected_label} ");
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(if self.compose.is_empty() {
                Color::DarkGray
            } else {
                Color::Cyan
            }))
            .title(Span::styled(
                title,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ));
        let inner = block.inner(area);
        block.render(area, buf);
        let body = if self.compose.is_empty() {
            Line::from(Span::styled(
                "Type a prompt and press Enter to resume with that prompt (or Ctrl+N for a fresh session).",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            ))
        } else {
            Line::from(vec![
                Span::styled("› ", Style::default().fg(Color::Cyan)),
                Span::raw(self.compose.clone()),
                Span::styled("▌", Style::default().fg(Color::Cyan)),
            ])
        };
        Paragraph::new(body).render(inner, buf);
    }

    fn render_list(&self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }
        match &self.state {
            BrowserState::Loading => {
                let p = Paragraph::new(Line::from(Span::styled(
                    "Loading sessions from $CODEX_HOME/sessions/…",
                    Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
                )));
                p.render(area, buf);
                return;
            }
            BrowserState::Failed(err) => {
                let p = Paragraph::new(Line::from(vec![
                    "Failed to load sessions: ".red().bold(),
                    err.clone().into(),
                ]))
                .wrap(Wrap { trim: true });
                p.render(area, buf);
                return;
            }
            BrowserState::Loaded(list) if list.is_empty() => {
                let p = Paragraph::new(Line::from(Span::styled(
                    "No sessions found. Start one with `codex` to populate this view.",
                    Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
                )))
                .wrap(Wrap { trim: true });
                p.render(area, buf);
                return;
            }
            BrowserState::Loaded(_) => {}
        }

        let Some(list) = self.sessions() else {
            return;
        };
        let visible_height = area.height as usize;
        self.last_visible_height.set(visible_height);
        let end = (self.view_offset + visible_height).min(list.len());
        let window = &list[self.view_offset..end];
        let now = OffsetDateTime::now_utc();

        let mut lines: Vec<Line<'static>> = Vec::with_capacity(window.len());
        let mut current_status: Option<SessionStatus> = None;
        for (visible_idx, summary) in window.iter().enumerate() {
            let absolute_idx = self.view_offset + visible_idx;
            if current_status != Some(summary.status) {
                lines.push(section_header(summary.status, count_with_status(list, summary.status)));
                current_status = Some(summary.status);
            }
            let pending_delete = self
                .pending_delete
                .is_some_and(|(idx, _)| idx == absolute_idx);
            lines.push(format_row(
                summary,
                absolute_idx == self.selected_index,
                pending_delete,
                now,
                area.width,
                absolute_idx + 1,
            ));
        }

        Paragraph::new(lines).render(area, buf);
    }

    fn render_peek(&self, area: Rect, buf: &mut Buffer) {
        let inset = 4u16;
        let width = area.width.saturating_sub(inset * 2).clamp(40, 140);
        let height = area.height.saturating_sub(inset).max(8);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let overlay = Rect::new(x, y, width, height);
        Clear.render(overlay, buf);

        let title_text = match &self.peek {
            PeekState::Closed => return,
            PeekState::Loading { path } => format!("Loading peek — {}", path.display()),
            PeekState::Ready(content) => {
                let trunc = if content.truncated { " (head truncated)" } else { "" };
                format!("Peek — {}{trunc}", content.path.display())
            }
            PeekState::Failed(err) => format!("Peek failed: {err}"),
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title(Span::styled(
                title_text,
                Style::default().add_modifier(Modifier::BOLD),
            ));
        let inner = block.inner(overlay);
        block.render(overlay, buf);

        let lines: Vec<Line<'static>> = match &self.peek {
            PeekState::Closed => return,
            PeekState::Loading { .. } => vec![Line::from(Span::styled(
                "Loading…",
                Style::default().fg(Color::DarkGray),
            ))],
            PeekState::Failed(_) => vec![Line::from(Span::styled(
                "Press Esc or Space to close.",
                Style::default().fg(Color::DarkGray),
            ))],
            PeekState::Ready(content) => peek_lines(&content.rendered),
        };
        let scroll_y = if let PeekState::Ready(content) = &self.peek {
            let visible = inner.height as usize;
            content.rendered.len().saturating_sub(visible).min(u16::MAX as usize) as u16
        } else {
            0
        };
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .scroll((scroll_y, 0))
            .render(inner, buf);
    }
}

impl BottomPaneView for SessionBrowserView {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        if !matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
            return;
        }

        // If a peek is open, route keys to it first.
        if matches!(
            self.peek,
            PeekState::Loading { .. } | PeekState::Ready(_) | PeekState::Failed(_)
        ) {
            match key_event.code {
                KeyCode::Esc | KeyCode::Char(' ') | KeyCode::Char('q') => self.close_peek(),
                KeyCode::Enter => self.close_peek(),
                _ => {}
            }
            return;
        }

        // Ctrl+X delete (with double-press confirm).
        if key_event.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key_event.code, KeyCode::Char('x') | KeyCode::Char('X'))
        {
            self.request_delete();
            return;
        }

        // Ctrl+N start fresh session (carries any composer text as prompt).
        if key_event.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key_event.code, KeyCode::Char('n') | KeyCode::Char('N'))
        {
            self.start_new_session();
            return;
        }

        let composing = !self.compose.is_empty();

        match key_event.code {
            // Navigation keys: always honoured even while composing — agent
            // view should let users keep moving through the list while a draft
            // is in flight.
            KeyCode::Up => self.move_selection(-1),
            KeyCode::Down => self.move_selection(1),
            KeyCode::PageUp => {
                let step = self.last_visible_height.get().max(1) as isize;
                self.move_selection(-step);
            }
            KeyCode::PageDown => {
                let step = self.last_visible_height.get().max(1) as isize;
                self.move_selection(step);
            }
            KeyCode::Home => {
                self.selected_index = 0;
                self.pending_delete = None;
                self.ensure_visible();
            }
            KeyCode::End => {
                if let Some(list) = self.sessions() {
                    self.selected_index = list.len().saturating_sub(1);
                }
                self.pending_delete = None;
                self.ensure_visible();
            }
            // Vim-style nav: only when not composing (so `j`, `k`, `g`, `G`,
            // `q` don't fight the composer).
            KeyCode::Char('k') if !composing => self.move_selection(-1),
            KeyCode::Char('j') if !composing => self.move_selection(1),
            KeyCode::Char('g') if !composing => {
                self.selected_index = 0;
                self.pending_delete = None;
                self.ensure_visible();
            }
            KeyCode::Char('G') if !composing => {
                if let Some(list) = self.sessions() {
                    self.selected_index = list.len().saturating_sub(1);
                }
                self.pending_delete = None;
                self.ensure_visible();
            }
            KeyCode::Char('q') if !composing => {
                self.completion = Some(ViewCompletion::Cancelled);
            }
            // Space: peek when not composing, otherwise insert a literal space.
            KeyCode::Char(' ') if !composing => self.open_peek(),
            // Enter and Right always activate the selection (with the composer
            // text becoming the initial prompt of the resumed session).
            KeyCode::Enter | KeyCode::Right => self.activate_selected(),
            // Esc clears the draft first; a second Esc closes the view.
            KeyCode::Esc => {
                if composing {
                    self.compose.clear();
                } else {
                    self.completion = Some(ViewCompletion::Cancelled);
                }
            }
            // Left closes only when no draft is in flight (otherwise reserved
            // for future cursor movement).
            KeyCode::Left if !composing => {
                self.completion = Some(ViewCompletion::Cancelled);
            }
            // Composer editing.
            KeyCode::Backspace => {
                self.compose.pop();
            }
            KeyCode::Char(ch) if !key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                self.compose.push(ch);
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

    fn set_session_browser_peek(&mut self, content: PeekContent) -> bool {
        self.set_peek(content);
        true
    }

    fn set_session_browser_peek_error(&mut self, message: String) -> bool {
        self.set_peek_error(message);
        true
    }

    fn after_session_browser_delete(
        &mut self,
        path: &std::path::Path,
        error: Option<String>,
    ) -> bool {
        self.after_delete(path, error);
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
        if area.is_empty() {
            return;
        }
        self.last_screen_size.set((area.width, area.height));

        // Paint a clean background.
        Clear.render(area, buf);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(HEADER_HEIGHT),
                Constraint::Min(1),
                Constraint::Length(COMPOSER_HEIGHT),
                Constraint::Length(FOOTER_HEIGHT + status_extra_height(self.status_text())),
            ])
            .split(area);

        self.render_header(chunks[0], buf);
        self.render_list(chunks[1], buf);
        self.render_composer(chunks[2], buf);
        self.render_footer(chunks[3], buf);

        if matches!(
            self.peek,
            PeekState::Loading { .. } | PeekState::Ready(_) | PeekState::Failed(_)
        ) {
            self.render_peek(area, buf);
        }
    }

    fn desired_height(&self, _width: u16) -> u16 {
        // Always claim the entire available height so the bottom pane gives us
        // the full frame (Claude Code-style full-screen agent view).
        u16::MAX
    }
}

#[derive(Default)]
struct StatusCounts {
    active: usize,
    idle: usize,
    unknown: usize,
}

fn group_counts(sessions: &[SessionSummary]) -> StatusCounts {
    let mut counts = StatusCounts::default();
    for s in sessions {
        match s.status {
            SessionStatus::Active => counts.active += 1,
            SessionStatus::Idle => counts.idle += 1,
            SessionStatus::Unknown => counts.unknown += 1,
        }
    }
    counts
}

fn section_header(status: SessionStatus, count: usize) -> Line<'static> {
    let (label, color) = match status {
        SessionStatus::Active => ("Active", Color::Green),
        SessionStatus::Idle => ("Idle", Color::Yellow),
        SessionStatus::Unknown => ("Unknown", Color::DarkGray),
    };
    Line::from(vec![Span::styled(
        format!("{label} ({count})"),
        Style::default().fg(color).add_modifier(Modifier::BOLD),
    )])
}

fn count_with_status(list: &[SessionSummary], status: SessionStatus) -> usize {
    list.iter().filter(|s| s.status == status).count()
}

fn format_row(
    summary: &SessionSummary,
    selected: bool,
    pending_delete: bool,
    now: OffsetDateTime,
    width: u16,
    ordinal: usize,
) -> Line<'static> {
    let caret = if selected {
        Span::styled("›", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
    } else {
        Span::raw(" ")
    };
    let ord_span = Span::styled(
        format!(" {ordinal:>3}. "),
        Style::default().fg(Color::DarkGray),
    );
    let title_style = if selected {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    let title_span = Span::styled(summary.title.clone(), title_style);
    let elapsed = elapsed_label(summary.updated_at_offset(), now);
    let cwd = summary
        .cwd
        .as_ref()
        .map(|p| compact_path(p, 36))
        .unwrap_or_else(|| "(no cwd)".to_string());
    let mut tail_parts = vec![cwd];
    if let Some(role) = &summary.agent_role {
        tail_parts.push(format!("[{role}]"));
    }
    if let Some(branch) = &summary.git_branch {
        tail_parts.push(format!("⎇ {branch}"));
    }
    tail_parts.push(elapsed);
    if pending_delete {
        tail_parts.push("⚠ confirm".to_string());
    }
    let tail = tail_parts.join(" · ");

    let used_caret = 1usize;
    let used_ord = 6usize;
    let used_title = summary.title.chars().count();
    let target = (width as usize).saturating_sub(used_caret + used_ord + 2);
    let space_for_title = target.saturating_sub(tail.chars().count() + 2);
    let (title_text, available) = if used_title > space_for_title {
        (truncate_to_chars(&summary.title, space_for_title), 1)
    } else {
        (summary.title.clone(), target.saturating_sub(used_title))
    };
    let final_title_span = Span::styled(title_text, title_style);
    let tail_pad = available.saturating_sub(tail.chars().count());
    let tail_text = format!("{}{tail}", " ".repeat(tail_pad));
    let tail_style = if pending_delete {
        Style::default().fg(Color::Red)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let _ = title_span;
    Line::from(vec![
        caret,
        ord_span,
        final_title_span,
        Span::raw("  "),
        Span::styled(tail_text, tail_style),
    ])
}

fn peek_lines(rendered: &[PeekLine]) -> Vec<Line<'static>> {
    if rendered.is_empty() {
        return vec![Line::from(Span::styled(
            "(rollout has no recognised items)",
            Style::default().fg(Color::DarkGray),
        ))];
    }
    rendered
        .iter()
        .map(|line| {
            Line::from(vec![
                Span::styled(
                    format!("{:<9}", line.kind),
                    Style::default()
                        .fg(kind_color(&line.kind))
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(line.text.clone()),
            ])
        })
        .collect()
}

fn kind_color(kind: &str) -> Color {
    match kind {
        "user" => Color::Cyan,
        "agent" => Color::Green,
        "exec" => Color::Yellow,
        "edit" => Color::Magenta,
        "reasoning" => Color::DarkGray,
        _ => Color::Blue,
    }
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

fn status_extra_height(status: Option<String>) -> u16 {
    if status.is_some() { 1 } else { 0 }
}
