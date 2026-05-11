//! Lightweight `/btw` side questions.
//!
//! `/side` opens a full ephemeral side conversation and switches the visible thread. `/btw` keeps
//! the main conversation visible and asks one transient, no-tools side question through app-server.

use super::*;
use crate::chatwidget::UserMessage;
use codex_app_server_protocol::BtwStartParams;
use std::sync::Mutex as StdMutex;

impl App {
    pub(super) async fn handle_start_btw(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        parent_thread_id: ThreadId,
        user_message: UserMessage,
    ) -> Result<AppRunControl> {
        if self.primary_thread_id.is_none() {
            self.chat_widget
                .restore_user_message_to_composer(user_message);
            self.chat_widget.add_error_message(
                "'/btw' is unavailable until the main thread is ready.".to_string(),
            );
            return Ok(AppRunControl::Continue);
        }

        self.session_telemetry.counter(
            "codex.thread.btw",
            /*inc*/ 1,
            &[("source", "slash_command")],
        );
        self.refresh_in_memory_config_from_disk_best_effort("starting a /btw side question")
            .await;

        let config = self.chat_widget.config_ref();
        let animations = config.animations;
        let cwd = config.cwd.to_path_buf();
        let service_tier = config.service_tier.clone();
        let effort = config.model_reasoning_effort;
        let model = match config.model.clone() {
            Some(model) if !model.trim().is_empty() => model,
            _ => {
                self.chat_widget
                    .restore_user_message_to_composer(user_message);
                self.chat_widget.add_error_message(
                    "'/btw' is unavailable until the thread model is ready.".to_string(),
                );
                return Ok(AppRunControl::Continue);
            }
        };
        let question = user_message.text().trim().to_string();
        if question.is_empty() {
            self.chat_widget
                .add_info_message("Usage: /btw <question>".to_string(), /*hint*/ None);
            return Ok(AppRunControl::Continue);
        }

        let btw_id = format!("btw-{}", Uuid::new_v4());
        let state = Arc::new(StdMutex::new(history_cell::BtwQuestionCellState::new(
            question.clone(),
            animations,
        )));
        self.pending_btw_questions
            .insert(btw_id.clone(), Arc::clone(&state));
        let _ = tui.enter_alt_screen();
        let overlay_cell: Box<dyn history_cell::HistoryCell> = Box::new(
            history_cell::new_btw_question_cell(Arc::clone(&state), cwd.as_path()),
        );
        let overlay_renderable: Box<dyn Renderable> = Box::new(overlay_cell);
        self.overlay = Some(Overlay::new_static_with_renderables(
            vec![overlay_renderable],
            "B T W".to_string(),
            self.keymap.pager.clone(),
        ));
        tui.frame_requester().schedule_frame();

        let response = app_server
            .btw_start(BtwStartParams {
                btw_id: btw_id.clone(),
                thread_id: parent_thread_id.to_string(),
                question,
                model: Some(model),
                service_tier,
                effort,
            })
            .await;
        if let Err(err) = response
            && let Some(state) = self.pending_btw_questions.remove(&btw_id)
            && let Ok(mut state) = state.lock()
        {
            state.complete(None, Some(format!("Failed to start /btw question: {err}")));
        }

        Ok(AppRunControl::Continue)
    }

    pub(super) fn note_btw_notification(&mut self, notification: &ServerNotification) -> bool {
        match notification {
            ServerNotification::BtwTextDelta(notification) => {
                let Some(state) = self.pending_btw_questions.get(&notification.btw_id) else {
                    return false;
                };
                if let Ok(mut state) = state.lock() {
                    state.push_delta(&notification.delta);
                }
                true
            }
            ServerNotification::BtwCompleted(notification) => {
                let Some(state) = self.pending_btw_questions.remove(&notification.btw_id) else {
                    return false;
                };
                if let Ok(mut state) = state.lock() {
                    state.complete(notification.answer.clone(), notification.error.clone());
                }
                true
            }
            _ => false,
        }
    }

    pub(super) async fn reject_btw_server_request(
        &mut self,
        _app_server: &AppServerSession,
        _request: &ServerRequest,
    ) -> bool {
        false
    }
}
