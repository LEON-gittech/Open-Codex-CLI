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
                    changed |= sync_collab_agent_activity_state(chat, parsed_thread_id, state);
                }
            }
        }
        for (thread_id, state) in agents_states {
            if let Ok(parsed_thread_id) = ThreadId::from_string(thread_id)
                && seen.insert(parsed_thread_id)
            {
                changed |= sync_collab_agent_activity_state(chat, parsed_thread_id, state);
            }
        }
    }

    if changed {
        refresh_task_backgrounded_from_activity_state(chat);
        chat.request_redraw();
    }
}

fn sync_collab_agent_activity_state(
    chat: &mut ChatWidget,
    thread_id: ThreadId,
    state: &CollabAgentState,
) -> bool {
    let is_live = matches!(
        state.status,
        CollabAgentStatus::PendingInit | CollabAgentStatus::Running
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
                return true;
            }
        }

        let id = chat.next_background_activity_id;
        chat.next_background_activity_id = chat.next_background_activity_id.wrapping_add(1);
        chat.background_activities.push_back(BackgroundActivity {
            id,
            cell: Box::new(multi_agents::CollabAgentActivityCell::new(
                thread_id, state, &metadata,
            )),
        });
        true
    } else {
        remove_collab_agent_background_activity(chat, thread_id)
    }
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

fn refresh_task_backgrounded_from_activity_state(chat: &mut ChatWidget) {
    chat.task_backgrounded = chat.agent_turn_running
        && chat.active_cell.is_none()
        && !chat.background_activities.is_empty();
    chat.update_task_running_state();
}
