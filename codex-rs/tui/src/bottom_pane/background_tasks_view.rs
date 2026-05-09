use codex_protocol::ThreadId;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;

use crate::app_command::AppCommand;
use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::bottom_pane_view::BottomPaneView;
use crate::bottom_pane::bottom_pane_view::ViewCompletion;
use crate::render::renderable::Renderable;

use super::CancellationEvent;
use super::selection_popup_common;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum BackgroundTaskKind {
    Subagent { thread_id: ThreadId },
    Terminal,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BackgroundTaskItem {
    pub(crate) kind: BackgroundTaskKind,
    pub(crate) title: String,
    pub(crate) detail: Vec<String>,
    pub(crate) status: Option<String>,
    pub(crate) output_lines: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct BackgroundTasksViewParams {
    pub(crate) subagents: Vec<BackgroundTaskItem>,
    pub(crate) terminals: Vec<BackgroundTaskItem>,
}

pub(crate) struct BackgroundTasksView {
    params: BackgroundTasksViewParams,
    selected_index: usize,
    mode: BackgroundTasksMode,
    thread_id: Option<ThreadId>,
    app_event_tx: AppEventSender,
    completion: Option<ViewCompletion>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
enum BackgroundTasksMode {
    #[default]
    List,
    Detail {
        index: usize,
    },
}

impl BackgroundTasksView {
    pub(crate) fn new(
        params: BackgroundTasksViewParams,
        thread_id: Option<ThreadId>,
        app_event_tx: AppEventSender,
    ) -> Self {
        Self {
            params,
            selected_index: 0,
            mode: BackgroundTasksMode::List,
            thread_id,
            app_event_tx,
            completion: None,
        }
    }

    fn item_count(&self) -> usize {
        self.params.subagents.len() + self.params.terminals.len()
    }

    fn item_at_index(&self, index: usize) -> Option<&BackgroundTaskItem> {
        let subagent_count = self.params.subagents.len();
        if index < subagent_count {
            return self.params.subagents.get(index);
        }

        self.params
            .terminals
            .get(index.saturating_sub(subagent_count))
    }

    fn selected_item(&self) -> Option<&BackgroundTaskItem> {
        self.item_at_index(self.selected_index)
    }

    fn detail_item(&self) -> Option<&BackgroundTaskItem> {
        match self.mode {
            BackgroundTasksMode::List => None,
            BackgroundTasksMode::Detail { index } => self.item_at_index(index),
        }
    }

    fn move_selection(&mut self, delta: isize) {
        let count = self.item_count();
        if count == 0 {
            self.selected_index = 0;
            return;
        }

        let next = (self.selected_index as isize + delta).rem_euclid(count as isize);
        self.selected_index = next as usize;
    }

    fn activate_selected(&mut self) {
        if self.selected_item().is_some() {
            self.mode = BackgroundTasksMode::Detail {
                index: self.selected_index,
            };
        }
    }

    fn stop_selected(&mut self) {
        let item = match self.mode {
            BackgroundTasksMode::List => self.selected_item(),
            BackgroundTasksMode::Detail { .. } => self.detail_item(),
        };

        if !matches!(
            item.map(|item| &item.kind),
            Some(BackgroundTaskKind::Terminal)
        ) {
            return;
        }

        if let Some(thread_id) = self.thread_id {
            self.app_event_tx.send(AppEvent::SubmitThreadOp {
                thread_id,
                op: AppCommand::clean_background_terminals(),
            });
            self.completion = Some(ViewCompletion::Accepted);
        }
    }

    fn render_lines(&self) -> Vec<Line<'static>> {
        if let BackgroundTasksMode::Detail { index } = self.mode {
            return self.render_detail_lines(index);
        }

        let total = self.item_count();
        let mut lines = vec![
            "Background tasks".bold().into(),
            format!(
                "{} subagent{} · {} terminal{}",
                self.params.subagents.len(),
                plural(self.params.subagents.len()),
                self.params.terminals.len(),
                plural(self.params.terminals.len())
            )
            .dim()
            .into(),
        ];

        if total == 0 {
            lines.push("".into());
            lines.push("No background tasks.".italic().into());
            return lines;
        }

        append_section(
            &mut lines,
            "Subagents",
            &self.params.subagents,
            0,
            self.selected_index,
        );
        append_section(
            &mut lines,
            "Terminals",
            &self.params.terminals,
            self.params.subagents.len(),
            self.selected_index,
        );
        lines.push("".into());
        lines.push(
            "↑/↓ select · Enter view · x stop terminal · Esc close"
                .dim()
                .into(),
        );
        lines
    }

    fn render_detail_lines(&self, index: usize) -> Vec<Line<'static>> {
        let Some(item) = self.item_at_index(index) else {
            return vec![
                "Background tasks".bold().into(),
                "".into(),
                "Task is no longer available.".italic().into(),
                "".into(),
                "← back · Esc close".dim().into(),
            ];
        };

        match &item.kind {
            BackgroundTaskKind::Terminal => self.render_terminal_detail(item),
            BackgroundTaskKind::Subagent { .. } => self.render_subagent_detail(item),
        }
    }

    fn render_terminal_detail(&self, item: &BackgroundTaskItem) -> Vec<Line<'static>> {
        let mut lines = vec![
            "Shell details".bold().into(),
            status_line(item.status.as_deref().unwrap_or("running")),
            "".into(),
            label_value_line("Command", &item.title),
            "".into(),
            "Output".bold().into(),
        ];

        if item.output_lines.is_empty() {
            lines.push("  No output yet.".italic().into());
        } else {
            for output in item
                .output_lines
                .iter()
                .rev()
                .take(10)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
            {
                lines.push(Line::from(vec!["  ".dim(), output.clone().into()]));
            }
        }

        lines.push("".into());
        lines.push(
            "← back · Esc/Enter/Space close · x stop terminal"
                .dim()
                .into(),
        );
        lines
    }

    fn render_subagent_detail(&self, item: &BackgroundTaskItem) -> Vec<Line<'static>> {
        let mut lines = vec![
            "Agent details".bold().into(),
            label_value_line("Agent", &item.title),
        ];
        if let Some(status) = item.status.as_deref() {
            lines.push(status_line(status));
        }

        lines.push("".into());
        lines.push("Progress".bold().into());
        let progress_lines = if item.detail.is_empty() {
            vec![item.title.as_str()]
        } else {
            item.detail.iter().map(String::as_str).collect()
        };
        for progress in progress_lines.into_iter().take(12) {
            lines.push(Line::from(vec!["  ".dim(), progress.to_string().into()]));
        }

        lines.push("".into());
        lines.push("← back · Esc/Enter/Space close".dim().into());
        lines
    }
}

