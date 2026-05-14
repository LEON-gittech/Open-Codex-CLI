use super::BackgroundActivity;
use super::ChatWidget;
use crate::multi_agents;
use codex_app_server_protocol::CollabAgentState;
use codex_app_server_protocol::CollabAgentStatus;
use codex_app_server_protocol::CollabAgentTool;
use codex_app_server_protocol::CollabAgentToolCallStatus;
use codex_app_server_protocol::ThreadItem;
use codex_protocol::ThreadId;
use std::collections::HashSet;
use std::time::Instant;

pub(super) fn sync_collab_agent_background_activity(chat: &mut ChatWidget, item: &ThreadItem) {
    let ThreadItem::CollabAgentToolCall {
        tool,
        status,
        receiver_thread_ids,
        agents_states,
        ..
    } = item
    else {
        return;
    };
    if matches!(status, CollabAgentToolCallStatus::InProgress) {
        return;
    }

    let mut changed = false;
    if matches!(tool, CollabAgentTool::CloseAgent) {
        for thread_id in receiver_thread_ids {
            if let Ok(thread_id) = ThreadId::from_string(thread_id) {
                changed |= remove_collab_agent_background_activity(chat, thread_id);
            }
        }
    } else {
        let mut seen = HashSet::new();
        for thread_id in receiver_thread_ids {
            if let Ok(parsed_thread_id) = ThreadId::from_string(thread_id) {
                seen.insert(parsed_thread_id);
                if let Some(state) = agents_states.get(thread_id) {
                    changed |=
                        sync_collab_agent_activity_state(chat, item, parsed_thread_id, state);
                }
            }
        }
        for (thread_id, state) in agents_states {
            if let Ok(parsed_thread_id) = ThreadId::from_string(thread_id)
                && seen.insert(parsed_thread_id)
            {
                changed |= sync_collab_agent_activity_state(chat, item, parsed_thread_id, state);
            }
        }
    }

    if changed {
        refresh_task_backgrounded_from_activity_state(chat);
        chat.refresh_background_tasks_view_if_open();
        chat.request_redraw();
    }
}

fn sync_collab_agent_activity_state(
    chat: &mut ChatWidget,
    item: &ThreadItem,
    thread_id: ThreadId,
    state: &CollabAgentState,
) -> bool {
    let is_live = matches!(
        state.status,
        CollabAgentStatus::PendingInit
            | CollabAgentStatus::Running
            | CollabAgentStatus::Interrupted
    );
    let active_matches = chat
        .active_cell
        .as_ref()
        .and_then(|cell| {
            cell.as_any()
                .downcast_ref::<multi_agents::CollabAgentActivityCell>()
        })
        .is_some_and(|cell| cell.thread_id() == thread_id);
    if active_matches {
        if is_live {
            let metadata = chat.collab_agent_metadata(thread_id);
            if let Some(cell) = chat.active_cell.as_mut().and_then(|cell| {
                cell.as_any_mut()
                    .downcast_mut::<multi_agents::CollabAgentActivityCell>()
            }) {
                cell.update(state, &metadata);
            }
        } else {
            chat.active_cell = None;
        }
        chat.bump_active_cell_revision();
        return true;
    }

    if is_live {
        let metadata = chat.collab_agent_metadata(thread_id);
        for activity in &mut chat.background_activities {
            if let Some(cell) = activity
                .cell
                .as_any_mut()
                .downcast_mut::<multi_agents::CollabAgentActivityCell>()
                && cell.thread_id() == thread_id
            {
                cell.update(state, &metadata);
                if activity.task.is_none() {
                    activity.task = prompt_for_thread(item, thread_id);
                }
                return true;
            }
        }

        chat.background_activities.push_back(BackgroundActivity {
            cell: Box::new(multi_agents::CollabAgentActivityCell::new(
                thread_id, state, &metadata,
            )),
            started_at: Instant::now(),
            task: prompt_for_thread(item, thread_id),
        });
        true
    } else {
        notify_collab_agent_background_completion(chat, thread_id, state);
        remove_collab_agent_background_activity(chat, thread_id)
    }
}

fn prompt_for_thread(item: &ThreadItem, thread_id: ThreadId) -> Option<String> {
    let ThreadItem::CollabAgentToolCall {
        tool,
        prompt,
        receiver_thread_ids,
        ..
    } = item
    else {
        return None;
    };
    if !matches!(tool, CollabAgentTool::SpawnAgent) {
        return None;
    }
    let targets_thread = receiver_thread_ids
        .iter()
        .filter_map(|id| ThreadId::from_string(id).ok())
        .any(|id| id == thread_id);
    if targets_thread { prompt.clone() } else { None }
}

fn remove_collab_agent_background_activity(chat: &mut ChatWidget, thread_id: ThreadId) -> bool {
    if let Some(index) = chat.background_activities.iter().position(|activity| {
        activity
            .cell
            .as_any()
            .downcast_ref::<multi_agents::CollabAgentActivityCell>()
            .is_some_and(|cell| cell.thread_id() == thread_id)
    }) {
        chat.background_activities.remove(index);
        true
    } else {
        false
    }
}

fn notify_collab_agent_background_completion(
    chat: &mut ChatWidget,
    thread_id: ThreadId,
    state: &CollabAgentState,
) {
    let Some(title) = chat.background_activities.iter().find_map(|activity| {
        activity
            .cell
            .as_any()
            .downcast_ref::<multi_agents::CollabAgentActivityCell>()
            .filter(|cell| cell.thread_id() == thread_id)
            .map(|_| {
                let lines = activity.cell.display_lines(u16::MAX);
                let raw_title = lines
                    .first()
                    .map(super::line_to_plain_string)
                    .unwrap_or_else(|| format!("Subagent {thread_id}"));
                super::split_background_task_status(raw_title).0
            })
    }) else {
        return;
    };

    let message = match state.status {
        CollabAgentStatus::Completed => "completed",
        CollabAgentStatus::Interrupted => "interrupted",
        CollabAgentStatus::Errored => "failed",
        CollabAgentStatus::Shutdown => "closed",
        CollabAgentStatus::NotFound => "not found",
        CollabAgentStatus::PendingInit | CollabAgentStatus::Running => return,
    };
    let mut notification = format!("{title} {message}.");
    if let Some(summary) = state
        .message
        .as_deref()
        .map(str::trim)
        .filter(|summary| !summary.is_empty())
    {
        notification.push(' ');
        notification.push_str(summary);
    }
    chat.add_info_message(notification, /*hint*/ None);
}

fn refresh_task_backgrounded_from_activity_state(chat: &mut ChatWidget) {
    chat.task_backgrounded = chat.should_mark_current_turn_backgrounded();
    chat.update_task_running_state();
}
