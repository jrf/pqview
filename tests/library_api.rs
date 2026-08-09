use pqview::{Config, RunOptions};

#[test]
fn public_api_supports_caller_supplied_options() {
    let options = RunOptions {
        file: Some("/synthetic/example.parquet".into()),
        config: Config::default(),
    };
    let run: fn(RunOptions) -> anyhow::Result<()> = pqview::run;

    assert_eq!(
        options.file.as_deref(),
        Some(std::path::Path::new("/synthetic/example.parquet"))
    );
    let _ = run;
}
