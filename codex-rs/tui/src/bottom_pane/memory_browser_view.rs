use codex_app_server_protocol::MemoryFileKind;
use codex_app_server_protocol::MemoryListResponse;
use codex_app_server_protocol::MemoryOverlayStatusResponse;
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

const MENU_SURFACE_HORIZONTAL_PADDING: u16 = 4;

pub(crate) struct MemoryBrowserView {
    title: String,
    subtitle: String,
    items: Vec<MemoryBrowserItem>,
    selected_index: usize,
    mode: MemoryBrowserMode,
    app_event_tx: AppEventSender,
    show_settings_action: bool,
    completion: Option<ViewCompletion>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct MemoryBrowserItem {
    group: String,
    title: String,
    path: String,
    status: Option<String>,
    content: String,
    metadata: Vec<(String, String)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum MemoryBrowserMode {
    List,
    Detail { index: usize },
}

impl MemoryBrowserView {
    pub(crate) fn from_memory_list(
        response: MemoryListResponse,
        app_event_tx: AppEventSender,
    ) -> Self {
        let items = response
            .files
            .into_iter()
            .map(|file| {
                let group = match file.kind {
                    MemoryFileKind::Summary => "Summary",
                    MemoryFileKind::Index => "Overall index",
                    MemoryFileKind::Topic => "Topics",
                }
                .to_string();
                let mut metadata = vec![("Path".to_string(), file.path.clone())];
                if file.truncated {
                    metadata.push(("Status".to_string(), "content truncated".to_string()));
                }
                MemoryBrowserItem {
                    group,
                    title: file.title,
                    path: file.path,
                    status: file.truncated.then(|| "truncated".to_string()),
                    content: file.content,
                    metadata,
                }
            })
            .collect();
        Self {
            title: "Memory".to_string(),
            subtitle: "Durable memory content grouped by summary, index, and topics.".to_string(),
            items,
            selected_index: 0,
            mode: MemoryBrowserMode::List,
            app_event_tx,
            show_settings_action: true,
            completion: None,
        }
    }

    pub(crate) fn from_overlay_status(
        response: MemoryOverlayStatusResponse,
        app_event_tx: AppEventSender,
    ) -> Self {
        let mut items = Vec::new();
        for thread in response.threads {
            for entry in thread.entries {
                let matched = !entry.durable_matches.is_empty();
                let mut metadata = vec![
                    ("Thread".to_string(), thread.thread_id.clone()),
                    ("Session".to_string(), thread.session_id.clone()),
                    ("Created".to_string(), entry.created_at_unix_ms.to_string()),
                ];
                if let Some(reason) = entry.reason {
                    metadata.push(("Reason".to_string(), reason));
                }
                if matched {
                    metadata.push((
                        "Durable matches".to_string(),
                        entry.durable_matches.join(", "),
                    ));
                }
                items.push(MemoryBrowserItem {
                    group: "Session overlays".to_string(),
                    title: compact_text(&entry.content, 72),
                    path: thread.thread_id.clone(),
                    status: Some(status_label(matched).to_string()),
                    content: entry.content,
                    metadata,
                });
            }
        }
        for note in response.ad_hoc_notes {
            let matched = !note.durable_matches.is_empty();
            let mut metadata = vec![("Path".to_string(), note.path.clone())];
            if let Some(reason) = note.reason {
                metadata.push(("Reason".to_string(), reason));
            }
            if matched {
                metadata.push((
                    "Durable matches".to_string(),
                    note.durable_matches.join(", "),
                ));
            }
            items.push(MemoryBrowserItem {
                group: "Ad-hoc staged notes".to_string(),
                title: compact_text(&note.content, 72),
                path: note.path,
                status: Some(status_label(matched).to_string()),
                content: note.content,
                metadata,
            });
        }
        Self {
            title: "Memory overlay status".to_string(),
            subtitle: "Exact matches are diagnostics; no exact match does not rule out semantic consolidation.".to_string(),
            items,
            selected_index: 0,
            mode: MemoryBrowserMode::List,
            app_event_tx,
            show_settings_action: false,
            completion: None,
        }
    }

    fn move_selection(&mut self, delta: isize) {
        if self.items.is_empty() {
            self.selected_index = 0;
            return;
        }
        self.selected_index =
            (self.selected_index as isize + delta).rem_euclid(self.items.len() as isize) as usize;
    }

    fn activate_selected(&mut self) {
        if !self.items.is_empty() {
            self.mode = MemoryBrowserMode::Detail {
                index: self.selected_index,
            };
        }
    }

    fn render_lines(&self, width: u16) -> Vec<Line<'static>> {
        match self.mode {
            MemoryBrowserMode::List => self.render_list_lines(width),
            MemoryBrowserMode::Detail { index } => self.render_detail_lines(index, width),
        }
    }

    fn render_list_lines(&self, width: u16) -> Vec<Line<'static>> {
        let mut lines = vec![
            self.title.clone().bold().into(),
            self.subtitle.clone().dim().into(),
        ];
        if self.items.is_empty() {
            lines.push("".into());
            lines.push("No memory entries found.".italic().into());
            lines.push("".into());
            lines.push(self.footer().dim().into());
            return lines;
        }

        lines.push("".into());
        let mut previous_group = "";
        for (index, item) in self.items.iter().enumerate() {
            if item.group != previous_group {
                previous_group = item.group.as_str();
                lines.push(item.group.clone().bold().into());
            }
            let marker = if index == self.selected_index {
                "› "
            } else {
                "  "
            };
            let status = item
                .status
                .as_ref()
                .map(|status| format!(" · {status}"))
                .unwrap_or_default();
            lines.push(Line::from(vec![
                marker.cyan(),
                item.title.clone().into(),
                status.dim(),
            ]));
            let preview = compact_text(&item.content, width.saturating_sub(6) as usize);
            lines.push(Line::from(vec!["    ".dim(), preview.dim()]));
        }
        lines.push("".into());
        lines.push(self.footer().dim().into());
        lines
    }

