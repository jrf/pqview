use anyhow::Result;
use clap::Parser;
use pqview::{Config, RunOptions};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "pqview", version, about = "Search through large Parquet files")]
struct Cli {
    /// Path to the Parquet file (optional — opens a file picker if omitted)
    file: Option<PathBuf>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if let Some(path) = &cli.file
        && !path.exists()
    {
        anyhow::bail!("File not found: {}", path.display());
    }

    pqview::run(RunOptions {
        file: cli.file,
        config: Config::load(),
    })
}
