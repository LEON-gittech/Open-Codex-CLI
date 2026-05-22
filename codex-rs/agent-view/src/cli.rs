use std::path::PathBuf;

use clap::Parser;
use codex_utils_cli::CliConfigOverrides;

const DEFAULT_LIMIT: usize = 50;

#[derive(Parser, Debug, Default)]
#[command(version)]
pub struct Cli {
    #[clap(skip)]
    pub config_overrides: CliConfigOverrides,

    /// Override the Codex home directory used to discover session rollout files.
    #[arg(long = "codex-home", value_name = "DIR")]
    pub codex_home: Option<PathBuf>,

    /// Maximum number of sessions to load (newest first).
    #[arg(long = "limit", default_value_t = DEFAULT_LIMIT, value_name = "N")]
    pub limit: usize,

    /// Print discovered sessions as JSON and exit instead of opening the TUI.
    #[arg(long = "json", default_value_t = false)]
    pub json: bool,
}
