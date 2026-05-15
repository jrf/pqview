use anyhow::Result;
use polars::prelude::*;
use std::collections::{HashMap, HashSet};
use std::path::Path;

pub fn read_schema(path: &Path) -> Result<Schema> {
    let mut lf = LazyFrame::scan_parquet(path, Default::default())?;
    let schema = lf.collect_schema()?;
    Ok(schema.as_ref().clone())
}

pub struct SearchResult {
    pub rows: Vec<Vec<String>>,
    pub total_matches: usize,
}

pub fn query(
    path: &Path,
    filters: &HashMap<String, Vec<String>>,
    exclude_empty: &HashSet<String>,
    search_column: Option<&str>,
    search_text: &str,
    display_columns: &[String],
    limit: u32,
    offset: u32,
) -> Result<SearchResult> {
    let lf = LazyFrame::scan_parquet(path, Default::default())?;

    let mut filtered = lf;

    for (column, values) in filters {
        if values.is_empty() {
            continue;
        }
        let lower_vals: Vec<String> = values.iter().map(|v| v.to_lowercase()).collect();
        let series = Series::new("".into(), &lower_vals);
        filtered = filtered.filter(
            col(column)
                .cast(DataType::String)
                .str()
                .to_lowercase()
                .is_in(lit(series)),
        );
    }

    for column in exclude_empty {
        filtered = filtered.filter(
            col(column)
                .is_not_null()
                .and(col(column).cast(DataType::String).neq(lit(""))),
        );
    }

    if let Some(search_col) = search_column {
        if !search_text.is_empty() {
            let pattern = format!("(?i){}", regex_escape(search_text));
            filtered = filtered.filter(
                col(search_col)
                    .cast(DataType::String)
                    .str()
                    .contains(lit(pattern), false),
            );
        }
    }

    let count_df = filtered
        .clone()
        .select([len().alias("count")])
        .collect()?;
    let total_matches = count_df.column("count")?.u32()?.get(0).unwrap_or(0) as usize;

    let select_cols: Vec<Expr> = display_columns.iter().map(|c| col(c)).collect();

    let result_df = filtered
        .select(select_cols)
        .slice(offset as i64, limit)
        .collect()?;

    let rows = dataframe_to_strings(&result_df, display_columns);

    Ok(SearchResult {
        rows,
        total_matches,
    })
}

pub fn export(
    path: &Path,
    output: &Path,
    filters: &HashMap<String, Vec<String>>,
    exclude_empty: &HashSet<String>,
    search_column: Option<&str>,
    search_text: &str,
    columns: &[String],
) -> Result<usize> {
    let lf = LazyFrame::scan_parquet(path, Default::default())?;
    let mut filtered = lf;

    for (column, values) in filters {
        if values.is_empty() {
            continue;
        }
        let lower_vals: Vec<String> = values.iter().map(|v| v.to_lowercase()).collect();
        let series = Series::new("".into(), &lower_vals);
        filtered = filtered.filter(
            col(column)
                .cast(DataType::String)
                .str()
                .to_lowercase()
                .is_in(lit(series)),
        );
    }

    for column in exclude_empty {
        filtered = filtered.filter(
            col(column)
                .is_not_null()
                .and(col(column).cast(DataType::String).neq(lit(""))),
        );
    }

    if let Some(search_col) = search_column {
        if !search_text.is_empty() {
            let pattern = format!("(?i){}", regex_escape(search_text));
            filtered = filtered.filter(
                col(search_col)
                    .cast(DataType::String)
                    .str()
                    .contains(lit(pattern), false),
            );
        }
    }

    let select_cols: Vec<Expr> = columns.iter().map(|c| col(c)).collect();
    let mut df = filtered.select(select_cols).collect()?;
    let count = df.height();

    use polars::prelude::ParquetWriter;
    let file = std::fs::File::create(output)?;
    ParquetWriter::new(file).finish(&mut df)?;

    Ok(count)
}

pub fn unique_values(
    path: &Path,
    column: &str,
    other_filters: &HashMap<String, Vec<String>>,
    limit: u32,
) -> Result<Vec<String>> {
    let lf = LazyFrame::scan_parquet(path, Default::default())?;

    let mut filtered = lf;
    for (col_name, values) in other_filters {
        if values.is_empty() || col_name == column {
            continue;
        }
        let lower_vals: Vec<String> = values.iter().map(|v| v.to_lowercase()).collect();
        let series = Series::new("".into(), &lower_vals);
        filtered = filtered.filter(
            col(col_name)
                .cast(DataType::String)
                .str()
                .to_lowercase()
                .is_in(lit(series)),
        );
    }

    let df = filtered
        .select([col(column).cast(DataType::String)])
        .unique(None, UniqueKeepStrategy::First)
        .sort([column], Default::default())
        .limit(limit)
        .collect()?;

    let mut values = Vec::new();
    let series = df.column(column)?;
    for i in 0..series.len() {
        let v = series.get(i)?;
        let s = format!("{}", v);
        let s = s.strip_prefix('"').unwrap_or(&s);
        let s = s.strip_suffix('"').unwrap_or(s);
        if !s.is_empty() && s != "null" {
            values.push(s.to_string());
        }
    }

    Ok(values)
}

