//! Runtime model provider resolution.
//!
//! `codex_model_provider_info` owns the config-facing provider metadata. This
//! crate turns that metadata into the narrow runtime facade that model-facing
//! callsites should depend on. The first slice is intentionally auth-only; the
//! facade can grow transport, catalog, and capability accessors as those
//! callsites move behind provider ownership.

use codex_model_provider_info::ModelProviderInfo;
use codex_protocol::config_types::ModelProviderAuthInfo;
use codex_protocol::openai_models::ModelsResponse;
use std::error::Error;
use std::fmt;

/// Stable identifier for a configured model provider.
pub type ModelProviderId = String;

/// Runtime facade for provider-owned model behavior.
///
/// This trait starts with only auth-facing accessors. Add model listing,
/// Responses client construction, and optional specialized clients here as
/// those callsites move behind provider ownership.
pub trait ModelProvider {
    fn id(&self) -> &str;
    fn info(&self) -> &ModelProviderInfo;
    fn auth_strategy(&self) -> &ProviderAuthStrategy;
    fn model_catalog(&self) -> &ProviderModelCatalog;
}

/// Auth strategy selected for a resolved model provider.
#[derive(Debug, Clone, PartialEq)]
pub enum ProviderAuthStrategy {
    /// OpenAI-managed auth through API key, ChatGPT, or ChatGPT auth tokens.
    OpenAi,
    /// Bearer token read from an environment variable.
    EnvBearer {
        env_key: String,
        env_key_instructions: Option<String>,
    },
    /// Bearer token embedded directly in provider config.
    ExperimentalBearer { token: String },
    /// Bearer token produced by an external command.
    ExternalBearer { config: ModelProviderAuthInfo },
    /// No provider-specific auth is configured; callers may use session auth fallback.
    NoProviderAuth,
}

impl ProviderAuthStrategy {
    /// Whether this auth strategy uses OpenAI account/API-key auth flows.
    pub fn requires_openai_auth(&self) -> bool {
        matches!(self, Self::OpenAi)
    }
}

/// Options used when resolving config-facing provider metadata.
///
/// Most provider behavior is derived from `ModelProviderInfo`, but callers may
/// provide an authoritative model catalog from config.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProviderResolutionOptions {
    pub model_catalog: Option<ModelsResponse>,
}

/// Model catalog source selected for a resolved provider.
#[derive(Debug, Clone, PartialEq)]
pub enum ProviderModelCatalog {
    /// Start from the bundled catalog and preserve the current cache/remote refresh behavior.
    BundledWithOptionalRemoteRefresh,
    /// Use a caller-provided catalog as authoritative.
    Static { models: ModelsResponse },
}

impl ProviderModelCatalog {
    /// Return the authoritative model list for static catalogs.
    pub fn static_models(&self) -> Option<&ModelsResponse> {
        match self {
            Self::Static { models } => Some(models),
            Self::BundledWithOptionalRemoteRefresh => None,
        }
    }

    /// Return the remote refresh policy for this catalog.
    pub fn remote_refresh_policy(&self) -> RemoteModelRefreshPolicy {
        match self {
            Self::Static { .. } => RemoteModelRefreshPolicy::Disabled,
            Self::BundledWithOptionalRemoteRefresh => RemoteModelRefreshPolicy::ExistingAuthGated,
        }
    }
}

/// Whether a catalog source permits remote refresh.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteModelRefreshPolicy {
    /// Never refresh this catalog from remote or cache.
    Disabled,
    /// Use the existing auth-gated refresh policy from `codex-models-manager`.
    ExistingAuthGated,
}

/// Resolved runtime provider facade.
///
/// This type starts as an auth-only facade. Future provider-owned behavior
/// should be added as methods/fields on this type rather than by teaching
/// callsites to branch on provider IDs.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedModelProvider {
    id: ModelProviderId,
    info: ModelProviderInfo,
    auth: ProviderAuthStrategy,
    model_catalog: ProviderModelCatalog,
}

impl ResolvedModelProvider {
    /// Resolve config-facing provider metadata into the runtime provider facade.
    pub fn resolve(
        id: impl Into<ModelProviderId>,
        info: ModelProviderInfo,
    ) -> Result<Self, ResolveProviderError> {
        Self::resolve_with_options(id, info, ProviderResolutionOptions::default())
    }

    /// Resolve provider metadata with runtime options supplied by config loading.
    pub fn resolve_with_options(
        id: impl Into<ModelProviderId>,
        info: ModelProviderInfo,
        options: ProviderResolutionOptions,
    ) -> Result<Self, ResolveProviderError> {
        info.validate()
            .map_err(ResolveProviderError::InvalidConfig)?;
        let auth = resolve_auth(&info)?;
        let model_catalog = resolve_model_catalog(options);
        Ok(Self {
            id: id.into(),
            info,
            auth,
            model_catalog,
        })
    }

