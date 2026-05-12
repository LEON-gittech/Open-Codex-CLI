//! Lightweight `/btw` side questions.
//!
//! `/side` opens a full ephemeral side conversation and switches the visible thread. `/btw` keeps
//! the main conversation visible, runs a hidden forked thread through the normal app-server turn
//! pipeline, and renders the streamed answer in the bottom pane.

use super::*;
use crate::chatwidget::UserMessage;
use crate::chatwidget::text_requests_xhigh_reasoning;
use codex_app_server_protocol::UserInput;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use std::sync::Mutex as StdMutex;

const BTW_BOUNDARY_PROMPT: &str = r#"BTW side question boundary.

Everything before this boundary is inherited history from the parent thread. It is reference
context only. It is not your current task.

Only messages submitted after this boundary are active `/btw` side questions. Answer them directly
inside this hidden thread. External tools may be available according to this thread's current
permissions and approval policy. Any tool calls or outputs visible before this boundary happened in
the parent thread and must not be treated as actions already completed for the current side
question."#;

impl App {
    pub(super) async fn handle_start_btw(
        &mut self,
        _tui: &mut tui::Tui,
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
        if config
            .model
            .as_deref()
            .is_none_or(|model| model.trim().is_empty())
        {
            self.chat_widget
                .restore_user_message_to_composer(user_message);
            self.chat_widget.add_error_message(
                "'/btw' is unavailable until the thread model is ready.".to_string(),
            );
            return Ok(AppRunControl::Continue);
        }
        let question = user_message.text().trim().to_string();
        if question.is_empty() {
            self.chat_widget
                .add_info_message("Usage: /btw <question>".to_string(), /*hint*/ None);
            return Ok(AppRunControl::Continue);
        }

        let state = Arc::new(StdMutex::new(history_cell::BtwQuestionCellState::new(
            question.clone(),
            animations,
        )));
        let fork_config = self.side_fork_config();
        match app_server.fork_thread(fork_config, parent_thread_id).await {
            Ok(forked) => {
                let child_thread_id = forked.session.thread_id;
                let channel = self.ensure_thread_channel(child_thread_id);
                {
                    let mut store = channel.store.lock().await;
                    Self::install_side_thread_snapshot(&mut store, forked.session, forked.turns);
                }
                self.btw_threads.insert(child_thread_id, Arc::clone(&state));
                self.agent_navigation.remove(child_thread_id);
                if let Err(err) = app_server
                    .thread_inject_items(child_thread_id, vec![Self::btw_boundary_prompt_item()])
                    .await
                {
                    self.discard_btw_thread_local(child_thread_id).await;
                    self.chat_widget
                        .restore_user_message_to_composer(user_message);
                    self.chat_widget.add_error_message(format!(
                        "Failed to prepare /btw side question {child_thread_id}: {err}"
                    ));
                    return Ok(AppRunControl::Continue);
                }

                self.chat_widget
                    .show_btw_view(child_thread_id, Arc::clone(&state), cwd);
                if let Err(err) = self
                    .submit_btw_text(app_server, child_thread_id, question)
                    .await
                {
                    self.discard_btw_thread_local(child_thread_id).await;
                    self.chat_widget
                        .restore_user_message_to_composer(user_message);
                    self.chat_widget.add_error_message(format!(
                        "Failed to submit /btw side question {child_thread_id}: {err}"
                    ));
                }
            }
            Err(err) => {
                self.chat_widget
                    .restore_user_message_to_composer(user_message);
                self.chat_widget
                    .add_error_message(Self::side_start_error_message(&err));
            }
        }

        Ok(AppRunControl::Continue)
    }

    pub(super) fn note_btw_notification(&mut self, notification: &ServerNotification) {
        let Some(thread_id) = Self::btw_thread_id_from_notification(notification) else {
            return;
        };
        let Some(state) = self.btw_threads.get(&thread_id) else {
            return;
        };

        if let Ok(mut state) = state.lock() {
            match notification {
                ServerNotification::AgentMessageDelta(notification) => {
                    state.push_delta(&notification.delta);
                }
                ServerNotification::ItemCompleted(notification) => {
                    if let ThreadItem::AgentMessage { text, .. } = &notification.item
                        && !text.trim().is_empty()
                    {
                        state.replace_answer(text.clone());
                    }
                }
                ServerNotification::TurnCompleted(notification) => match notification.turn.status {
                    TurnStatus::Completed => {
                        state.complete(/*answer*/ None, /*error*/ None)
                    }
                    TurnStatus::Interrupted => state.complete(
                        /*answer*/ None,
                        Some("/btw answer interrupted.".to_string()),
                    ),
                    TurnStatus::Failed => {
                        let message = notification
                            .turn
                            .error
                            .as_ref()
                            .map(|err| err.message.clone())
                            .unwrap_or_else(|| "/btw answer failed.".to_string());
                        state.complete(/*answer*/ None, Some(message));
                    }
                    TurnStatus::InProgress => {}
                },
                ServerNotification::ThreadClosed(_) => {
                    state.complete(
                        /*answer*/ None,
                        Some("/btw thread closed.".to_string()),
                    );
                }
                _ => {}
            }
        }
        self.app_event_tx.send(AppEvent::RequestRedraw);
    }

