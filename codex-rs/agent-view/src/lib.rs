//! `codex agents` — an offline viewer for past Codex sessions.
//!
//! Read-only: walks `$CODEX_HOME/sessions/` rollout files, summarises each one,
//! and renders a Ratatui list with a peek panel. Does not connect to a daemon
//! or attach to live sessions; that comes in a later PR once a session
//! supervisor exists.

use std::io::IsTerminal as _;

use anyhow::Context as _;
use tracing_subscriber::EnvFilter;

pub mod cli;
mod peek;
mod scanner;
mod view;

pub use cli::Cli;
pub use peek::PeekContent;
pub use peek::load_peek;
pub use scanner::SessionStatus;
pub use scanner::SessionSummary;
pub use scanner::list_sessions;

/// Entry point for the `codex agents` subcommand.
pub async fn run_main(cli: Cli) -> anyhow::Result<()> {
    init_tracing();

    let codex_home = match cli.codex_home.clone() {
        Some(path) => path,
        None => codex_utils_home_dir::find_codex_home()
            .context("failed to resolve CODEX_HOME")?
            .into(),
    };

    let sessions = list_sessions(&codex_home, cli.limit)
        .await
        .context("failed to scan sessions")?;

    if cli.json {
        let stdout = std::io::stdout().lock();
        serde_json::to_writer_pretty(stdout, &sessions)
            .context("failed to serialise sessions as JSON")?;
        println!();
        return Ok(());
    }

    view::run_tui(sessions, codex_home).await
}

fn init_tracing() {
    let default_level = "error";
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .or_else(|_| EnvFilter::try_new(default_level))
                .unwrap_or_else(|_| EnvFilter::new(default_level)),
        )
        .with_ansi(std::io::stderr().is_terminal())
        .with_writer(std::io::stderr)
        .try_init();
}
