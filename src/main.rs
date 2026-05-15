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
    /// Path to the Parquet file
    file: PathBuf,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if !cli.file.exists() {
        anyhow::bail!("File not found: {}", cli.file.display());
    }

    let schema = search::read_schema(&cli.file)?;
    let columns: Vec<String> = schema.iter().map(|(name, _)| name.to_string()).collect();

    if columns.is_empty() {
        anyhow::bail!("No columns found in {}", cli.file.display());
    }

    app::run(cli.file, columns)
}
