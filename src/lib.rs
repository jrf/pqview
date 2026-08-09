//! Embeddable entry point for the pqview terminal application.

mod app;
mod background;
pub mod config;
mod input;
mod picker;
mod recent;
mod render;
mod search;
mod terminal_session;
mod theme;

use anyhow::Result;
use std::path::PathBuf;

pub use config::Config;

/// Options for an interactive pqview session.
#[derive(Debug, Clone, Default)]
pub struct RunOptions {
    /// Parquet file to open. When omitted, pqview starts in its file picker.
    pub file: Option<PathBuf>,
    /// Theme and theme-catalog configuration for this session.
    pub config: Config,
}

/// Runs pqview in the calling process until the user exits.
///
/// This takes ownership of the process terminal for the duration of the call.
pub fn run(options: RunOptions) -> Result<()> {
    app::run(options.file, &options.config)
}
