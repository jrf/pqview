use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const MAX_RECENTS: usize = 50;

/// Loads recently opened Parquet files, most-recent first.
pub fn load() -> Vec<PathBuf> {
    let Some(path) = recent_path() else {
        return Vec::new();
    };
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(PathBuf::from)
        .collect()
}

/// Records `path` as the most recently opened file. History failures never
/// prevent a file from opening.
pub fn record(path: &Path) {
    let Some(store) = recent_path() else {
        return;
    };
    let entries = promote(load(), path, MAX_RECENTS);
    if let Some(parent) = store.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let contents = entries
        .iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("\n");
    let _ = fs::write(store, contents);
}

fn promote(mut entries: Vec<PathBuf>, path: &Path, max: usize) -> Vec<PathBuf> {
    entries.retain(|existing| existing != path);
    entries.insert(0, path.to_path_buf());
    entries.truncate(max);
    entries
}

fn recent_path() -> Option<PathBuf> {
    let base = env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))?;
    Some(base.join("pqview").join("recent"))
}

#[cfg(test)]
mod tests {
    use super::promote;
    use std::path::PathBuf;

    #[test]
    fn promote_moves_existing_entry_to_front() {
        let entries = vec![
            PathBuf::from("/synthetic/a.parquet"),
            PathBuf::from("/synthetic/b.parquet"),
        ];

        let promoted = promote(entries, &PathBuf::from("/synthetic/b.parquet"), 50);

        assert_eq!(
            promoted,
            vec![
                PathBuf::from("/synthetic/b.parquet"),
                PathBuf::from("/synthetic/a.parquet")
            ]
        );
    }

    #[test]
    fn promote_caps_the_history_length() {
        let entries = vec![
            PathBuf::from("/synthetic/a.parquet"),
            PathBuf::from("/synthetic/b.parquet"),
        ];

        let promoted = promote(entries, &PathBuf::from("/synthetic/c.parquet"), 2);

        assert_eq!(
            promoted,
            vec![
                PathBuf::from("/synthetic/c.parquet"),
                PathBuf::from("/synthetic/a.parquet")
            ]
        );
    }
}
