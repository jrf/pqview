use anyhow::{Result, bail};
use polars::prelude::*;
use std::collections::{HashMap, HashSet};
use std::path::Path;

#[derive(Clone, Default)]
pub struct SearchCriteria {
    pub filters: HashMap<String, Vec<String>>,
    pub exclude_empty: HashSet<String>,
    pub column: Option<String>,
    pub text: String,
}

pub fn read_schema(path: &Path) -> Result<Schema> {
    let mut frame = LazyFrame::scan_parquet(path, Default::default())?;
    let schema = frame.collect_schema()?;
    Ok(schema.as_ref().clone())
}

pub struct SearchResult {
    pub rows: Vec<Vec<String>>,
    pub total_matches: usize,
}

pub fn query(
    path: &Path,
    criteria: &SearchCriteria,
    display_columns: &[String],
    limit: u32,
    offset: u32,
) -> Result<SearchResult> {
    if display_columns.is_empty() {
        bail!("At least one column must be visible");
    }

    let filtered = filtered_scan(path, criteria)?;
    let total_matches = count_matches(filtered.clone())?;
    let select_columns = display_columns.iter().map(col).collect::<Vec<_>>();
    let result = filtered
        .select(select_columns)
        .slice(i64::from(offset), limit)
        .collect()?;

    Ok(SearchResult {
        rows: dataframe_to_strings(&result, display_columns),
        total_matches,
    })
}

pub fn export(
    path: &Path,
    output: &Path,
    criteria: &SearchCriteria,
    columns: &[String],
) -> Result<usize> {
    if columns.is_empty() {
        bail!("At least one column must be visible");
    }
    if output.exists() {
        bail!("Export path already exists: {}", output.display());
    }

    let filtered = filtered_scan(path, criteria)?;
    let count = count_matches(filtered.clone())?;
    let select_columns = columns.iter().map(col).collect::<Vec<_>>();
    let output_directory = output.parent().unwrap_or_else(|| Path::new("."));
    let temporary = tempfile::Builder::new()
        .prefix(".pqview-export-")
        .suffix(".parquet")
        .tempfile_in(output_directory)?;
    filtered
        .select(select_columns)
        .sink_parquet(&temporary.path(), Default::default(), None)?;
    temporary.persist_noclobber(output)?;
    Ok(count)
}

pub fn unique_values(
    path: &Path,
    column: &str,
    other_filters: &HashMap<String, Vec<String>>,
    limit: u32,
) -> Result<Vec<String>> {
    let criteria = SearchCriteria {
        filters: other_filters.clone(),
        ..Default::default()
    };
    let frame = filtered_scan(path, &criteria)?
        .select([col(column).cast(DataType::String)])
        .unique(None, UniqueKeepStrategy::First)
        .sort([column], Default::default())
        .limit(limit)
        .collect()?;

    let series = frame.column(column)?;
    let mut values = Vec::with_capacity(series.len());
    for index in 0..series.len() {
        let value = any_value_to_string(series.get(index)?);
        if !value.is_empty() {
            values.push(value);
        }
    }
    Ok(values)
}

fn filtered_scan(path: &Path, criteria: &SearchCriteria) -> Result<LazyFrame> {
    let mut filtered = LazyFrame::scan_parquet(path, Default::default())?;

    for (column, values) in &criteria.filters {
        if values.is_empty() {
            continue;
        }
        let lowercase_values = values
            .iter()
            .map(|value| value.to_lowercase())
            .collect::<Vec<_>>();
        let series = Series::new("".into(), &lowercase_values);
        filtered = filtered.filter(
            col(column)
                .cast(DataType::String)
                .str()
                .to_lowercase()
                .is_in(lit(series)),
        );
    }

    for column in &criteria.exclude_empty {
        filtered = filtered.filter(
            col(column)
                .is_not_null()
                .and(col(column).cast(DataType::String).neq(lit(""))),
        );
    }

    if let Some(column) = &criteria.column
        && !criteria.text.is_empty()
    {
        let pattern = format!("(?i){}", regex_escape(&criteria.text));
        filtered = filtered.filter(
            col(column)
                .cast(DataType::String)
                .str()
                .contains(lit(pattern), false),
        );
    }

    Ok(filtered)
}

fn count_matches(filtered: LazyFrame) -> Result<usize> {
    let frame = filtered.select([len().alias("count")]).collect()?;
    Ok(frame.column("count")?.u32()?.get(0).unwrap_or(0) as usize)
}

fn dataframe_to_strings(frame: &DataFrame, columns: &[String]) -> Vec<Vec<String>> {
    (0..frame.height())
        .map(|row_index| {
            columns
                .iter()
                .map(|column| {
                    frame
                        .column(column)
                        .ok()
                        .and_then(|series| series.get(row_index).ok())
                        .map(any_value_to_string)
                        .unwrap_or_default()
                })
                .collect()
        })
        .collect()
}

fn any_value_to_string(value: AnyValue<'_>) -> String {
    match value {
        AnyValue::Null => String::new(),
        AnyValue::String(value) => value.to_owned(),
        AnyValue::StringOwned(value) => value.as_str().to_owned(),
        value => value.to_string(),
    }
}

