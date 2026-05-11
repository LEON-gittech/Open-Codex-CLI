//! Lightweight `/btw` side questions.
//!
//! `/side` opens a full ephemeral side conversation and switches the visible thread. `/btw` keeps
//! the main conversation visible: it forks a hidden ephemeral thread, asks one constrained question,
//! and inserts the resulting answer back into the current transcript.

use super::app_server_event_targets::server_request_thread_id;
use super::*;
use crate::chatwidget::UserMessage;
use codex_app_server_protocol::RequestId as AppServerRequestId;
use codex_app_server_protocol::UserInput;

const BTW_DEVELOPER_INSTRUCTIONS: &str = r#"You are answering a one-shot /btw side question.

The main thread is not interrupted. Use the inherited context only as reference. Answer the user's side question directly in one response.

Do not call tools, read files, run commands, request approvals, modify files, modify git state, or take any workspace action. If the answer is not available from the inherited context, say so briefly instead of trying to investigate."#;

fn wrap_btw_question(question: &str) -> String {
    format!(
        "<system-reminder>This is a /btw side question. Answer directly in one response. You have no tools available and must not take actions.</system-reminder>\n\n{question}"
    )
}

#[derive(Debug, Clone)]
pub(super) struct BtwQuestionState {
    pub(super) parent_thread_id: ThreadId,
    pub(super) question: String,
    pub(super) answer: String,
    pub(super) completed: bool,
    pub(super) failed: Option<String>,
}

impl BtwQuestionState {
    fn new(parent_thread_id: ThreadId, question: String) -> Self {
        Self {
            parent_thread_id,
            question,
            answer: String::new(),
            completed: false,
            failed: None,
        }
    }
}

impl App {
    fn btw_fork_config(&self) -> Config {
        let mut fork_config = self.side_fork_config();
        fork_config.developer_instructions = Some(match fork_config.developer_instructions {
            Some(existing) if !existing.trim().is_empty() => {
                format!("{existing}\n\n{BTW_DEVELOPER_INSTRUCTIONS}")
            }
            _ => BTW_DEVELOPER_INSTRUCTIONS.to_string(),
        });
        fork_config
    }