    fn render_detail_lines(&self, index: usize, width: u16) -> Vec<Line<'static>> {
        let Some(item) = self.items.get(index) else {
            return vec![
                self.title.clone().bold().into(),
                "Memory entry is no longer available.".italic().into(),
                "← back · Esc close".dim().into(),
            ];
        };

        let mut lines = vec![
            item.title.clone().bold().into(),
            item.group.clone().dim().into(),
        ];
        for (label, value) in &item.metadata {
            push_label_value(&mut lines, label, value, width);
        }
        lines.push("".into());
        lines.push("Content".bold().into());
        push_wrapped(&mut lines, &item.content, width);
        lines.push("".into());
        lines.push("← back · Esc close".dim().into());
        lines
    }

    fn footer(&self) -> String {
        if self.show_settings_action {
            "↑/↓ select · Enter view · s settings · Esc close".to_string()
        } else {
            "↑/↓ select · Enter view · Esc close".to_string()
        }
    }
}

impl BottomPaneView for MemoryBrowserView {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        if !matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
            return;
        }

        match self.mode {
            MemoryBrowserMode::List => match key_event.code {
                KeyCode::Up => self.move_selection(-1),
                KeyCode::Down => self.move_selection(1),
                KeyCode::Enter => self.activate_selected(),
                KeyCode::Char('s') if self.show_settings_action => {
                    self.app_event_tx.send(AppEvent::OpenMemorySettings);
                    self.completion = Some(ViewCompletion::Accepted);
                }
                KeyCode::Esc | KeyCode::Left => self.completion = Some(ViewCompletion::Cancelled),
                _ => {}
            },
            MemoryBrowserMode::Detail { .. } => match key_event.code {
                KeyCode::Left => self.mode = MemoryBrowserMode::List,
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

    fn on_ctrl_c(&mut self) -> CancellationEvent {
        self.completion = Some(ViewCompletion::Cancelled);
        CancellationEvent::Handled
    }

    fn prefer_esc_to_handle_key_event(&self) -> bool {
        true
    }
}

impl Renderable for MemoryBrowserView {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let inner = selection_popup_common::render_menu_surface(area, buf);
        let width = inner.width.saturating_sub(MENU_SURFACE_HORIZONTAL_PADDING);
        let lines = self.render_lines(width);
        Paragraph::new(lines).render(inner, buf);
    }

    fn desired_height(&self, width: u16) -> u16 {
        self.render_lines(width.saturating_sub(MENU_SURFACE_HORIZONTAL_PADDING))
            .len()
            .try_into()
            .unwrap_or(u16::MAX)
            .saturating_add(selection_popup_common::menu_surface_padding_height())
    }
}

