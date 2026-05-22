//! Ratatui screen for the `codex agents` viewer.
//!
//! Two modes:
//! - `list`: full-screen scrolling list of sessions grouped by status.
//! - `peek`: overlay showing the most recent items of the selected rollout.

use std::collections::BTreeMap;
use std::io::IsTerminal as _;
use std::path::PathBuf;
use std::time::Duration;

use crossterm::ExecutableCommand as _;
use crossterm::event::DisableBracketedPaste;
use crossterm::event::EnableBracketedPaste;
use crossterm::event::Event;
use crossterm::event::EventStream;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use crossterm::terminal::EnterAlternateScreen;
use crossterm::terminal::LeaveAlternateScreen;
use crossterm::terminal::disable_raw_mode;
use crossterm::terminal::enable_raw_mode;
use ratatui::Frame;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Constraint;
use ratatui::layout::Direction;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Clear;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Wrap;
use time::OffsetDateTime;
use tokio_stream::StreamExt as _;
use unicode_width::UnicodeWidthStr;

use crate::peek::DEFAULT_PEEK_LIMIT;
use crate::peek::PeekContent;
use crate::peek::PeekLine;
use crate::peek::load_peek;
use crate::scanner::SessionStatus;
use crate::scanner::SessionSummary;
use crate::scanner::list_sessions;

/// Render the agent-view full-screen UI until the user quits.
pub async fn run_tui(sessions: Vec<SessionSummary>, codex_home: PathBuf) -> anyhow::Result<()> {
    if !std::io::stdout().is_terminal() {
        anyhow::bail!("`codex agents` requires a TTY; use --json for non-interactive output");
    }

    let mut stdout = std::io::stdout();
    enable_raw_mode()?;
    stdout.execute(EnterAlternateScreen)?;
    let _ = stdout.execute(EnableBracketedPaste);
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let result = run_event_loop(&mut terminal, sessions, codex_home).await;

    let _ = disable_raw_mode();
    let mut stdout = std::io::stdout();
    let _ = stdout.execute(DisableBracketedPaste);
    let _ = stdout.execute(LeaveAlternateScreen);
    result
}

async fn run_event_loop<B>(
    terminal: &mut Terminal<B>,
    sessions: Vec<SessionSummary>,
    codex_home: PathBuf,
) -> anyhow::Result<()>
where
    B: ratatui::backend::Backend,
{
    let mut app = App::new(sessions, codex_home);
    let mut events = EventStream::new();
    let mut redraw = true;
    let mut tick = tokio::time::interval(Duration::from_secs(1));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        if app.quit {
            return Ok(());
        }
        if redraw {
            terminal.draw(|f| app.render(f))?;
            redraw = false;
        }

        tokio::select! {
            biased;
            event = events.next() => {
                match event {
                    Some(Ok(Event::Key(key))) => {
                        if key.kind == KeyEventKind::Press || key.kind == KeyEventKind::Repeat {
                            app.handle_key(key).await?;
                            redraw = true;
                        }
                    }
                    Some(Ok(Event::Resize(_, _))) => {
                        redraw = true;
                    }
                    Some(Ok(_)) => {}
                    Some(Err(err)) => return Err(err.into()),
                    None => return Ok(()),
                }
            }
            _ = tick.tick() => {
                redraw = true;
            }
        }
    }
}

#[derive(Debug)]
struct App {
    sessions: Vec<SessionSummary>,
    codex_home: PathBuf,
    selected: usize,
    quit: bool,
    status_message: Option<String>,
    peek: PeekState,
}

#[derive(Debug)]
enum PeekState {
    Closed,
    Loading,
    Ready(PeekContent),
    Failed(String),
}

impl App {
    fn new(sessions: Vec<SessionSummary>, codex_home: PathBuf) -> Self {
        Self {
            sessions,
            codex_home,
            selected: 0,
            quit: false,
            status_message: None,
            peek: PeekState::Closed,
        }
    }

