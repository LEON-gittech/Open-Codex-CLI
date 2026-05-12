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

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::bottom_pane_view::BottomPaneView;
use crate::bottom_pane::bottom_pane_view::ViewCompletion;
use crate::render::renderable::Renderable;

use super::CancellationEvent;
use super::selection_popup_common;

pub(crate) const BACKGROUND_TASKS_VIEW_ID: &str = "background_tasks";
const MENU_SURFACE_HORIZONTAL_PADDING: u16 = 4;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum BackgroundTaskKind {
    Subagent { thread_id: ThreadId },
    Terminal { process_id: Option<String> },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BackgroundTaskItem {
    pub(crate) kind: BackgroundTaskKind,
    pub(crate) title: String,
    pub(crate) role: Option<String>,
    pub(crate) task: Option<String>,
    pub(crate) elapsed: Option<String>,
    pub(crate) detail: Vec<String>,
    pub(crate) status: Option<String>,
    pub(crate) output_lines: Vec<String>,
    pub(crate) stoppable: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PlanTaskItem {
    pub(crate) status: String,
    pub(crate) step: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct BackgroundTasksViewParams {
    pub(crate) tasks: Vec<PlanTaskItem>,
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

        let terminal_index = index.saturating_sub(subagent_count);
        if terminal_index < self.params.terminals.len() {
            return self.params.terminals.get(terminal_index);
        }
        None
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

    fn index_for_kind(&self, kind: &BackgroundTaskKind) -> Option<usize> {
        (0..self.item_count()).find(|index| {
            self.item_at_index(*index)
                .is_some_and(|item| &item.kind == kind)
        })
    }

    pub(crate) fn update_params(&mut self, params: BackgroundTasksViewParams) {
        let selected_kind = self.selected_item().map(|item| item.kind.clone());
        let detail_kind = self.detail_item().map(|item| item.kind.clone());
        self.params = params;

        if let Some(kind) = selected_kind.as_ref()
            && let Some(index) = self.index_for_kind(kind)
        {
            self.selected_index = index;
        } else {
            let count = self.item_count();
            self.selected_index = if count == 0 {
                0
            } else {
                self.selected_index.min(count - 1)
            };
        }

        if let Some(kind) = detail_kind.as_ref()
            && let Some(index) = self.index_for_kind(kind)
        {
            self.mode = BackgroundTasksMode::Detail { index };
        } else if matches!(self.mode, BackgroundTasksMode::Detail { .. }) {
            self.mode = BackgroundTasksMode::List;
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

        let Some(item) = item.filter(|item| item.stoppable) else {
            return;
        };

        match &item.kind {
            BackgroundTaskKind::Subagent { thread_id } => {
                self.app_event_tx.send(AppEvent::StopBackgroundSubagent {
                    thread_id: *thread_id,
                });
                self.completion = Some(ViewCompletion::Accepted);
            }
            BackgroundTaskKind::Terminal {
                process_id: Some(process_id),
            } => {
                if let Some(thread_id) = self.thread_id {
                    self.app_event_tx.send(AppEvent::StopBackgroundTerminal {
                        thread_id,
                        process_id: process_id.clone(),
                    });
                    self.completion = Some(ViewCompletion::Accepted);
                }
            }
            BackgroundTaskKind::Terminal { process_id: None } => {}
        }
    }

    fn render_lines(&self, width: u16) -> Vec<Line<'static>> {
        if let BackgroundTasksMode::Detail { index } = self.mode {
            return self.render_detail_lines(index, width);
        }

        let total = self.item_count();
        let mut lines = vec![
            "Background tasks".bold().into(),
            format!(
                "{} task{} · {} subagent{} · {} terminal{}",
                self.params.tasks.len(),
                plural(self.params.tasks.len()),
                self.params.subagents.len(),
                plural(self.params.subagents.len()),
                self.params.terminals.len(),
                plural(self.params.terminals.len())
            )
            .dim()
            .into(),
        ];

        append_task_section(&mut lines, &self.params.tasks);
        append_section(
            &mut lines,
            "Subagents",
            &self.params.subagents,
            0,
            self.selected_index,
            width,
        );
        append_section(
            &mut lines,
            "Terminals",
            &self.params.terminals,
            self.params.subagents.len(),
            self.selected_index,
            width,
        );
        lines.push("".into());
        let footer = if total == 0 {
            "Esc close"
        } else {
            "↑/↓ select · Enter view · x stop running · Esc close"
        };
        lines.push(footer.dim().into());
        lines
    }

    fn render_detail_lines(&self, index: usize, width: u16) -> Vec<Line<'static>> {
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
            BackgroundTaskKind::Terminal { .. } => self.render_terminal_detail(item, width),
            BackgroundTaskKind::Subagent { .. } => self.render_subagent_detail(item, width),
        }
    }

    fn render_terminal_detail(&self, item: &BackgroundTaskItem, width: u16) -> Vec<Line<'static>> {
        let mut lines = vec![
            "Shell details".bold().into(),
            status_line(item.status.as_deref().unwrap_or("running")),
        ];
        if let Some(elapsed) = item.elapsed.as_deref() {
            lines.push(label_value_line("Elapsed", elapsed));
        }
        lines.push("".into());
        push_label_value_lines(&mut lines, "Command", &item.title, width);
        if let Some(task) = item.task.as_deref() {
            push_label_value_lines(&mut lines, "Task", task, width);
        }
        lines.push("".into());
        lines.push("Output".bold().into());

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
        lines.push(detail_footer(item, "terminal"));
        lines
    }

    fn render_subagent_detail(&self, item: &BackgroundTaskItem, width: u16) -> Vec<Line<'static>> {
        let mut lines = vec!["Agent details".bold().into()];
        push_label_value_lines(&mut lines, "Agent", &item.title, width);
        if let Some(role) = item.role.as_deref() {
            push_label_value_lines(&mut lines, "Role", role, width);
        }
        if let Some(status) = item.status.as_deref() {
            lines.push(status_line(status));
        }
        if let Some(elapsed) = item.elapsed.as_deref() {
            lines.push(label_value_line("Elapsed", elapsed));
        }
        if let Some(task) = item.task.as_deref() {
            lines.push("".into());
            push_label_value_lines(&mut lines, "Task", task, width);
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
        lines.push(detail_footer(item, "subagent"));
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
        Some(BACKGROUND_TASKS_VIEW_ID)
    }

    fn update_background_tasks(&mut self, params: BackgroundTasksViewParams) -> bool {
        self.update_params(params);
        true
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

fn append_task_section(lines: &mut Vec<Line<'static>>, tasks: &[PlanTaskItem]) {
    lines.push("".into());
    lines.push(format!("Tasks ({})", tasks.len()).bold().into());
    if tasks.is_empty() {
        lines.push("  No tasks.".italic().into());
        return;
    }

    for task in tasks {
        lines.push(Line::from(vec![
            "  ".dim(),
            format!("[{}] ", task.status).dim(),
            task.step.clone().into(),
        ]));
    }
}

fn append_section(
    lines: &mut Vec<Line<'static>>,
    title: &'static str,
    items: &[BackgroundTaskItem],
    selected_offset: usize,
    selected_index: usize,
    width: u16,
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
        if let Some(elapsed) = item.elapsed.as_deref() {
            title.push(" · ".dim());
            title.push(elapsed.to_string().dim());
        }
        lines.push(Line::from(title));
        if let Some(role) = item.role.as_deref() {
            push_label_value_lines(lines, "    Role", role, width);
        }
        if let Some(task) = item.task.as_deref() {
            push_label_value_lines(lines, "    Task", task, width);
        }
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

fn detail_footer(item: &BackgroundTaskItem, noun: &str) -> Line<'static> {
    if item.stoppable {
        format!("← back · Esc/Enter/Space close · x stop {noun}")
            .dim()
            .into()
    } else {
        "← back · Esc/Enter/Space close".dim().into()
    }
}

fn label_value_line(label: &'static str, value: &str) -> Line<'static> {
    Line::from(vec![format!("{label}: ").bold(), value.to_string().into()])
}

fn push_label_value_lines(
    lines: &mut Vec<Line<'static>>,
    label: &'static str,
    value: &str,
    width: u16,
) {
    let prefix = format!("{label}: ");
    let prefix_width = prefix.len();
    let width = usize::from(width.max(1));
    if width <= prefix_width + 1 {
        lines.push(label_value_line(label, value));
        return;
    }

    let options = textwrap::Options::new(width - prefix_width).break_words(true);
    let wrapped = textwrap::wrap(value, options);
    if wrapped.is_empty() {
        lines.push(Line::from(vec![prefix.bold()]));
        return;
    }

    for (index, chunk) in wrapped.into_iter().enumerate() {
        if index == 0 {
            lines.push(Line::from(vec![
                prefix.clone().bold(),
                chunk.into_owned().into(),
            ]));
        } else {
            lines.push(Line::from(vec![
                " ".repeat(prefix_width).dim(),
                chunk.into_owned().into(),
            ]));
        }
    }
}
