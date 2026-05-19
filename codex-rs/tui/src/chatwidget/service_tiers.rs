//! Service-tier selection and model-catalog helpers for `ChatWidget`.

use super::ChatWidget;
use crate::app_command::AppCommand;
use crate::app_event::AppEvent;
use crate::bottom_pane::slash_commands::ServiceTierCommand;
use codex_features::Feature;
use codex_protocol::config_types::ServiceTier;
use codex_protocol::openai_models::ReasoningEffort as ReasoningEffortConfig;
use codex_protocol::openai_models::SPEED_TIER_FAST;

impl ChatWidget {
    pub(crate) fn set_service_tier(&mut self, service_tier: Option<String>) {
        self.config.service_tier = service_tier.clone();
        self.effective_service_tier = service_tier;
        self.refresh_model_dependent_surfaces();
    }

    pub(crate) fn current_service_tier(&self) -> Option<&str> {
        self.effective_service_tier.as_deref()
    }

    pub(crate) fn configured_service_tier(&self) -> Option<String> {
        self.config.service_tier.clone()
    }

    pub(crate) fn fast_default_opt_out(&self) -> Option<bool> {
        self.config.notices.fast_default_opt_out
    }

    pub(crate) fn should_show_fast_status(&self, model: &str, service_tier: Option<&str>) -> bool {
        service_tier.is_some_and(|service_tier| {
            ServiceTier::from_request_value(service_tier) == Some(ServiceTier::Fast)
        }) && self.model_supports_fast_mode(model)
            && self.has_chatgpt_account
    }

    fn model_supports_fast_mode(&self, model: &str) -> bool {
        self.model_catalog
            .try_list_models()
            .ok()
            .and_then(|models| {
                models
                    .into_iter()
                    .find(|preset| preset.model == model)
                    .map(|preset| preset.supports_fast_mode())
            })
            .unwrap_or(false)
    }

    pub(super) fn fast_mode_enabled(&self) -> bool {
        self.config.features.enabled(Feature::FastMode)
    }

    pub(crate) fn can_toggle_fast_mode_from_keybinding(&self) -> bool {
        self.fast_mode_enabled()
            && self.current_model_fast_service_tier().is_some()
            && !self.is_user_turn_pending_or_running()
            && self.bottom_pane.no_modal_or_popup_active()
    }

    pub(crate) fn toggle_fast_mode_from_ui(&mut self) {
        let Some(fast_tier) = self.current_model_fast_service_tier() else {
            return;
        };
        let next_tier = if self.current_service_tier() == Some(fast_tier.id.as_str()) {
            None
        } else {
            Some(fast_tier.id)
        };
        self.set_service_tier_selection(next_tier);
    }

    pub(crate) fn toggle_service_tier_from_ui(&mut self, command: ServiceTierCommand) {
        let next_tier = if self.current_service_tier() == Some(command.id.as_str()) {
            None
        } else {
            Some(command.id)
        };
        self.set_service_tier_selection(next_tier);
    }

    pub(super) fn sync_service_tier_commands(&mut self) {
        self.bottom_pane
            .set_service_tier_commands_enabled(self.fast_mode_enabled());
        self.bottom_pane
            .set_service_tier_commands(self.current_model_service_tier_commands());
        self.sync_shift_tab_reasoning_speed_toggle_enabled();
    }

    pub(super) fn current_model_service_tier_commands(&self) -> Vec<ServiceTierCommand> {
        let model = self.current_model();
        self.model_catalog
            .try_list_models()
            .ok()
            .and_then(|models| {
                models
                    .into_iter()
                    .find(|preset| preset.model == model)
                    .map(|preset| {
                        preset
                            .service_tiers
                            .into_iter()
                            .map(|tier| ServiceTierCommand {
                                id: tier.id,
                                name: tier.name.to_lowercase(),
                                description: tier.description,
                            })
                            .collect()
                    })
            })
            .unwrap_or_default()
    }

    pub(super) fn set_service_tier_selection(&mut self, service_tier: Option<String>) {
        if service_tier.is_none() {
            self.config.notices.fast_default_opt_out = Some(true);
        }
        self.set_service_tier(service_tier.clone());
        self.app_event_tx
            .send(AppEvent::CodexOp(AppCommand::override_turn_context(
                /*cwd*/ None,
                /*approval_policy*/ None,
                /*approvals_reviewer*/ None,
                /*permission_profile*/ None,
                /*windows_sandbox_level*/ None,
                /*model*/ None,
                /*effort*/ None,
                /*summary*/ None,
                Some(service_tier.clone()),
                /*collaboration_mode*/ None,
                /*personality*/ None,
            )));
        self.app_event_tx
            .send(AppEvent::PersistServiceTierSelection { service_tier });
    }

    fn model_supports_service_tier(&self, model: &str, service_tier: &str) -> bool {
        self.model_catalog
            .try_list_models()
            .ok()
            .and_then(|models| {
                models
                    .into_iter()
                    .find(|preset| preset.model == model)
                    .map(|preset| {
                        preset
                            .service_tiers
                            .iter()
                            .any(|tier| tier.id == service_tier)
                    })
            })
            .unwrap_or(false)
    }

    fn current_model_fast_service_tier(&self) -> Option<ServiceTierCommand> {
        self.current_model_service_tier_commands()
            .into_iter()
            .find(|tier| tier.name.eq_ignore_ascii_case(SPEED_TIER_FAST))
    }

    pub(crate) fn can_toggle_reasoning_speed_mode_from_keybinding(&self) -> bool {
        self.can_toggle_fast_mode_from_keybinding()
            && self.current_model_fast_service_tier().is_some()
    }

    pub(crate) fn toggle_reasoning_speed_mode_from_ui(&mut self) {
        let current_model = self.current_model().to_string();
        let fast_value = ServiceTier::Fast.request_value();
        let is_high_fast = self
            .effective_reasoning_effort()
            .is_some_and(|e| e == ReasoningEffortConfig::High)
            && self.current_service_tier() == Some(fast_value);
        let (next_effort, next_tier) = if is_high_fast {
            (ReasoningEffortConfig::XHigh, None)
        } else {
            (ReasoningEffortConfig::High, Some(fast_value.to_string()))
        };

        if next_tier.is_none() {
            self.config.notices.fast_default_opt_out = Some(true);
        }
        self.set_reasoning_effort(Some(next_effort));
        self.set_service_tier(next_tier.clone());
        self.app_event_tx
            .send(AppEvent::CodexOp(AppCommand::override_turn_context(
                /*cwd*/ None,
                /*approval_policy*/ None,
                /*approvals_reviewer*/ None,
                /*permission_profile*/ None,
                /*windows_sandbox_level*/ None,
                /*model*/ None,
                Some(Some(next_effort)),
                /*summary*/ None,
                Some(next_tier.clone()),
                /*collaboration_mode*/ None,
                /*personality*/ None,
            )));
        self.app_event_tx
            .send(AppEvent::UpdateReasoningEffort(Some(next_effort)));
        self.app_event_tx.send(AppEvent::PersistModelSelection {
            model: current_model,
            effort: Some(next_effort),
        });
        self.app_event_tx
            .send(AppEvent::PersistServiceTierSelection {
                service_tier: next_tier,
            });
    }

    pub(super) fn sync_shift_tab_reasoning_speed_toggle_enabled(&mut self) {
        let enabled = self.fast_mode_enabled() && self.current_model_fast_service_tier().is_some();
        self.bottom_pane
            .set_shift_tab_reasoning_speed_toggle_enabled(enabled);
    }
}
