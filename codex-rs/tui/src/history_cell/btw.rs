//! `/btw` side-question history cells.
//!
//! These types support the lightweight `/btw` inline question flow, rendering the question and its
//! streamed answer in the bottom pane.

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

use ratatui::style::Stylize;
use ratatui::text::Line;

use super::*;

/// Shared mutable state for a `/btw` side question, updated from notification handlers.
#[derive(Debug)]
pub(crate) struct BtwQuestionCellState {
    question: String,
    answer: String,
    error: Option<String>,
    completed: bool,
    animations_enabled: bool,
}

impl BtwQuestionCellState {
    pub(crate) fn new(question: String, animations_enabled: bool) -> Self {
        Self {
            question,
            answer: String::new(),
            error: None,
            completed: false,
            animations_enabled,
        }
    }

    pub(crate) fn push_delta(&mut self, delta: &str) {
        self.answer.push_str(delta);
    }

    pub(crate) fn replace_answer(&mut self, text: String) {
        self.answer = text;
    }

    pub(crate) fn complete(&mut self, answer: Option<String>, error: Option<String>) {
        if let Some(answer) = answer {
            self.answer = answer;
        }
        self.error = error;
        self.completed = true;
    }

    pub(crate) fn start_follow_up(&mut self, text: String) {
        self.question = text;
        self.answer.clear();
        self.error = None;
        self.completed = false;
    }

    pub(crate) fn is_completed(&self) -> bool {
        self.completed
    }
}

/// A renderable cell that wraps shared `BtwQuestionCellState`.
#[derive(Debug)]
pub(crate) struct BtwQuestionCell {
    state: Arc<Mutex<BtwQuestionCellState>>,
    cwd: PathBuf,
}

impl HistoryCell for BtwQuestionCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        let Ok(state) = self.state.lock() else {
            return vec![Line::from("(btw state unavailable)".dim())];
        };
        let mut lines = Vec::new();
        lines.push(Line::from(format!("Q: {}", state.question).bold()));

        if let Some(error) = &state.error {
            lines.push(Line::from(error.clone().red()));
        } else if state.answer.is_empty() && !state.completed {
            lines.push(Line::from("thinking...".dim()));
        } else {
            let wrap_width = (width as usize).max(1);
            let wrapped = textwrap::wrap(&state.answer, wrap_width);
            for chunk in wrapped {
                lines.push(Line::from(chunk.into_owned()));
            }
        }

        lines
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        let Ok(state) = self.state.lock() else {
            return Vec::new();
        };
        let mut lines = vec![Line::from(format!("Q: {}", state.question))];
        if !state.answer.is_empty() {
            lines.push(Line::from(state.answer.clone()));
        }
        if let Some(error) = &state.error {
            lines.push(Line::from(error.clone()));
        }
        lines
    }
}

/// Create a new `BtwQuestionCell` backed by the given shared state.
pub(crate) fn new_btw_question_cell(
    state: Arc<Mutex<BtwQuestionCellState>>,
    cwd: &Path,
) -> BtwQuestionCell {
    BtwQuestionCell {
        state,
        cwd: cwd.to_path_buf(),
    }
}