fn dataframe_to_strings(df: &DataFrame, columns: &[String]) -> Vec<Vec<String>> {
    let height = df.height();
    let mut rows = Vec::with_capacity(height);

    for i in 0..height {
        let mut row = Vec::with_capacity(columns.len());
        for col_name in columns {
            let val = df
                .column(col_name)
                .ok()
                .and_then(|s| {
                    let v = s.get(i).ok()?;
                    Some(format!("{}", v))
                })
                .unwrap_or_default();
            let val = val.strip_prefix('"').unwrap_or(&val);
            let val = val.strip_suffix('"').unwrap_or(val);
            row.push(val.to_string());
        }
        rows.push(row);
    }

    rows
}

fn regex_escape(s: &str) -> String {
    let mut escaped = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' | '.' | '+' | '*' | '?' | '(' | ')' | '|' | '[' | ']' | '{' | '}' | '^'
            | '$' => {
                escaped.push('\\');
                escaped.push(c);
            }
            _ => escaped.push(c),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn test_file() -> PathBuf {
        PathBuf::from("/tmp/test_notes.parquet")
    }

    #[test]
    fn test_read_schema() {
        let schema = read_schema(&test_file()).unwrap();
        let names: Vec<&str> = schema.iter().map(|(n, _)| n.as_str()).collect();
        assert!(names.contains(&"clinical_note"));
        assert!(names.contains(&"patient_id"));
    }

    #[test]
    fn test_browse_no_filters() {
        let cols = vec!["patient_id".into(), "clinical_note".into()];
        let result = query(&test_file(), &HashMap::new(), &HashSet::new(), None, "", &cols, 10, 0).unwrap();
        assert_eq!(result.rows.len(), 10);
        assert_eq!(result.total_matches, 100);
    }

    #[test]
    fn test_filter_single_value() {
        let cols = vec!["patient_id".into(), "department".into()];
        let mut filters = HashMap::new();
        filters.insert("department".into(), vec!["Cardiology".into()]);
        let result = query(&test_file(), &filters, &HashSet::new(), None, "", &cols, 50, 0).unwrap();
        assert!(result.total_matches > 0);
        for row in &result.rows {
            assert_eq!(row[1].to_lowercase(), "cardiology");
        }
    }

    #[test]
    fn test_filter_multi_value() {
        let cols = vec!["patient_id".into(), "department".into()];
        let mut filters = HashMap::new();
        filters.insert(
            "department".into(),
            vec!["Cardiology".into(), "Emergency".into()],
        );
        let result = query(&test_file(), &filters, &HashSet::new(), None, "", &cols, 50, 0).unwrap();
        assert!(result.total_matches > 0);
        for row in &result.rows {
            let dept = row[1].to_lowercase();
            assert!(dept == "cardiology" || dept == "emergency");
        }
    }

    #[test]
    fn test_search_substring() {
        let cols = vec!["patient_id".into(), "clinical_note".into()];
        let result = query(
            &test_file(),
            &HashMap::new(),
            &HashSet::new(),
            Some("clinical_note"),
            "chest pain",
            &cols,
            50,
            0,
        )
        .unwrap();
        assert!(result.total_matches > 0);
        assert!(result.rows[0][1].to_lowercase().contains("chest pain"));
    }

    #[test]
    fn test_filter_and_search_combined() {
        let cols = vec![
            "patient_id".into(),
            "clinical_note".into(),
            "department".into(),
        ];
        let mut filters = HashMap::new();
        filters.insert("department".into(), vec!["Cardiology".into()]);
        let result = query(
            &test_file(),
            &filters,
            &HashSet::new(),
            Some("clinical_note"),
            "chest pain",
            &cols,
            50,
            0,
        )
        .unwrap();
        assert!(result.total_matches > 0);
        for row in &result.rows {
            assert_eq!(row[2].to_lowercase(), "cardiology");
            assert!(row[1].to_lowercase().contains("chest pain"));
        }
    }

    #[test]
    fn test_unique_values() {
        let values = unique_values(&test_file(), "department", &HashMap::new(), 100).unwrap();
        assert!(values.contains(&"Cardiology".to_string()));
        assert!(values.contains(&"Emergency".to_string()));
    }
}
