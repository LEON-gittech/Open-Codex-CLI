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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UpgradeInfo {
    pub(crate) latest_version: String,
    pub(crate) release_notes: Vec<String>,
    pub(crate) release_notes_url: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct VersionInfo {
    latest_version: String,
    #[serde(default)]
    release_notes: Vec<String>,
    #[serde(default)]
    release_notes_url: Option<String>,
    // ISO-8601 timestamp (RFC3339)
    last_checked_at: DateTime<Utc>,
    #[serde(default)]
    dismissed_version: Option<String>,
}

#[derive(Deserialize, Debug)]
struct GithubReleaseInfo {
    html_url: Option<String>,
    body: Option<String>,
}

const VERSION_FILENAME: &str = "version.json";
const CACHE_REFRESH_INTERVAL: Duration = Duration::hours(20);
const STARTUP_UPDATE_CHECK_TIMEOUT: StdDuration = StdDuration::from_secs(3);
const RELEASE_NOTES_LIMIT: usize = 5;
const OPEN_CODEX_RELEASES_URL: &str = "https://github.com/LEON-gittech/Open-Codex-CLI/releases";
const OPEN_CODEX_RELEASE_API_URL_PREFIX: &str =
    "https://api.github.com/repos/LEON-gittech/Open-Codex-CLI/releases/tags/rust-v";

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
    upgrade_info_from_info(info).map(|info| info.latest_version)
}

fn upgrade_info_from_info(info: Option<VersionInfo>) -> Option<UpgradeInfo> {
    let info = info?;
    if !is_newer(&info.latest_version, CODEX_CLI_VERSION).unwrap_or(false) {
        return None;
    }

    Some(UpgradeInfo {
        release_notes_url: info
            .release_notes_url
            .unwrap_or_else(|| fallback_release_notes_url(&info.latest_version)),
        latest_version: info.latest_version,
        release_notes: info.release_notes,
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
    let release_notes = fetch_release_notes(&latest_version)
        .await
        .unwrap_or_else(|err| {
            tracing::error!("Failed to fetch release notes for version {latest_version}: {err}");
            ReleaseNotes {
                url: fallback_release_notes_url(&latest_version),
                notes: Vec::new(),
            }
        });

    // Preserve any previously dismissed version if present.
    let prev_info = read_version_info(version_file).ok();
    let info = VersionInfo {
        latest_version,
        release_notes: release_notes.notes,
        release_notes_url: Some(release_notes.url),
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

struct ReleaseNotes {
    url: String,
    notes: Vec<String>,
}

async fn fetch_release_notes(version: &str) -> anyhow::Result<ReleaseNotes> {
    let url = format!("{OPEN_CODEX_RELEASE_API_URL_PREFIX}{version}");
    let release = create_client()
        .get(&url)
        .header(reqwest::header::USER_AGENT, "open-codex-update-check")
        .send()
        .await?
        .error_for_status()?
        .json::<GithubReleaseInfo>()
        .await?;

    let notes = release
        .body
        .as_deref()
        .map(release_note_items_from_body)
        .unwrap_or_default();

    Ok(ReleaseNotes {
        url: release
            .html_url
            .unwrap_or_else(|| fallback_release_notes_url(version)),
        notes,
    })
}

fn fallback_release_notes_url(version: &str) -> String {
    format!("{OPEN_CODEX_RELEASES_URL}/tag/rust-v{version}")
}

fn release_note_items_from_body(body: &str) -> Vec<String> {
    body.lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            trimmed
                .strip_prefix("- ")
                .or_else(|| trimmed.strip_prefix("* "))
        })
        .map(clean_release_note_item)
        .filter(|line| !line.is_empty())
        .take(RELEASE_NOTES_LIMIT)
        .collect()
}

fn clean_release_note_item(item: &str) -> String {
    let mut cleaned = String::with_capacity(item.len());
    let mut previous_was_space = false;
    for ch in item.chars() {
        if ch == '`' {
            continue;
        }
        if ch.is_whitespace() {
            if !previous_was_space {
                cleaned.push(' ');
            }
            previous_was_space = true;
        } else {
            cleaned.push(ch);
            previous_was_space = false;
        }
    }
    cleaned.trim().to_string()
}

/// Returns the latest version to show in a popup, if it should be shown.
/// This respects the user's dismissal choice for the current latest version.
pub async fn get_upgrade_info_for_popup(config: &Config) -> Option<UpgradeInfo> {
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

    let upgrade_info = upgrade_info_from_info(info)?;
    // If the user dismissed this exact version previously, do not show the popup.
    if let Ok(info) = read_version_info(&version_file)
        && info.dismissed_version.as_deref() == Some(upgrade_info.latest_version.as_str())
    {
        return None;
    }
    Some(upgrade_info)
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

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn release_notes_body_extracts_top_bullets() {
        let body = r#"
## 0.130.11

- Fix revoke conversation-only restores so they do not touch files.
- Add inline release notes to the update prompt.
- Collapse   extra   spacing.
- Fourth item.
- Fifth item.
- Sixth item is ignored.

Details after the list are ignored.
"#;

        assert_eq!(
            release_note_items_from_body(body),
            vec![
                "Fix revoke conversation-only restores so they do not touch files.".to_string(),
                "Add inline release notes to the update prompt.".to_string(),
                "Collapse extra spacing.".to_string(),
                "Fourth item.".to_string(),
                "Fifth item.".to_string(),
            ]
        );
    }

    #[test]
    fn upgrade_info_preserves_cached_release_notes() {
        let info = VersionInfo {
            latest_version: "999.0.0".to_string(),
            release_notes: vec!["One concrete change.".to_string()],
            release_notes_url: Some("https://example.test/release".to_string()),
            last_checked_at: Utc::now(),
            dismissed_version: None,
        };

        assert_eq!(
            upgrade_info_from_info(Some(info)),
            Some(UpgradeInfo {
                latest_version: "999.0.0".to_string(),
                release_notes: vec!["One concrete change.".to_string()],
                release_notes_url: "https://example.test/release".to_string(),
            })
        );
    }
}
