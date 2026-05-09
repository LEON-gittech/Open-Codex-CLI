#![cfg(not(debug_assertions))]

use crate::legacy_core::config::Config;
use crate::npm_registry;
use crate::npm_registry::NpmPackageInfo;
use crate::update_action;
use crate::update_action::UpdateAction;
use crate::update_versions::is_newer;
use crate::update_versions::is_source_build_version;
use crate::update_versions::should_refresh_update_cache;
use chrono::DateTime;
use chrono::Duration;
use chrono::Utc;
use codex_login::default_client::create_client;
use serde::Deserialize;
use serde::Serialize;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration as StdDuration;

use crate::version::CODEX_CLI_VERSION;

pub fn get_upgrade_version(config: &Config) -> Option<String> {
    if !config.check_for_update_on_startup || is_source_build_version(CODEX_CLI_VERSION) {
        return None;
    }

    let action = update_action::get_update_action();
    let version_file = version_filepath(config);
    let info = read_version_info(&version_file).ok();

    if should_refresh_version_info(info.as_ref(), Utc::now()) {
        // Refresh the cached latest version in the background so TUI startup
        // isn’t blocked by a network call. The UI reads the previously cached
        // value (if any) for this run; the next run shows the banner if needed.
        tokio::spawn(async move {
            check_for_update(&version_file, action)
                .await
                .inspect_err(|e| tracing::error!("Failed to update version: {e}"))
        });
    }

    upgrade_version_from_info(info)
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct VersionInfo {
    latest_version: String,
    // ISO-8601 timestamp (RFC3339)
    last_checked_at: DateTime<Utc>,
    #[serde(default)]
    dismissed_version: Option<String>,
}

const VERSION_FILENAME: &str = "version.json";
const CACHE_REFRESH_INTERVAL: Duration = Duration::hours(20);
const STARTUP_UPDATE_CHECK_TIMEOUT: StdDuration = StdDuration::from_secs(3);

fn version_filepath(config: &Config) -> PathBuf {
    config.codex_home.join(VERSION_FILENAME).into_path_buf()
}

fn read_version_info(version_file: &Path) -> anyhow::Result<VersionInfo> {
    let contents = std::fs::read_to_string(version_file)?;
    Ok(serde_json::from_str(&contents)?)
}

fn should_refresh_version_info(info: Option<&VersionInfo>, now: DateTime<Utc>) -> bool {
    let cache_is_stale = info
        .map(|info| info.last_checked_at < now - CACHE_REFRESH_INTERVAL)
        .unwrap_or(true);
    let cached_latest = info.map(|info| info.latest_version.as_str());
    should_refresh_update_cache(cached_latest, cache_is_stale, CODEX_CLI_VERSION)
}

fn upgrade_version_from_info(info: Option<VersionInfo>) -> Option<String> {
    info.and_then(|info| {
        if is_newer(&info.latest_version, CODEX_CLI_VERSION).unwrap_or(false) {
            Some(info.latest_version)
        } else {
            None
        }
    })
}

async fn check_for_update(
    version_file: &Path,
    _action: Option<UpdateAction>,
) -> anyhow::Result<()> {
    let package_info = create_client()
        .get(npm_registry::PACKAGE_URL)
        .send()
        .await?
        .error_for_status()?
        .json::<NpmPackageInfo>()
        .await?;
    let latest_version = npm_registry::latest_version(&package_info)?.to_string();

    // Preserve any previously dismissed version if present.
    let prev_info = read_version_info(version_file).ok();
    let info = VersionInfo {
        latest_version,
        last_checked_at: Utc::now(),
        dismissed_version: prev_info.and_then(|p| p.dismissed_version),
    };

    let json_line = format!("{}\n", serde_json::to_string(&info)?);
    if let Some(parent) = version_file.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(version_file, json_line).await?;
    Ok(())
}

/// Returns the latest version to show in a popup, if it should be shown.
/// This respects the user's dismissal choice for the current latest version.
pub async fn get_upgrade_version_for_popup(config: &Config) -> Option<String> {
    if !config.check_for_update_on_startup || is_source_build_version(CODEX_CLI_VERSION) {
        return None;
    }

    let version_file = version_filepath(config);
    let mut info = read_version_info(&version_file).ok();
    if should_refresh_version_info(info.as_ref(), Utc::now()) {
        let action = update_action::get_update_action();
        match tokio::time::timeout(
            STARTUP_UPDATE_CHECK_TIMEOUT,
            check_for_update(&version_file, action),
        )
        .await
        {
            Ok(Ok(())) => {
                info = read_version_info(&version_file).ok();
            }
            Ok(Err(err)) => {
                tracing::error!("Failed to update version before startup prompt: {err}");
            }
            Err(_) => {
                tracing::error!("Timed out updating version before startup prompt");
            }
        }
    }

    let latest = upgrade_version_from_info(info)?;
    // If the user dismissed this exact version previously, do not show the popup.
    if let Ok(info) = read_version_info(&version_file)
        && info.dismissed_version.as_deref() == Some(latest.as_str())
    {
        return None;
    }
    Some(latest)
}

/// Persist a dismissal for the current latest version so we don't show
/// the update popup again for this version.
pub async fn dismiss_version(config: &Config, version: &str) -> anyhow::Result<()> {
    let version_file = version_filepath(config);
    let mut info = match read_version_info(&version_file) {
        Ok(info) => info,
        Err(_) => return Ok(()),
    };
    info.dismissed_version = Some(version.to_string());
    let json_line = format!("{}\n", serde_json::to_string(&info)?);
    if let Some(parent) = version_file.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(version_file, json_line).await?;
    Ok(())
}