    pub(super) async fn handle_start_btw(
        &mut self,
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

        let fork_config = self.btw_fork_config();
        let model = match fork_config.model.clone() {
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

        match app_server
            .fork_thread(fork_config.clone(), parent_thread_id)
            .await
        {
            Ok(forked) => {
                let child_thread_id = forked.session.thread_id;
                let channel = self.ensure_thread_channel(child_thread_id);
                {
                    let mut store = channel.store.lock().await;
                    Self::install_side_thread_snapshot(&mut store, forked.session, forked.turns);
                }
                self.pending_btw_questions.insert(
                    child_thread_id,
                    BtwQuestionState::new(parent_thread_id, question.clone()),
                );
                if let Err(err) = app_server
                    .thread_inject_items(child_thread_id, vec![Self::side_boundary_prompt_item()])
                    .await
                {
                    self.pending_btw_questions.remove(&child_thread_id);
                    self.remove_btw_thread_local(child_thread_id).await;
                    self.chat_widget
                        .restore_user_message_to_composer(UserMessage::from(question));
                    self.chat_widget
                        .add_error_message(format!("Failed to prepare /btw question: {err}"));
                    return Ok(AppRunControl::Continue);
                }

                let permission_profile = fork_config.permissions.permission_profile();
                let service_tier = match fork_config.service_tier.clone() {
                    Some(service_tier) => Some(Some(service_tier)),
                    None if fork_config.notices.fast_default_opt_out == Some(true) => Some(None),
                    None => None,
                };
                let personality = fork_config
                    .personality
                    .filter(|_| fork_config.features.enabled(Feature::Personality));
                let op = AppCommand::user_turn(
                    vec![UserInput::Text {
                        text: wrap_btw_question(&question),
                        text_elements: Vec::new(),
                    }],
                    fork_config.cwd.to_path_buf(),
                    AskForApproval::from(fork_config.permissions.approval_policy.value()),
                    permission_profile,
                    model,
                    fork_config.model_reasoning_effort,
                    /*summary*/ None,
                    service_tier,
                    /*final_output_json_schema*/ None,
                    /*collaboration_mode*/ None,
                    personality,
                );
                self.submit_thread_op(app_server, child_thread_id, op)
                    .await?;
                self.chat_widget
                    .add_info_message(format!("/btw {question}"), Some("Answering...".to_string()));
            }
            Err(err) => {
                self.chat_widget
                    .restore_user_message_to_composer(UserMessage::from(question));
                self.chat_widget
                    .add_error_message(format!("Failed to start /btw question: {err}"));
            }
        }

        Ok(AppRunControl::Continue)
    }

    pub(super) fn note_btw_notification(&mut self, notification: &ServerNotification) -> bool {
        let thread_id = match notification {
            ServerNotification::ItemCompleted(notification) => {
                ThreadId::from_string(&notification.thread_id).ok()
            }
            ServerNotification::TurnCompleted(notification) => {
                ThreadId::from_string(&notification.thread_id).ok()
            }
            ServerNotification::Error(notification) => {
                ThreadId::from_string(&notification.thread_id).ok()
            }
            ServerNotification::ThreadClosed(notification) => {
                ThreadId::from_string(&notification.thread_id).ok()
            }
            _ => None,
        };
        let Some(thread_id) = thread_id else {
            return false;
        };
        let Some(state) = self.pending_btw_questions.get_mut(&thread_id) else {
            return false;
        };

        match notification {
            ServerNotification::ItemCompleted(notification) => {
                if let ThreadItem::AgentMessage { text, .. } = &notification.item
                    && !text.trim().is_empty()
                {
                    if !state.answer.is_empty() {
                        state.answer.push_str("\n\n");
                    }
                    state.answer.push_str(text.trim());
                }
            }
            ServerNotification::TurnCompleted(notification) => {
                if !state.completed {
                    state.completed = true;
                    if !matches!(notification.turn.status, TurnStatus::Completed) {
                        state.failed = Some(format!(
                            "/btw side question ended with status {:?}.",
                            notification.turn.status
                        ));
                    }
                    self.app_event_tx.send(AppEvent::CompleteBtw { thread_id });
                }
            }
            ServerNotification::Error(notification) => {
                if !state.completed {
                    state.completed = true;
                    state.failed = Some(notification.error.message.clone());
                    self.app_event_tx.send(AppEvent::CompleteBtw { thread_id });
                }
            }
            ServerNotification::ThreadClosed(_) => {
                if !state.completed {
                    state.completed = true;
                    state.failed = Some("/btw side question closed before answering.".to_string());
                    self.app_event_tx.send(AppEvent::CompleteBtw { thread_id });
                }
            }
            _ => {}
        }

        true
    }

    pub(super) async fn reject_btw_server_request(
        &mut self,
        app_server: &AppServerSession,
        request: &ServerRequest,
    ) -> bool {
        let Some(thread_id) = server_request_thread_id(request) else {
            return false;
        };
        if !self.pending_btw_questions.contains_key(&thread_id) {
            return false;
        }
        let Some(request_id) = request_id_for_server_request(request) else {
            return true;
        };
        if let Err(err) = self
            .reject_app_server_request(
                app_server,
                request_id,
                "/btw side questions cannot use tools.".to_string(),
            )
            .await
        {
            tracing::warn!("{err}");
        }
        true
    }

    pub(super) async fn handle_complete_btw(
        &mut self,
        app_server: &mut AppServerSession,
        thread_id: ThreadId,
    ) {
        let Some(state) = self.pending_btw_questions.remove(&thread_id) else {
            return;
        };
        if let Some(error) = state.failed {
            self.chat_widget.add_error_message(error);
        } else if state.answer.trim().is_empty() {
            self.chat_widget
                .add_error_message("/btw side question produced no answer.".to_string());
        } else {
            self.chat_widget.add_plain_history_lines(vec![
                vec![
                    "/btw ".bold(),
                    state.question.dim(),
                    format!(" · from {}", state.parent_thread_id).dark_gray(),
                ]
                .into(),
            ]);
            self.chat_widget
                .add_to_history(history_cell::AgentMarkdownCell::new(
                    state.answer,
                    self.chat_widget.config_ref().cwd.as_path(),
                ));
        }

        if let Err(err) = app_server.thread_unsubscribe(thread_id).await {
            tracing::warn!("failed to unsubscribe /btw side question {thread_id}: {err}");
        }
        self.remove_btw_thread_local(thread_id).await;
    }

    async fn remove_btw_thread_local(&mut self, thread_id: ThreadId) {
        self.abort_thread_event_listener(thread_id);
        self.thread_event_channels.remove(&thread_id);
        self.agent_navigation.remove(thread_id);
        self.refresh_pending_thread_approvals().await;
        self.sync_active_agent_label();
    }
}

fn request_id_for_server_request(request: &ServerRequest) -> Option<AppServerRequestId> {
    match request {
        ServerRequest::CommandExecutionRequestApproval { request_id, .. }
        | ServerRequest::FileChangeRequestApproval { request_id, .. }
        | ServerRequest::ToolRequestUserInput { request_id, .. }
        | ServerRequest::McpServerElicitationRequest { request_id, .. }
        | ServerRequest::PermissionsRequestApproval { request_id, .. }
        | ServerRequest::DynamicToolCall { request_id, .. }
        | ServerRequest::ApplyPatchApproval { request_id, .. }
        | ServerRequest::ExecCommandApproval { request_id, .. } => Some(request_id.clone()),
        ServerRequest::ChatgptAuthTokensRefresh { .. } => None,
    }
}
