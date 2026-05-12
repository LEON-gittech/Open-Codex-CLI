use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

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
use crate::history_cell;
use crate::history_cell::HistoryCell;
use crate::render::renderable::Renderable;

use super::CancellationEvent;
use super::selection_popup_common;

const MENU_SURFACE_HORIZONTAL_PADDING: u16 = 4;

pub(crate) struct BtwView {
    thread_id: ThreadId,
    cell: history_cell::BtwQuestionCell,
    app_event_tx: AppEventSender,
    draft: String,
    scroll_offset: usize,
    completion: Option<ViewCompletion>,
}

impl BtwView {
    pub(crate) fn new(
        thread_id: ThreadId,
        state: Arc<Mutex<history_cell::BtwQuestionCellState>>,
        cwd: PathBuf,
        app_event_tx: AppEventSender,
    ) -> Self {
        Self {
            thread_id,
            cell: history_cell::new_btw_question_cell(state, cwd.as_path()),
            app_event_tx,
            draft: String::new(),
            scroll_offset: 0,
            completion: None,
        }
    }

    fn submit_follow_up(&mut self) {
        let text = self.draft.trim().to_string();
        if text.is_empty() {
            return;
        }
        self.draft.clear();
        self.app_event_tx.send(AppEvent::SubmitBtwFollowup {
            thread_id: self.thread_id,
            text,
        });
    }

    fn close(&mut self) {
        self.app_event_tx.send(AppEvent::CloseBtw {
            thread_id: self.thread_id,
        });
        self.completion = Some(ViewCompletion::Cancelled);
    }

    fn lines(&self, width: u16) -> Vec<Line<'static>> {
        let mut lines = vec!["BTW".magenta().bold().into()];
        lines.extend(self.cell.display_lines(width));
        lines.push("".into());
        push_wrapped_value(&mut lines, "Follow-up", &self.draft, width);
        lines.push("".into());
        lines.push("Enter send · Up/Down scroll · Esc close".dim().into());
        lines
    }

    fn max_scroll(&self, area_height: u16, width: u16) -> usize {
        self.lines(width)
            .len()
            .saturating_sub(usize::from(area_height))
    }
}

impl BottomPaneView for BtwView {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        if !matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
            return;
        }

        match key_event.code {
            KeyCode::Esc => self.close(),
            KeyCode::Enter => self.submit_follow_up(),
            KeyCode::Up => {
                self.scroll_offset = self.scroll_offset.saturating_sub(1);
            }
            KeyCode::Down => {
                self.scroll_offset = self.scroll_offset.saturating_add(1);
            }
            KeyCode::Backspace => {
                self.draft.pop();
            }
            KeyCode::Char(ch) => {
                self.draft.push(ch);
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

    fn on_ctrl_c(&mut self) -> CancellationEvent {
        self.close();
        CancellationEvent::Handled
    }

    fn prefer_esc_to_handle_key_event(&self) -> bool {
        true
    }
}

impl Renderable for BtwView {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let content_area = selection_popup_common::render_menu_surface(area, buf);
        let scroll_offset = self
            .scroll_offset
            .min(self.max_scroll(content_area.height, content_area.width));
        Paragraph::new(self.lines(content_area.width))
            .scroll((scroll_offset as u16, 0))
            .render(content_area, buf);
    }

    fn desired_height(&self, width: u16) -> u16 {
        self.lines(width.saturating_sub(MENU_SURFACE_HORIZONTAL_PADDING))
            .len()
            .try_into()
            .unwrap_or(u16::MAX)
            .saturating_add(selection_popup_common::menu_surface_padding_height())
    }
}

fn push_wrapped_value(
    lines: &mut Vec<Line<'static>>,
    label: &'static str,
    value: &str,
    width: u16,
) {
    let shown = if value.is_empty() { "" } else { value };
    let prefix = format!("{label}: ");
    let prefix_width = prefix.len();
    let width = usize::from(width.max(1));
    if width <= prefix_width + 1 {
        lines.push(Line::from(vec![prefix.bold(), shown.to_string().into()]));
        return;
    }

    let options = textwrap::Options::new(width - prefix_width).break_words(true);
    let wrapped = textwrap::wrap(shown, options);
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

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn render_view(view: &BtwView, width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal
            .draw(|frame| view.render(frame.area(), frame.buffer_mut()))
            .expect("draw");
        terminal.backend().to_string()
    }

    #[test]
    fn wraps_long_answer_inside_bottom_pane_surface() {
        let state = Arc::new(Mutex::new(history_cell::BtwQuestionCellState::new(
            "介绍凝聚态物理".to_string(),
            /*animations_enabled*/ false,
        )));
        state.lock().expect("state").complete(
            Some("凝聚态物理是研究大量粒子在相互作用下形成的凝聚态物质性质的物理学分支，最常见对象是固体和液体。".to_string()),
            None,
        );
        let view = BtwView::new(
            ThreadId::new(),
            state,
            PathBuf::from("/tmp"),
            AppEventSender::new(tokio::sync::mpsc::unbounded_channel().0),
        );

        let rendered = render_view(&view, 48, 16);
        assert!(rendered.contains("凝聚态物理是研究大量粒子"), "{rendered}");
        assert!(rendered.contains("最常见对象"), "{rendered}");
    }

    #[test]
    fn enter_submits_follow_up_to_btw_thread() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let thread_id = ThreadId::new();
        let state = Arc::new(Mutex::new(history_cell::BtwQuestionCellState::new(
            "first".to_string(),
            /*animations_enabled*/ false,
        )));
        let mut view = BtwView::new(
            thread_id,
            state,
            PathBuf::from("/tmp"),
            AppEventSender::new(tx),
        );
        for ch in "follow up".chars() {
            view.handle_key_event(KeyEvent::from(KeyCode::Char(ch)));
        }
        view.handle_key_event(KeyEvent::from(KeyCode::Enter));

        let event = rx.try_recv().expect("event");
        match event {
            AppEvent::SubmitBtwFollowup {
                thread_id: actual_thread_id,
                text,
            } => {
                assert_eq!(actual_thread_id, thread_id);
                assert_eq!(text, "follow up");
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }
}