    async fn handle_key(&mut self, key: KeyEvent) -> anyhow::Result<()> {
        if matches!(
            self.peek,
            PeekState::Loading | PeekState::Ready(_) | PeekState::Failed(_)
        ) {
            return self.handle_key_peek(key);
        }
        match (key.code, key.modifiers) {
            (KeyCode::Char('q'), _) | (KeyCode::Esc, _) => {
                self.quit = true;
            }
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                self.quit = true;
            }
            (KeyCode::Up, _) | (KeyCode::Char('k'), _) => {
                self.selected = self.selected.saturating_sub(1);
            }
            (KeyCode::Down, _) | (KeyCode::Char('j'), _) => {
                if !self.sessions.is_empty() {
                    self.selected = (self.selected + 1).min(self.sessions.len() - 1);
                }
            }
            (KeyCode::Home, _) | (KeyCode::Char('g'), _) => {
                self.selected = 0;
            }
            (KeyCode::End, _) | (KeyCode::Char('G'), _) => {
                if !self.sessions.is_empty() {
                    self.selected = self.sessions.len() - 1;
                }
            }
            (KeyCode::Char(' '), _) | (KeyCode::Enter, _) => {
                self.open_peek().await;
            }
            (KeyCode::Char('r'), _) => {
                self.refresh().await;
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_key_peek(&mut self, key: KeyEvent) -> anyhow::Result<()> {
        match (key.code, key.modifiers) {
            (KeyCode::Esc, _)
            | (KeyCode::Char('q'), _)
            | (KeyCode::Char(' '), _)
            | (KeyCode::Enter, _)
            | (KeyCode::Left, _) => {
                self.peek = PeekState::Closed;
            }
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                self.quit = true;
            }
            _ => {}
        }
        Ok(())
    }

    async fn open_peek(&mut self) {
        let Some(summary) = self.sessions.get(self.selected) else {
            self.status_message = Some("no session selected".to_string());
            return;
        };
        let path = summary.path.clone();
        self.peek = PeekState::Loading;
        match load_peek(&path, DEFAULT_PEEK_LIMIT).await {
            Ok(content) => self.peek = PeekState::Ready(content),
            Err(err) => self.peek = PeekState::Failed(format!("{err:#}")),
        }
    }

    async fn refresh(&mut self) {
        match list_sessions(&self.codex_home, default_limit()).await {
            Ok(sessions) => {
                self.sessions = sessions;
                if self.selected >= self.sessions.len() {
                    self.selected = self.sessions.len().saturating_sub(1);
                }
                self.status_message = Some(format!("refreshed: {} sessions", self.sessions.len()));
            }
            Err(err) => {
                self.status_message = Some(format!("refresh failed: {err:#}"));
            }
        }
    }

    fn render(&self, f: &mut Frame<'_>) {
        let area = f.area();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2),
                Constraint::Min(1),
                Constraint::Length(1),
            ])
            .split(area);

        self.render_header(f, chunks[0]);
        self.render_list(f, chunks[1]);
        self.render_footer(f, chunks[2]);

        if let PeekState::Loading | PeekState::Ready(_) | PeekState::Failed(_) = &self.peek {
            self.render_peek(f, area);
        }
    }

    fn render_header(&self, f: &mut Frame<'_>, area: Rect) {
        let counts = group_counts(&self.sessions);
        let title = Line::from(vec![
            Span::styled(
                "Open Codex · agent view",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                format!("{} sessions", self.sessions.len()),
                Style::default().fg(Color::DarkGray),
            ),
        ]);
        let mut summary_parts: Vec<Span<'_>> = Vec::new();
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
        let paragraph = Paragraph::new(vec![title, summary]);
        f.render_widget(paragraph, area);
    }

    fn render_list(&self, f: &mut Frame<'_>, area: Rect) {
        if self.sessions.is_empty() {
            let empty = Paragraph::new(Line::from(vec![Span::styled(
                "No sessions found under $CODEX_HOME/sessions/. Start one with `codex` first.",
                Style::default().fg(Color::DarkGray),
            )]))
            .wrap(Wrap { trim: true });
            f.render_widget(empty, area);
            return;
        }

        let mut lines: Vec<Line<'_>> = Vec::with_capacity(self.sessions.len() + 8);
        let groups = group_sessions(&self.sessions);
        let now = OffsetDateTime::now_utc();
        for (status, indices) in groups {
            lines.push(Line::from(vec![Span::styled(
                format!("{} ({})", status_section_title(status), indices.len()),
                Style::default()
                    .fg(status_color(status))
                    .add_modifier(Modifier::BOLD),
            )]));
            for idx in indices {
                let summary = &self.sessions[idx];
                let selected = idx == self.selected;
                lines.push(format_row(summary, selected, now, area.width));
            }
            lines.push(Line::raw(""));
        }
        let para = Paragraph::new(lines).wrap(Wrap { trim: false });
        f.render_widget(para, area);
    }

    fn render_footer(&self, f: &mut Frame<'_>, area: Rect) {
        let hint = "↑/↓ select · Space/Enter peek · r refresh · q quit";
        let text = match &self.status_message {
            Some(msg) => format!("{hint}    {msg}"),
            None => hint.to_string(),
        };
        let footer = Paragraph::new(Line::from(vec![Span::styled(
            text,
            Style::default().fg(Color::DarkGray),
        )]));
        f.render_widget(footer, area);
    }

    fn render_peek(&self, f: &mut Frame<'_>, full: Rect) {
        let width = (full.width.saturating_sub(6)).clamp(40, 120);
        let height = (full.height.saturating_sub(4)).max(8);
        let x = full.x + (full.width.saturating_sub(width)) / 2;
        let y = full.y + (full.height.saturating_sub(height)) / 2;
        let area = Rect::new(x, y, width, height);
        f.render_widget(Clear, area);

        let title_text = match &self.peek {
            PeekState::Closed => return,
            PeekState::Loading => "Loading peek…".to_string(),
            PeekState::Ready(content) => {
                let trunc = if content.truncated {
                    " (truncated)"
                } else {
                    ""
                };
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
        let inner = block.inner(area);
        f.render_widget(block, area);

        let lines: Vec<Line<'_>> = match &self.peek {
            PeekState::Closed => return,
            PeekState::Loading => vec![Line::raw("…")],
            PeekState::Failed(_) => vec![Line::from(Span::styled(
                "Press Esc to close",
                Style::default().fg(Color::DarkGray),
            ))],
            PeekState::Ready(content) => peek_lines(&content.rendered),
        };
        let para = Paragraph::new(lines).wrap(Wrap { trim: false });
        f.render_widget(para, inner);
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

fn group_sessions(sessions: &[SessionSummary]) -> Vec<(SessionStatus, Vec<usize>)> {
    let mut buckets: BTreeMap<u8, (SessionStatus, Vec<usize>)> = BTreeMap::new();
    for (idx, s) in sessions.iter().enumerate() {
        let key = match s.status {
            SessionStatus::Active => 0,
            SessionStatus::Idle => 1,
            SessionStatus::Unknown => 2,
        };
        buckets
            .entry(key)
            .or_insert_with(|| (s.status, Vec::new()))
            .1
            .push(idx);
    }
    buckets.into_values().collect()
}

fn status_section_title(status: SessionStatus) -> &'static str {
    match status {
        SessionStatus::Active => "Active",
        SessionStatus::Idle => "Idle",
        SessionStatus::Unknown => "Unknown",
    }
}

fn status_color(status: SessionStatus) -> Color {
    match status {
        SessionStatus::Active => Color::Green,
        SessionStatus::Idle => Color::Yellow,
        SessionStatus::Unknown => Color::DarkGray,
    }
}

fn format_row(
    summary: &SessionSummary,
    selected: bool,
    now: OffsetDateTime,
    width: u16,
) -> Line<'static> {
    let caret = if selected { "› " } else { "  " };
    let caret_span = Span::styled(
        caret.to_string(),
        Style::default().fg(if selected { Color::Cyan } else { Color::Reset }),
    );
    let title_style = if selected {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    let title = Span::styled(summary.title.clone(), title_style);
    let elapsed = elapsed_label(summary.updated_at_offset(), now);
    let cwd = summary
        .cwd
        .as_ref()
        .map(|p| compact_path(p, 30))
        .unwrap_or_else(|| "(no cwd)".to_string());
    let mut tail_parts = vec![cwd];
    if let Some(role) = &summary.agent_role {
        tail_parts.push(format!("[{role}]"));
    }
    if let Some(branch) = &summary.git_branch {
        tail_parts.push(format!("⎇ {branch}"));
    }
    if let Some(src) = &summary.source {
        tail_parts.push(format!("via {src}"));
    }
    tail_parts.push(elapsed);
    let tail = tail_parts.join(" · ");

    let used_caret = caret.width();
    let used_title = summary.title.width();
    let available = (width as usize).saturating_sub(used_caret + used_title + 3);
    let tail_text = if tail.width() > available {
        truncate_to_width(&tail, available)
    } else {
        let pad = available - tail.width();
        format!("{}{tail}", " ".repeat(pad))
    };
    let tail_span = Span::styled(
        format!("  {tail_text}"),
        Style::default().fg(Color::DarkGray),
    );
    Line::from(vec![caret_span, title, tail_span])
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
    let mut suffix: String = display.chars().rev().take(max - 1).collect();
    suffix = suffix.chars().rev().collect();
    format!("…{suffix}")
}

fn truncate_to_width(text: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut used = 0usize;
    for ch in text.chars() {
        let w = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if used + w > max.saturating_sub(1) {
            out.push('…');
            return out;
        }
        used += w;
        out.push(ch);
    }
    out
}

fn default_limit() -> usize {
    50
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::path::PathBuf;

    fn fake_session(title: &str, status: SessionStatus, minutes_ago: i64) -> SessionSummary {
        let now = OffsetDateTime::now_utc();
        let ts = now - time::Duration::minutes(minutes_ago);
        SessionSummary {
            thread_id: Some("00000000-0000-0000-0000-000000000000".to_string()),
            path: PathBuf::from("/tmp/fake.jsonl"),
            title: title.to_string(),
            cwd: Some(PathBuf::from("/tmp/repo")),
            git_branch: Some("main".to_string()),
            agent_nickname: None,
            agent_role: None,
            source: Some("cli".to_string()),
            created_at: Some(
                ts.format(&time::format_description::well_known::Rfc3339)
                    .unwrap(),
            ),
            updated_at: Some(
                ts.format(&time::format_description::well_known::Rfc3339)
                    .unwrap(),
            ),
            status,
        }
    }

    fn buffer_contains(terminal: &Terminal<TestBackend>, needle: &str) -> bool {
        let backend = terminal.backend();
        let buf = backend.buffer();
        let area = buf.area;
        for y in 0..area.height {
            let mut row = String::new();
            for x in 0..area.width {
                let cell = &buf[(area.x + x, area.y + y)];
                row.push_str(cell.symbol());
            }
            if row.contains(needle) {
                return true;
            }
        }
        false
    }

    #[test]
    fn renders_session_list_header_and_rows() {
        let sessions = vec![
            fake_session("investigate auth bug", SessionStatus::Active, 1),
            fake_session("stale session", SessionStatus::Idle, 720),
        ];
        let app = App::new(sessions, PathBuf::from("/tmp/codex-home"));

        let backend = TestBackend::new(120, 24);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal.draw(|f| app.render(f)).expect("draw");

        assert!(buffer_contains(&terminal, "agent view"), "header missing");
        assert!(
            buffer_contains(&terminal, "Active"),
            "active section missing"
        );
        assert!(buffer_contains(&terminal, "Idle"), "idle section missing");
        assert!(
            buffer_contains(&terminal, "investigate auth bug"),
            "title row missing"
        );
        assert!(
            buffer_contains(&terminal, "1m ago") || buffer_contains(&terminal, "just now"),
            "elapsed label missing"
        );
        assert!(
            buffer_contains(&terminal, "Space/Enter peek"),
            "footer hint missing"
        );
    }

    #[test]
    fn renders_empty_state_when_no_sessions() {
        let app = App::new(Vec::new(), PathBuf::from("/tmp/codex-home"));
        let backend = TestBackend::new(100, 12);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal.draw(|f| app.render(f)).expect("draw");

        assert!(
            buffer_contains(&terminal, "No sessions found"),
            "empty state missing"
        );
    }

    #[test]
    fn renders_peek_overlay_when_ready() {
        let sessions = vec![fake_session("explore module", SessionStatus::Active, 1)];
        let mut app = App::new(sessions, PathBuf::from("/tmp/codex-home"));
        app.peek = PeekState::Ready(PeekContent {
            path: PathBuf::from("/tmp/fake.jsonl"),
            turns: Vec::new(),
            rendered: vec![
                PeekLine {
                    kind: "user".to_string(),
                    text: "first question".to_string(),
                },
                PeekLine {
                    kind: "agent".to_string(),
                    text: "first answer".to_string(),
                },
            ],
            truncated: false,
        });

        let backend = TestBackend::new(100, 20);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal.draw(|f| app.render(f)).expect("draw");

        assert!(buffer_contains(&terminal, "Peek"), "peek title missing");
        assert!(
            buffer_contains(&terminal, "first question"),
            "user line missing"
        );
        assert!(
            buffer_contains(&terminal, "first answer"),
            "agent line missing"
        );
    }
}