    pub fn id(&self) -> &str {
        self.id.as_str()
    }

    pub fn info(&self) -> &ModelProviderInfo {
        &self.info
    }

    /// Return the provider-owned auth strategy.
    pub fn auth_strategy(&self) -> &ProviderAuthStrategy {
        &self.auth
    }

    /// Return the provider-owned model catalog strategy.
    pub fn model_catalog(&self) -> &ProviderModelCatalog {
        &self.model_catalog
    }
}

impl ModelProvider for ResolvedModelProvider {
    fn id(&self) -> &str {
        self.id.as_str()
    }

    fn info(&self) -> &ModelProviderInfo {
        &self.info
    }

    fn auth_strategy(&self) -> &ProviderAuthStrategy {
        &self.auth
    }

    fn model_catalog(&self) -> &ProviderModelCatalog {
        &self.model_catalog
    }
}

fn resolve_auth(info: &ModelProviderInfo) -> Result<ProviderAuthStrategy, ResolveProviderError> {
    if let Some(config) = info.auth.as_ref() {
        return Ok(ProviderAuthStrategy::ExternalBearer {
            config: config.clone(),
        });
    }

    if let Some(env_key) = info.env_key.as_ref() {
        return Ok(ProviderAuthStrategy::EnvBearer {
            env_key: env_key.clone(),
            env_key_instructions: info.env_key_instructions.clone(),
        });
    }

    if let Some(token) = info.experimental_bearer_token.as_ref() {
        return Ok(ProviderAuthStrategy::ExperimentalBearer {
            token: token.clone(),
        });
    }

    if info.requires_openai_auth {
        Ok(ProviderAuthStrategy::OpenAi)
    } else {
        Ok(ProviderAuthStrategy::NoProviderAuth)
    }
}

fn resolve_model_catalog(options: ProviderResolutionOptions) -> ProviderModelCatalog {
    match options.model_catalog {
        Some(models) => ProviderModelCatalog::Static { models },
        None => ProviderModelCatalog::BundledWithOptionalRemoteRefresh,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveProviderError {
    InvalidConfig(String),
}

impl fmt::Display for ResolveProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfig(message) => write!(f, "invalid provider config: {message}"),
        }
    }
}