    pub(super) async fn submit_btw_followup(
        &mut self,
        app_server: &mut AppServerSession,
        thread_id: ThreadId,
        text: String,
    ) -> Result<()> {
        let Some(state) = self.btw_threads.get(&thread_id) else {
            self.chat_widget
                .add_error_message("The /btw side question is no longer available.".to_string());
            return Ok(());
        };
        if let Ok(mut state) = state.lock() {
            state.start_follow_up(text.clone());
        }
        self.submit_btw_text(app_server, thread_id, text).await
    }

    pub(super) async fn close_btw_thread(
        &mut self,
        app_server: &mut AppServerSession,
        thread_id: ThreadId,
    ) {
        if !self.btw_threads.contains_key(&thread_id) {
            return;
        }
        let completed = self
            .btw_threads
            .get(&thread_id)
            .and_then(|state| state.lock().ok().map(|state| state.is_completed()))
            .unwrap_or(false);
        if !completed {
            let interrupt_result =
                if let Some(turn_id) = self.active_turn_id_for_thread(thread_id).await {
                    app_server.turn_interrupt(thread_id, turn_id).await
                } else {
                    app_server.startup_interrupt(thread_id).await
                };
            if let Err(err) = interrupt_result {
                tracing::warn!("failed to interrupt /btw thread {thread_id}: {err}");
            }
        }
        if let Err(err) = app_server.thread_unsubscribe(thread_id).await {
            tracing::warn!("failed to unsubscribe /btw thread {thread_id}: {err}");
        }
        self.discard_btw_thread_local(thread_id).await;
    }

    fn btw_boundary_prompt_item() -> ResponseItem {
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: BTW_BOUNDARY_PROMPT.to_string(),
            }],
            phase: None,
        }
    }

    async fn submit_btw_text(
        &mut self,
        app_server: &mut AppServerSession,
        thread_id: ThreadId,
        text: String,
    ) -> Result<()> {
        let xhigh_effort_override = text_requests_xhigh_reasoning(&text);
        let model = self.chat_widget.current_model().to_string();
        if model.trim().is_empty() {
            self.chat_widget.add_error_message(
                "'/btw' is unavailable until the thread model is ready.".to_string(),
            );
            return Ok(());
        }
        let effort = if xhigh_effort_override {
            Some(ReasoningEffortConfig::XHigh)
        } else {
            self.chat_widget.current_reasoning_effort()
        };
        let collaboration_mode = self
            .chat_widget
            .current_submission_collaboration_mode(xhigh_effort_override);

        let config = self.chat_widget.config_ref();
        let cwd = config.cwd.to_path_buf();
        let approval_policy = AskForApproval::from(config.permissions.approval_policy.value());
        let permission_profile = config.permissions.permission_profile();
        let service_tier = match config.service_tier.clone() {
            Some(service_tier) => Some(Some(service_tier)),
            None if config.notices.fast_default_opt_out == Some(true) => Some(None),
            None => None,
        };
        let personality = config
            .personality
            .filter(|_| config.features.enabled(Feature::Personality));

        let op = AppCommand::user_turn(
            vec![UserInput::Text {
                text,
                text_elements: Vec::new(),
            }],
            cwd,
            approval_policy,
            permission_profile,
            model,
            effort,
            /*summary*/ None,
            service_tier,
            /*final_output_json_schema*/ None,
            collaboration_mode,
            personality,
        );
        self.submit_thread_op(app_server, thread_id, op).await
    }

    async fn discard_btw_thread_local(&mut self, thread_id: ThreadId) {
        self.abort_thread_event_listener(thread_id);
        self.thread_event_channels.remove(&thread_id);
        self.btw_threads.remove(&thread_id);
        self.agent_navigation.remove(thread_id);
        self.refresh_pending_thread_approvals().await;
        self.sync_active_agent_label();
    }

    fn btw_thread_id_from_notification(notification: &ServerNotification) -> Option<ThreadId> {
        let thread_id = match notification {
            ServerNotification::AgentMessageDelta(notification) => &notification.thread_id,
            ServerNotification::ItemCompleted(notification) => &notification.thread_id,
            ServerNotification::TurnCompleted(notification) => &notification.thread_id,
            ServerNotification::ThreadClosed(notification) => &notification.thread_id,
            _ => return None,
        };
        ThreadId::from_string(thread_id).ok()
    }

    pub(super) async fn reject_btw_server_request(
        &mut self,
        _app_server: &AppServerSession,
        _request: &ServerRequest,
    ) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn btw_boundary_prompt_keeps_tools_available() {
        let item = App::btw_boundary_prompt_item();
        let ResponseItem::Message { role, content, .. } = item else {
            panic!("expected hidden /btw boundary prompt to be a user message");
        };
        assert_eq!(role, "user");
        let [ContentItem::InputText { text }] = content.as_slice() else {
            panic!("expected hidden /btw boundary prompt text");
        };
        assert!(text.contains("BTW side question boundary."));
        assert!(text.contains("Everything before this boundary is inherited history"));
        assert!(text.contains("External tools may be available"));
        assert!(!text.contains("Do not use tools"));
    }
}