impl BottomPaneView for BackgroundTasksView {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        if !matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
            return;
        }

        match self.mode {
            BackgroundTasksMode::List => match key_event.code {
                KeyCode::Up => self.move_selection(-1),
                KeyCode::Down => self.move_selection(1),
                KeyCode::Enter => self.activate_selected(),
                KeyCode::Char('x') => self.stop_selected(),
                KeyCode::Esc | KeyCode::Left => self.completion = Some(ViewCompletion::Cancelled),
                _ => {}
            },
            BackgroundTasksMode::Detail { .. } => match key_event.code {
                KeyCode::Left => self.mode = BackgroundTasksMode::List,
                KeyCode::Char('x') => self.stop_selected(),
                KeyCode::Esc | KeyCode::Enter | KeyCode::Char(' ') => {
                    self.completion = Some(ViewCompletion::Cancelled)
                }
                _ => {}
            },
        }
    }

    fn is_complete(&self) -> bool {
        self.completion.is_some()
    }

    fn completion(&self) -> Option<ViewCompletion> {
        self.completion
    }

    fn view_id(&self) -> Option<&'static str> {
        Some("background_tasks")
    }

    fn selected_index(&self) -> Option<usize> {
        Some(self.selected_index)
    }

    fn on_ctrl_c(&mut self) -> CancellationEvent {
        self.completion = Some(ViewCompletion::Cancelled);
        CancellationEvent::Handled
    }

    fn prefer_esc_to_handle_key_event(&self) -> bool {
        true
    }
}

impl Renderable for BackgroundTasksView {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let content_area = selection_popup_common::render_menu_surface(area, buf);
        Paragraph::new(self.render_lines()).render(content_area, buf);
    }

    fn desired_height(&self, _width: u16) -> u16 {
        self.render_lines()
            .len()
            .try_into()
            .unwrap_or(u16::MAX)
            .saturating_add(selection_popup_common::menu_surface_padding_height())
    }
}

fn append_section(
    lines: &mut Vec<Line<'static>>,
    title: &'static str,
    items: &[BackgroundTaskItem],
    selected_offset: usize,
    selected_index: usize,
) {
    lines.push("".into());
    lines.push(format!("{title} ({})", items.len()).bold().into());
    if items.is_empty() {
        lines.push(
            format!("  No background {}.", title.to_ascii_lowercase())
                .italic()
                .into(),
        );
        return;
    }

    for (item_index, item) in items.iter().enumerate() {
        let global_index = selected_offset + item_index;
        let prefix = if global_index == selected_index {
            "› ".cyan()
        } else {
            "  ".into()
        };
        let mut title = vec![prefix, item.title.clone().into()];
        if let Some(status) = item.status.as_deref() {
            title.push(": ".dim());
            title.push(status.to_string().dim());
        }
        lines.push(Line::from(title));
        for detail in item.detail.iter().take(2) {
            lines.push(Line::from(vec!["    ↳ ".dim(), detail.clone().dim()]));
        }
    }
}

fn plural(count: usize) -> &'static str {
    if count == 1 { "" } else { "s" }
}

fn status_line(status: &str) -> Line<'static> {
    label_value_line("Status", status)
}

fn label_value_line(label: &'static str, value: &str) -> Line<'static> {
    Line::from(vec![format!("{label}: ").bold(), value.to_string().into()])
}
