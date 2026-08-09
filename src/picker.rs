use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Matcher, Utf32Str};
use std::path::{Path, PathBuf};

pub fn rank(matcher: &mut Matcher, query: &str, source: &[String]) -> Vec<usize> {
    if query.is_empty() {
        return (0..source.len()).collect();
    }
    let pattern = Pattern::parse(query, CaseMatching::Smart, Normalization::Smart);
    let mut scored = Vec::with_capacity(source.len());
    for (index, candidate) in source.iter().enumerate() {
        let mut buffer = Vec::new();
        let haystack = Utf32Str::new(candidate, &mut buffer);
        if let Some(score) = pattern.score(haystack, matcher) {
            scored.push((index, score));
        }
    }
    scored.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| source[a.0].cmp(&source[b.0])));
    scored.into_iter().map(|(index, _)| index).collect()
}

pub fn match_indices(query: &str, candidate: &str) -> Vec<usize> {
    if query.is_empty() {
        return Vec::new();
    }
    let pattern = Pattern::parse(query, CaseMatching::Smart, Normalization::Smart);
    let mut matcher = Matcher::new(nucleo_matcher::Config::DEFAULT.match_paths());
    let mut buffer = Vec::new();
    let haystack = Utf32Str::new(candidate, &mut buffer);
    let mut indices = Vec::new();
    let _ = pattern.indices(haystack, &mut matcher, &mut indices);
    indices.sort_unstable();
    indices.dedup();
    indices
        .into_iter()
        .filter_map(|index| usize::try_from(index).ok())
        .collect()
}

pub fn walk_parquet_files(
    root: &Path,
    dir: &Path,
    depth: usize,
    paths: &mut Vec<PathBuf>,
    labels: &mut Vec<String>,
) {
    if depth == 0 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if file_name.starts_with('.') {
            continue;
        }
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            if matches!(file_name, "target" | "node_modules") {
                continue;
            }
            walk_parquet_files(root, &path, depth - 1, paths, labels);
        } else if file_type.is_file() && path.extension().is_some_and(|ext| ext == "parquet") {
            labels.push(
                path.strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string(),
            );
            paths.push(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nucleo_matcher::Config;

    #[test]
    fn ranks_fuzzy_matches() {
        let mut matcher = Matcher::new(Config::DEFAULT.match_paths());
        let source = vec!["alpha.parquet".into(), "beta.parquet".into()];
        assert_eq!(rank(&mut matcher, "alp", &source), vec![0]);
    }

    #[test]
    fn identifies_fuzzy_match_characters() {
        assert_eq!(
            match_indices("spq", "synthetic/path.parquet"),
            vec![0, 10, 18]
        );
    }

    #[test]
    fn finds_parquet_files_and_skips_ignored_directories() {
        let root = tempfile::tempdir().unwrap();
        let nested = root.path().join("nested");
        let ignored = root.path().join("target");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::create_dir_all(&ignored).unwrap();
        std::fs::write(nested.join("synthetic.parquet"), []).unwrap();
        std::fs::write(nested.join("notes.txt"), []).unwrap();
        std::fs::write(ignored.join("ignored.parquet"), []).unwrap();

        let mut paths = Vec::new();
        let mut labels = Vec::new();
        walk_parquet_files(root.path(), root.path(), 6, &mut paths, &mut labels);

        assert_eq!(labels, vec!["nested/synthetic.parquet"]);
        assert_eq!(paths, vec![nested.join("synthetic.parquet")]);
    }
}