fn regex_escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '\\' | '.' | '+' | '*' | '?' | '(' | ')' | '|' | '[' | ']' | '{' | '}' | '^' | '$' => {
                escaped.push('\\');
                escaped.push(character);
            }
            _ => escaped.push(character),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    fn synthetic_file() -> NamedTempFile {
        let file = NamedTempFile::with_suffix("_pqview_synthetic.parquet").unwrap();
        let record_ids = (0..100_i64).collect::<Vec<_>>();
        let descriptions = (0..100)
            .map(|index| {
                if index % 2 == 0 {
                    "alpha synthetic item"
                } else {
                    "beta synthetic item"
                }
            })
            .collect::<Vec<_>>();
        let categories = (0..100)
            .map(|index| if index % 3 == 0 { "Group A" } else { "Group B" })
            .collect::<Vec<_>>();
        let optional = (0..100)
            .map(|index| (index % 4 != 0).then_some("present"))
            .collect::<Vec<_>>();
        let mut frame = df!(
            "record_id" => record_ids,
            "description" => descriptions,
            "category" => categories,
            "optional" => optional,
        )
        .unwrap();
        let event_times = Int64Chunked::from_vec("event_time".into(), vec![0; 100])
            .into_datetime(TimeUnit::Milliseconds, Some("America/Chicago".into()))
            .into_series();
        frame.with_column(event_times).unwrap();
        ParquetWriter::new(file.reopen().unwrap())
            .finish(&mut frame)
            .unwrap();
        file
    }

    #[test]
    fn reads_schema() {
        let file = synthetic_file();
        let schema = read_schema(file.path()).unwrap();
        let names = schema
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>();
        assert!(names.contains(&"description"));
        assert!(names.contains(&"record_id"));
    }

    #[test]
    fn formats_timezone_aware_datetimes() {
        let file = synthetic_file();
        let columns = vec!["event_time".into()];

        let result = query(file.path(), &SearchCriteria::default(), &columns, 1, 0).unwrap();

        assert!(result.rows[0][0].contains("1969-12-31 18:00:00"));
    }

    #[test]
    fn browses_without_filters() {
        let file = synthetic_file();
        let columns = vec!["record_id".into(), "description".into()];
        let result = query(file.path(), &SearchCriteria::default(), &columns, 10, 0).unwrap();
        assert_eq!(result.rows.len(), 10);
        assert_eq!(result.total_matches, 100);
    }

    #[test]
    fn filters_single_and_multiple_values() {
        let file = synthetic_file();
        let columns = vec!["record_id".into(), "category".into()];
        let mut criteria = SearchCriteria::default();
        criteria
            .filters
            .insert("category".into(), vec!["Group A".into()]);
        let result = query(file.path(), &criteria, &columns, 100, 0).unwrap();
        assert!(result.rows.iter().all(|row| row[1] == "Group A"));

        criteria
            .filters
            .insert("category".into(), vec!["Group A".into(), "Group B".into()]);
        let result = query(file.path(), &criteria, &columns, 100, 0).unwrap();
        assert_eq!(result.total_matches, 100);
    }

    #[test]
    fn searches_and_combines_filters() {
        let file = synthetic_file();
        let columns = vec!["record_id".into(), "description".into(), "category".into()];
        let mut criteria = SearchCriteria {
            column: Some("description".into()),
            text: "alpha".into(),
            ..Default::default()
        };
        criteria
            .filters
            .insert("category".into(), vec!["Group A".into()]);
        let result = query(file.path(), &criteria, &columns, 100, 0).unwrap();
        assert!(
            result
                .rows
                .iter()
                .all(|row| { row[1].contains("alpha") && row[2] == "Group A" })
        );
    }

    #[test]
    fn excludes_null_values() {
        let file = synthetic_file();
        let columns = vec!["optional".into()];
        let mut criteria = SearchCriteria::default();
        criteria.exclude_empty.insert("optional".into());
        let result = query(file.path(), &criteria, &columns, 100, 0).unwrap();
        assert_eq!(result.total_matches, 75);
        assert!(result.rows.iter().all(|row| row[0] == "present"));
    }

    #[test]
    fn lists_unique_values() {
        let file = synthetic_file();
        let values = unique_values(file.path(), "category", &HashMap::new(), 100).unwrap();
        assert_eq!(values, vec!["Group A", "Group B"]);
    }

    #[test]
    fn exports_filtered_rows() {
        let file = synthetic_file();
        let output_directory = tempfile::tempdir().unwrap();
        let output = output_directory
            .path()
            .join("pqview_synthetic_export.parquet");
        let columns = vec!["record_id".into(), "category".into()];
        let mut criteria = SearchCriteria::default();
        criteria
            .filters
            .insert("category".into(), vec!["Group A".into()]);

        let count = export(file.path(), &output, &criteria, &columns).unwrap();
        let exported = LazyFrame::scan_parquet(&output, Default::default())
            .unwrap()
            .collect()
            .unwrap();
        assert_eq!(count, 34);
        assert_eq!(exported.height(), count);
    }

    #[test]
    fn refuses_to_overwrite_an_export() {
        let file = synthetic_file();
        let output = NamedTempFile::with_suffix("_pqview_existing.parquet").unwrap();
        let columns = vec!["record_id".into()];

        let error = export(
            file.path(),
            output.path(),
            &SearchCriteria::default(),
            &columns,
        )
        .unwrap_err();
        assert!(error.to_string().contains("already exists"));
    }
}
