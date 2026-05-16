mod app;
mod render;
mod search;
mod theme;

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "pqview", about = "Search through large Parquet files")]
struct Cli {
    /// Path to the Parquet file (optional — opens a file picker if omitted)
    file: Option<PathBuf>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if let Some(path) = &cli.file {
        if !path.exists() {
            anyhow::bail!("File not found: {}", path.display());
        }
    }

    app::run(cli.file)
}