fn status_label(matched: bool) -> &'static str {
    if matched {
        "matched durable memory"
    } else {
        "pending exact match"
    }
}

fn compact_text(text: &str, max_chars: usize) -> String {
    let flattened = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flattened.chars().count() <= max_chars {
        return flattened;
    }
    let mut truncated = flattened
        .chars()
        .take(max_chars.saturating_sub(3))
        .collect::<String>();
    truncated.push_str("...");
    truncated
}

fn push_label_value(lines: &mut Vec<Line<'static>>, label: &str, value: &str, width: u16) {
    let text = format!("{label}: {value}");
    push_wrapped(lines, &text, width);
}

fn push_wrapped(lines: &mut Vec<Line<'static>>, text: &str, width: u16) {
    let width = usize::from(width.max(1));
    for raw_line in text.lines() {
        if raw_line.trim().is_empty() {
            lines.push("".into());
            continue;
        }
        for wrapped in textwrap::wrap(raw_line, width) {
            lines.push(wrapped.into_owned().into());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_app_server_protocol::AdHocMemoryNoteStatus;
    use codex_app_server_protocol::MemoryFileStatus;
    use codex_app_server_protocol::MemoryOverlayEntryStatus;
    use codex_app_server_protocol::ThreadMemoryOverlayStatus;
    use insta::assert_snapshot;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use tokio::sync::mpsc::unbounded_channel;

    fn render_lines(view: &MemoryBrowserView, width: u16) -> String {
        let height = view.desired_height(width);
        let area = Rect::new(0, 0, width, height);
        let mut buf = Buffer::empty(area);
        view.render(area, &mut buf);

        (0..area.height)
            .map(|row| {
                let rendered = (0..area.width)
                    .map(|col| {
                        let symbol = buf[(area.x + col, area.y + row)].symbol();
                        if symbol.is_empty() {
                            " ".to_string()
                        } else {
                            symbol.to_string()
                        }
                    })
                    .collect::<String>();
                rendered.trim_end().to_string()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn sender() -> AppEventSender {
        let (tx, _rx) = unbounded_channel();
        AppEventSender::new(tx)
    }

    #[test]
    fn renders_memory_list_grouped_by_role() {
        let view = MemoryBrowserView::from_memory_list(
            MemoryListResponse {
                files: vec![
                    MemoryFileStatus {
                        kind: MemoryFileKind::Summary,
                        path: "memory_summary.md".to_string(),
                        title: "Summary".to_string(),
                        content: "# Summary\n\nGeneral memory summary.".to_string(),
                        truncated: false,
                    },
                    MemoryFileStatus {
                        kind: MemoryFileKind::Index,
                        path: "MEMORY.md".to_string(),
                        title: "Memory Index".to_string(),
                        content: "# Memory Index\n\n- Codex topic".to_string(),
                        truncated: false,
                    },
                    MemoryFileStatus {
                        kind: MemoryFileKind::Topic,
                        path: "topics/codex.md".to_string(),
                        title: "Codex".to_string(),
                        content: "# Codex\n\nTopic body".to_string(),
                        truncated: false,
                    },
                ],
            },
            sender(),
        );

        assert_snapshot!("memory_browser_list", render_lines(&view, /*width*/ 88));
    }

    #[test]
    fn renders_memory_overlay_grouped_by_source() {
        let view = MemoryBrowserView::from_overlay_status(
            MemoryOverlayStatusResponse {
                threads: vec![ThreadMemoryOverlayStatus {
                    thread_id: "thread-1".to_string(),
                    session_id: "session-1".to_string(),
                    entries: vec![MemoryOverlayEntryStatus {
                        content: "remember this session detail".to_string(),
                        reason: Some("user asked".to_string()),
                        created_at_unix_ms: 42,
                        durable_matches: Vec::new(),
                    }],
                }],
                ad_hoc_notes: vec![AdHocMemoryNoteStatus {
                    path: "extensions/ad_hoc/notes/one.md".to_string(),
                    content: "durable note text".to_string(),
                    reason: None,
                    durable_matches: vec!["topics/codex.md".to_string()],
                }],
            },
            sender(),
        );

        assert_snapshot!("memory_browser_overlay", render_lines(&view, /*width*/ 96));
    }
}