impl Error for ResolveProviderError {}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_model_provider_info::ModelProviderInfo;
    use codex_model_provider_info::WireApi;
    use codex_protocol::openai_models::ModelsResponse;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;
    use std::num::NonZeroU64;

    fn provider() -> ModelProviderInfo {
        ModelProviderInfo {
            name: "Test Provider".to_string(),
            base_url: Some("https://example.com/v1".to_string()),
            env_key: None,
            env_key_instructions: None,
            experimental_bearer_token: None,
            auth: None,
            wire_api: WireApi::Responses,
            query_params: None,
            http_headers: None,
            env_http_headers: None,
            request_max_retries: None,
            stream_max_retries: None,
            stream_idle_timeout_ms: None,
            websocket_connect_timeout_ms: None,
            requires_openai_auth: false,
            supports_websockets: false,
        }
    }

    #[test]
    fn resolves_openai_auth() {
        let info = ModelProviderInfo::create_openai_provider(/*base_url*/ None);

        let provider = ResolvedModelProvider::resolve("openai", info.clone()).unwrap();

        assert_eq!(provider.id(), "openai");
        assert_eq!(provider.info(), &info);
        assert_eq!(provider.auth_strategy(), &ProviderAuthStrategy::OpenAi);
        assert_eq!(
            provider.model_catalog(),
            &ProviderModelCatalog::BundledWithOptionalRemoteRefresh
        );
        assert!(provider.auth_strategy().requires_openai_auth());
    }

    #[test]
    fn resolved_provider_implements_model_provider_facade() {
        fn auth_from_provider(provider: &impl ModelProvider) -> &ProviderAuthStrategy {
            provider.auth_strategy()
        }

        let provider = ResolvedModelProvider::resolve(
            "openai",
            ModelProviderInfo::create_openai_provider(/*base_url*/ None),
        )
        .unwrap();

        assert_eq!(auth_from_provider(&provider), &ProviderAuthStrategy::OpenAi);
    }

    #[test]
    fn resolved_provider_implements_model_catalog_facade() {
        fn catalog_from_provider(provider: &impl ModelProvider) -> &ProviderModelCatalog {
            provider.model_catalog()
        }

        let provider = ResolvedModelProvider::resolve(
            "openai",
            ModelProviderInfo::create_openai_provider(/*base_url*/ None),
        )
        .unwrap();

        assert_eq!(
            catalog_from_provider(&provider),
            &ProviderModelCatalog::BundledWithOptionalRemoteRefresh
        );
    }

    #[test]
    fn resolves_env_bearer_auth() {
        let mut info = provider();
        info.env_key = Some("TEST_API_KEY".to_string());
        info.env_key_instructions = Some("Set TEST_API_KEY.".to_string());

        let provider = ResolvedModelProvider::resolve("custom", info).unwrap();

        assert_eq!(
            provider.auth_strategy(),
            &ProviderAuthStrategy::EnvBearer {
                env_key: "TEST_API_KEY".to_string(),
                env_key_instructions: Some("Set TEST_API_KEY.".to_string()),
            }
        );
    }

    #[test]
    fn resolves_legacy_auth_priority_for_non_command_auth_fields() {
        let mut env_over_experimental = provider();
        env_over_experimental.env_key = Some("TEST_API_KEY".to_string());
        env_over_experimental.experimental_bearer_token = Some("token".to_string());
        assert_eq!(
            ResolvedModelProvider::resolve("custom", env_over_experimental)
                .unwrap()
                .auth_strategy(),
            &ProviderAuthStrategy::EnvBearer {
                env_key: "TEST_API_KEY".to_string(),
                env_key_instructions: None,
            }
        );

        let mut env_over_openai = provider();
        env_over_openai.env_key = Some("TEST_API_KEY".to_string());
        env_over_openai.requires_openai_auth = true;
        assert_eq!(
            ResolvedModelProvider::resolve("custom", env_over_openai)
                .unwrap()
                .auth_strategy(),
            &ProviderAuthStrategy::EnvBearer {
                env_key: "TEST_API_KEY".to_string(),
                env_key_instructions: None,
            }
        );

        let mut experimental_over_openai = provider();
        experimental_over_openai.experimental_bearer_token = Some("token".to_string());
        experimental_over_openai.requires_openai_auth = true;
        assert_eq!(
            ResolvedModelProvider::resolve("custom", experimental_over_openai)
                .unwrap()
                .auth_strategy(),
            &ProviderAuthStrategy::ExperimentalBearer {
                token: "token".to_string(),
            }
        );
    }

    #[test]
    fn resolves_experimental_bearer_auth() {
        let mut info = provider();
        info.experimental_bearer_token = Some("token".to_string());

        let provider = ResolvedModelProvider::resolve("custom", info).unwrap();

        assert_eq!(
            provider.auth_strategy(),
            &ProviderAuthStrategy::ExperimentalBearer {
                token: "token".to_string(),
            }
        );
    }

    #[test]
    fn resolves_external_bearer_auth() {
        let mut info = provider();
        let auth_config = ModelProviderAuthInfo {
            command: "credential-helper".to_string(),
            args: vec!["token".to_string()],
            timeout_ms: NonZeroU64::new(10_000).unwrap(),
            refresh_interval_ms: 300_000,
            cwd: AbsolutePathBuf::from_absolute_path("/tmp").unwrap(),
        };
        info.auth = Some(auth_config.clone());

        let provider = ResolvedModelProvider::resolve("custom", info).unwrap();

        assert_eq!(
            provider.auth_strategy(),
            &ProviderAuthStrategy::ExternalBearer {
                config: auth_config,
            }
        );
    }

    #[test]
    fn resolves_no_auth_for_custom_provider() {
        let provider = ResolvedModelProvider::resolve("custom", provider()).unwrap();

        assert_eq!(
            provider.auth_strategy(),
            &ProviderAuthStrategy::NoProviderAuth
        );
        assert!(!provider.auth_strategy().requires_openai_auth());
    }

    #[test]
    fn resolves_static_model_catalog_from_options() {
        let models = ModelsResponse { models: Vec::new() };

        let provider = ResolvedModelProvider::resolve_with_options(
            "custom",
            provider(),
            ProviderResolutionOptions {
                model_catalog: Some(models.clone()),
            },
        )
        .unwrap();

        assert_eq!(
            provider.model_catalog(),
            &ProviderModelCatalog::Static { models }
        );
        assert_eq!(
            provider.model_catalog().remote_refresh_policy(),
            RemoteModelRefreshPolicy::Disabled
        );
    }

    #[test]
    fn default_catalog_allows_existing_auth_gated_refresh() {
        let provider = ResolvedModelProvider::resolve("custom", provider()).unwrap();

        assert_eq!(
            provider.model_catalog().remote_refresh_policy(),
            RemoteModelRefreshPolicy::ExistingAuthGated
        );
        assert_eq!(provider.model_catalog().static_models(), None);
    }

    #[test]
    fn rejects_invalid_command_auth() {
        let mut info = provider();
        info.auth = Some(ModelProviderAuthInfo {
            command: " ".to_string(),
            args: Vec::new(),
            timeout_ms: NonZeroU64::new(10_000).unwrap(),
            refresh_interval_ms: 300_000,
            cwd: AbsolutePathBuf::from_absolute_path("/tmp").unwrap(),
        });

        let err = ResolvedModelProvider::resolve("custom", info).unwrap_err();

        assert_eq!(
            err,
            ResolveProviderError::InvalidConfig(
                "provider auth.command must not be empty".to_string()
            )
        );
    }
}
