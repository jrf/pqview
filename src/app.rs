use crate::render;
use crate::search;
use crate::theme::{self, Theme};
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::ExecutableCommand;
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use ratatui::prelude::*;
use std::collections::{HashMap, HashSet};
use std::io::stdout;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

const PAGE_SIZE: u32 = 200;
const PICKER_MAX_DEPTH: usize = 6;

#[derive(Clone, PartialEq)]
pub enum Mode {
    Browse,
    Search,
    Filter,
    Columns,
    Export,
    FilePicker,
}

pub struct App {
    pub mode: Mode,
    pub file: Option<PathBuf>,
    pub columns: Vec<String>,

    // Search: substring match within a column
    pub search_query: String,
    pub search_cursor: usize,
    pub search_column: usize,

    // Filters: multi-select exact-value subsetting per column
    pub filters: HashMap<String, Vec<String>>,
    pub filter_column: usize,
    pub filter_suggestions: Vec<String>,
    pub filter_selected: HashSet<String>,
    pub filter_cursor_idx: usize,
    pub filter_scroll: usize,

    // Column visibility
    pub visible_columns: HashSet<String>,
    pub column_picker_idx: usize,

    // Exclude nulls/empties per column
    pub exclude_empty: HashSet<String>,

    // Export
    pub export_path: String,
    pub export_cursor: usize,

    // File picker
    pub picker_root: PathBuf,
    pub picker_paths: Vec<PathBuf>,
    pub picker_strs: Vec<String>,
    pub picker_matches: Vec<usize>,
    pub picker_query: String,
    pub picker_cursor: usize,
    pub picker_idx: usize,
    pub picker_scroll: usize,

    // Fuzzy query inside Filter/Columns popups (matches index into the source list)
    pub popup_query: String,
    pub popup_query_cursor: usize,
    pub popup_searching: bool,
    pub popup_matches: Vec<usize>,
    pub popup_visible_height: usize,

    matcher: Matcher,

    pub rows: Vec<Vec<String>>,
    pub total_matches: usize,
    pub selected: usize,
    pub offset: u32,
    pub scroll_offset: usize,
    pub preview_scroll: usize,
    pub preview_column: Option<usize>,
    pub show_preview: bool,
    pub table_height: usize,
    pub loading: bool,
    pub flash: Option<(String, Instant)>,
    pub theme: Theme,
    pub theme_idx: usize,
    query_rx: Option<mpsc::Receiver<Result<search::SearchResult>>>,
}

impl App {
    fn new() -> Self {
        let picker_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self {
            mode: Mode::Browse,
            file: None,
            columns: Vec::new(),
            search_query: String::new(),
            search_cursor: 0,
            search_column: 0,
            filters: HashMap::new(),
            filter_column: 0,
            filter_suggestions: Vec::new(),
            filter_selected: HashSet::new(),
            filter_cursor_idx: 0,
            filter_scroll: 0,
            visible_columns: HashSet::new(),
            column_picker_idx: 0,
            exclude_empty: HashSet::new(),
            export_path: String::new(),
            export_cursor: 0,
            picker_root,
            picker_paths: Vec::new(),
            picker_strs: Vec::new(),
            picker_matches: Vec::new(),
            picker_query: String::new(),
            picker_cursor: 0,
            picker_idx: 0,
            picker_scroll: 0,
            popup_query: String::new(),
            popup_query_cursor: 0,
            popup_searching: false,
            popup_matches: Vec::new(),
            popup_visible_height: 0,
            matcher: Matcher::new(Config::DEFAULT.match_paths()),
            rows: Vec::new(),
            total_matches: 0,
            selected: 0,
            offset: 0,
            scroll_offset: 0,
            preview_scroll: 0,
            preview_column: None,
            show_preview: true,
            table_height: 0,
            loading: false,
            flash: None,
            theme: theme::THEMES[0].clone(),
            theme_idx: 0,
            query_rx: None,
        }
    }

    pub fn active_filters(&self) -> &HashMap<String, Vec<String>> {
        &self.filters
    }

    pub fn has_active_filters(&self) -> bool {
        self.filters.values().any(|v| !v.is_empty()) || !self.exclude_empty.is_empty()
    }

    pub fn filter_display(&self, col: &str) -> Option<String> {
        self.filters.get(col).map(|vals| vals.join(", "))
    }

    pub fn search_column_name(&self) -> &str {
        self.columns
            .get(self.search_column)
            .map(|s| s.as_str())
            .unwrap_or("")
    }

    pub fn display_columns(&self) -> Vec<String> {
        self.columns
            .iter()
            .filter(|c| self.visible_columns.contains(*c))
            .cloned()
            .collect()
    }

    fn toggle_column_visibility(&mut self) {
        let Some(&src_idx) = self.popup_matches.get(self.column_picker_idx) else {
            return;
        };
        let Some(col) = self.columns.get(src_idx).cloned() else {
            return;
        };
        if self.visible_columns.contains(&col) {
            self.visible_columns.remove(&col);
        } else {
            self.visible_columns.insert(col);
        }
        self.snap_filter_column();
    }

    fn select_all_columns(&mut self) {
        for &idx in &self.popup_matches {
            if let Some(col) = self.columns.get(idx) {
                self.visible_columns.insert(col.clone());
            }
        }
    }

    fn deselect_all_columns(&mut self) {
        for &idx in &self.popup_matches {
            if let Some(col) = self.columns.get(idx) {
                self.visible_columns.remove(col);
            }
        }
        self.snap_filter_column();
    }

    fn cycle_theme(&mut self) {
        self.theme_idx = (self.theme_idx + 1) % theme::THEMES.len();
        self.theme = theme::THEMES[self.theme_idx].clone();
        self.flash = Some((format!("Theme: {}", self.theme.name), Instant::now()));
    }

    fn toggle_exclude_empty(&mut self) {
        let Some(col) = self.columns.get(self.filter_column).cloned() else {
            return;
        };
        if self.exclude_empty.contains(&col) {
            self.exclude_empty.remove(&col);
        } else {
            self.exclude_empty.insert(col);
        }
        self.reset_results();
        self.execute_query();
    }

    fn snap_filter_column(&mut self) {
        if self.columns.is_empty() {
            self.filter_column = 0;
            return;
        }
        if !self.visible_columns.contains(&self.columns[self.filter_column]) {
            for (i, col) in self.columns.iter().enumerate() {
                if self.visible_columns.contains(col) {
                    self.filter_column = i;
                    return;
                }
            }
        }
    }

    fn execute_query(&mut self) {
        let Some(path) = self.file.clone() else {
            return;
        };
        let filters: HashMap<String, Vec<String>> = self
            .filters
            .iter()
            .filter(|(_, v)| !v.is_empty())
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let columns = self.columns.clone();
        let offset = self.offset;
        let search_col = self
            .columns
            .get(self.search_column)
            .cloned()
            .unwrap_or_default();
        let search_text = self.search_query.clone();
        let exclude_empty = self.exclude_empty.clone();

        self.loading = true;

        let (tx, rx) = mpsc::channel();
        self.query_rx = Some(rx);

        std::thread::spawn(move || {
            let search = if search_text.is_empty() {
                None
            } else {
                Some(search_col.as_str())
            };
            let result = search::query(
                &path,
                &filters,
                &exclude_empty,
                search,
                &search_text,
                &columns,
                PAGE_SIZE,
                offset,
            );
            let _ = tx.send(result);
        });
    }

    fn check_query_result(&mut self) {
        if let Some(rx) = &self.query_rx {
            match rx.try_recv() {
                Ok(Ok(result)) => {
                    self.rows = result.rows;
                    self.total_matches = result.total_matches;
                    self.loading = false;
                    self.query_rx = None;
                    if self.selected >= self.rows.len() && !self.rows.is_empty() {
                        self.selected = self.rows.len() - 1;
                    }
                }
                Ok(Err(e)) => {
                    self.flash = Some((format!("Query error: {}", e), Instant::now()));
                    self.loading = false;
                    self.query_rx = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.loading = false;
                    self.query_rx = None;
                }
            }
        }
    }

    fn next_page(&mut self) {
        if (self.offset + PAGE_SIZE) < self.total_matches as u32 {
            self.offset += PAGE_SIZE;
            self.selected = 0;
            self.scroll_offset = 0;
            self.execute_query();
        }
    }

    fn prev_page(&mut self) {
        if self.offset >= PAGE_SIZE {
            self.offset -= PAGE_SIZE;
            self.selected = 0;
            self.scroll_offset = 0;
            self.execute_query();
        }
    }

    fn enter_filter_mode(&mut self) {
        if self.file.is_none() || self.columns.is_empty() {
            return;
        }
        let col_name = &self.columns[self.filter_column];
        self.filter_selected = self
            .filters
            .get(col_name)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .collect();
        self.filter_cursor_idx = 0;
        self.filter_scroll = 0;
        self.load_filter_suggestions();
        self.reset_popup_search();
        self.refresh_popup_matches_for(PopupSource::FilterValues);
        self.mode = Mode::Filter;
    }

    fn enter_columns_mode(&mut self) {
        if self.file.is_none() || self.columns.is_empty() {
            return;
        }
        self.column_picker_idx = 0;
        self.reset_popup_search();
        self.refresh_popup_matches_for(PopupSource::Columns);
        self.mode = Mode::Columns;
    }

    fn reset_popup_search(&mut self) {
        self.popup_query.clear();
        self.popup_query_cursor = 0;
        self.popup_searching = false;
        self.popup_matches.clear();
    }

    fn refresh_popup_matches_for(&mut self, source: PopupSource) {
        self.popup_matches = match source {
            PopupSource::FilterValues => {
                rank_against(&mut self.matcher, &self.popup_query, &self.filter_suggestions)
            }
            PopupSource::Columns => {
                rank_against(&mut self.matcher, &self.popup_query, &self.columns)
            }
        };
        let cursor = match source {
            PopupSource::FilterValues => &mut self.filter_cursor_idx,
            PopupSource::Columns => &mut self.column_picker_idx,
        };
        if *cursor >= self.popup_matches.len() {
            *cursor = self.popup_matches.len().saturating_sub(1);
        }
    }

    fn refresh_popup_matches_current(&mut self) {
        match self.mode {
            Mode::Filter => self.refresh_popup_matches_for(PopupSource::FilterValues),
            Mode::Columns => self.refresh_popup_matches_for(PopupSource::Columns),
            _ => {}
        }
    }

    fn popup_insert_char(&mut self, c: char) {
        self.popup_query.insert(self.popup_query_cursor, c);
        self.popup_query_cursor += 1;
        self.reset_popup_cursor();
        self.refresh_popup_matches_current();
    }

    fn popup_backspace(&mut self) {
        if self.popup_query_cursor > 0 {
            self.popup_query_cursor -= 1;
            self.popup_query.remove(self.popup_query_cursor);
            self.reset_popup_cursor();
            self.refresh_popup_matches_current();
        }
    }

    fn popup_clear_query(&mut self) {
        self.popup_query.clear();
        self.popup_query_cursor = 0;
        self.reset_popup_cursor();
        self.refresh_popup_matches_current();
    }

    fn reset_popup_cursor(&mut self) {
        match self.mode {
            Mode::Filter => self.filter_cursor_idx = 0,
            Mode::Columns => self.column_picker_idx = 0,
            _ => {}
        }
    }

    fn load_filter_suggestions(&mut self) {
        let Some(file) = self.file.clone() else {
            self.filter_suggestions = Vec::new();
            return;
        };
        let col_name = self.columns[self.filter_column].clone();
        // Load unique values ignoring this column's own filter so all options are visible
        let other_filters: HashMap<String, Vec<String>> = self
            .filters
            .iter()
            .filter(|(k, v)| *k != &col_name && !v.is_empty())
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        match search::unique_values(&file, &col_name, &other_filters, 500) {
            Ok(values) => self.filter_suggestions = values,
            Err(_) => self.filter_suggestions = Vec::new(),
        }
    }

    fn toggle_filter_value(&mut self) {
        let Some(&src_idx) = self.popup_matches.get(self.filter_cursor_idx) else {
            return;
        };
        let Some(value) = self.filter_suggestions.get(src_idx).cloned() else {
            return;
        };
        if self.filter_selected.contains(&value) {
            self.filter_selected.remove(&value);
        } else {
            self.filter_selected.insert(value);
        }
        self.sync_filter_to_state();
        self.reset_results();
        self.execute_query();
    }

    fn sync_filter_to_state(&mut self) {
        let col_name = self.columns[self.filter_column].clone();
        if self.filter_selected.is_empty() {
            self.filters.remove(&col_name);
        } else {
            let mut vals: Vec<String> = self.filter_selected.iter().cloned().collect();
            vals.sort();
            self.filters.insert(col_name, vals);
        }
    }

    fn clear_all_filters(&mut self) {
        self.filters.clear();
        self.exclude_empty.clear();
        self.search_query.clear();
        self.search_cursor = 0;
        self.reset_results();
        self.execute_query();
    }

    fn reset_results(&mut self) {
        self.offset = 0;
        self.selected = 0;
        self.scroll_offset = 0;
        self.preview_scroll = 0;
    }

    pub fn enter_file_picker(&mut self) {
        self.picker_paths.clear();
        self.picker_strs.clear();
        walk_parquet_files(
            &self.picker_root,
            &self.picker_root,
            PICKER_MAX_DEPTH,
            &mut self.picker_paths,
            &mut self.picker_strs,
        );
        // Stable alphabetical baseline for empty query
        let mut order: Vec<usize> = (0..self.picker_strs.len()).collect();
        order.sort_by(|a, b| self.picker_strs[*a].cmp(&self.picker_strs[*b]));
        let paths: Vec<PathBuf> = order.iter().map(|i| self.picker_paths[*i].clone()).collect();
        let strs: Vec<String> = order.iter().map(|i| self.picker_strs[*i].clone()).collect();
        self.picker_paths = paths;
        self.picker_strs = strs;
        self.picker_query.clear();
        self.picker_cursor = 0;
        self.picker_idx = 0;
        self.picker_scroll = 0;
        self.refresh_picker_matches();
        self.mode = Mode::FilePicker;
    }

    fn refresh_picker_matches(&mut self) {
        self.picker_matches = rank_against(&mut self.matcher, &self.picker_query, &self.picker_strs);
        if self.picker_idx >= self.picker_matches.len() {
            self.picker_idx = self.picker_matches.len().saturating_sub(1);
        }
        self.picker_scroll = 0;
    }

    fn pick_current_file(&mut self) {
        let Some(&path_idx) = self.picker_matches.get(self.picker_idx) else {
            return;
        };
        let path = self.picker_paths[path_idx].clone();
        match self.load_file(path) {
            Ok(()) => {
                self.mode = Mode::Browse;
            }
            Err(e) => {
                self.flash = Some((format!("Load error: {}", e), Instant::now()));
            }
        }
    }

    fn load_file(&mut self, path: PathBuf) -> Result<()> {
        let schema = search::read_schema(&path)?;
        let columns: Vec<String> = schema.iter().map(|(name, _)| name.to_string()).collect();
        if columns.is_empty() {
            anyhow::bail!("No columns found in {}", path.display());
        }

        self.file = Some(path);
        self.columns = columns;
        self.visible_columns = self.columns.iter().cloned().collect();
        self.filters.clear();
        self.exclude_empty.clear();
        self.search_query.clear();
        self.search_cursor = 0;
        self.search_column = 0;
        self.filter_column = 0;
        self.filter_suggestions.clear();
        self.filter_selected.clear();
        self.filter_cursor_idx = 0;
        self.filter_scroll = 0;
        self.column_picker_idx = 0;
        self.preview_column = None;
        self.rows.clear();
        self.total_matches = 0;
        self.query_rx = None;
        self.reset_results();
        self.execute_query();
        Ok(())
    }
}

enum PopupSource {
    FilterValues,
    Columns,
}

fn rank_against(matcher: &mut Matcher, query: &str, source: &[String]) -> Vec<usize> {
    if query.is_empty() {
        return (0..source.len()).collect();
    }
    let pattern = Pattern::parse(query, CaseMatching::Smart, Normalization::Smart);
    let mut scored: Vec<(usize, u32)> = Vec::with_capacity(source.len());
    for (i, cand) in source.iter().enumerate() {
        let mut hay_buf = Vec::new();
        let haystack = Utf32Str::new(cand, &mut hay_buf);
        if let Some(score) = pattern.score(haystack, matcher) {
            scored.push((i, score));
        }
    }
    scored.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| source[a.0].cmp(&source[b.0])));
    scored.into_iter().map(|(i, _)| i).collect()
}

fn walk_parquet_files(
    root: &Path,
    dir: &Path,
    depth: usize,
    paths: &mut Vec<PathBuf>,
    strs: &mut Vec<String>,
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
            .and_then(|n| n.to_str())
            .unwrap_or("");
        if file_name.starts_with('.') {
            continue;
        }
        let Ok(ft) = entry.file_type() else { continue };
        if ft.is_dir() {
            if matches!(file_name, "target" | "node_modules") {
                continue;
            }
            walk_parquet_files(root, &path, depth - 1, paths, strs);
        } else if ft.is_file() && path.extension().is_some_and(|e| e == "parquet") {
            let display = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();
            paths.push(path);
            strs.push(display);
        }
    }
}

pub fn run(file: Option<PathBuf>) -> Result<()> {
    let mut app = App::new();

    if let Some(path) = file {
        match app.load_file(path) {
            Ok(()) => {
                // Replace background query with a synchronous initial load so
                // data is visible on first frame.
                app.query_rx = None;
                app.loading = false;
                if let (Some(file), false) = (app.file.clone(), app.columns.is_empty()) {
                    match search::query(
                        &file,
                        &HashMap::new(),
                        &HashSet::new(),
                        None,
                        "",
                        &app.columns,
                        PAGE_SIZE,
                        0,
                    ) {
                        Ok(result) => {
                            app.rows = result.rows;
                            app.total_matches = result.total_matches;
                        }
                        Err(e) => {
                            app.flash = Some((format!("Load error: {}", e), Instant::now()));
                        }
                    }
                }
            }
            Err(e) => {
                app.flash = Some((format!("Load error: {}", e), Instant::now()));
            }
        }
    } else {
        app.enter_file_picker();
    }

    let mut tty = stdout();
    tty.execute(EnterAlternateScreen)?;
    terminal::enable_raw_mode()?;

    let panic_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = terminal::disable_raw_mode();
        let _ = stdout().execute(LeaveAlternateScreen);
        panic_hook(info);
    }));

    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;

    loop {
        app.check_query_result();

        terminal.draw(|f| render::draw(f, &mut app))?;

        let timeout = if app.loading {
            Duration::from_millis(50)
        } else if app.flash.is_some() {
            Duration::from_millis(100)
        } else {
            Duration::from_secs(60)
        };

        if !event::poll(timeout)? {
            if let Some((_, t)) = &app.flash {
                if t.elapsed() > Duration::from_secs(2) {
                    app.flash = None;
                }
            }
            continue;
        }

        let Event::Key(key) = event::read()? else {
            continue;
        };

        if key.kind != KeyEventKind::Press {
            continue;
        }

        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            break;
        }

        match app.mode {
            Mode::Browse => match key.code {
                KeyCode::Char('q') | KeyCode::Esc => break,
                KeyCode::Char('o') => {
                    app.enter_file_picker();
                }
                KeyCode::Char('/') | KeyCode::Char('s') => {
                    if app.file.is_some() {
                        app.search_column = app.filter_column;
                        app.mode = Mode::Search;
                    }
                }
                KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    let page = app.table_height.saturating_sub(2);
                    let max = app.rows.len().saturating_sub(1);
                    app.selected = (app.selected + page).min(max);
                    app.scroll_offset = (app.scroll_offset + page).min(max);
                    app.preview_scroll = 0;
                }
                KeyCode::Char('f') => {
                    app.enter_filter_mode();
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if app.selected > 0 {
                        app.selected -= 1;
                        app.preview_scroll = 0;
                        if app.selected < app.scroll_offset {
                            app.scroll_offset = app.selected;
                        }
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if app.selected < app.rows.len().saturating_sub(1) {
                        app.selected += 1;
                        app.preview_scroll = 0;
                        let visible = app.table_height.saturating_sub(1);
                        if visible > 0 && app.selected >= app.scroll_offset + visible {
                            app.scroll_offset = app.selected - visible + 1;
                        }
                    }
                }
                KeyCode::Left | KeyCode::Char('h') => {
                    let mut idx = app.filter_column;
                    while idx > 0 {
                        idx -= 1;
                        if app.visible_columns.contains(&app.columns[idx]) {
                            app.filter_column = idx;
                            break;
                        }
                    }
                }
                KeyCode::Right | KeyCode::Char('l') => {
                    if app.columns.is_empty() {
                        continue;
                    }
                    let mut idx = app.filter_column;
                    while idx < app.columns.len() - 1 {
                        idx += 1;
                        if app.visible_columns.contains(&app.columns[idx]) {
                            app.filter_column = idx;
                            break;
                        }
                    }
                }
                KeyCode::Char('n') => app.next_page(),
                KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    let page = app.table_height.saturating_sub(2);
                    app.selected = app.selected.saturating_sub(page);
                    app.scroll_offset = app.scroll_offset.saturating_sub(page);
                    app.preview_scroll = 0;
                }
                KeyCode::Char('p') => app.prev_page(),
                KeyCode::Tab => {
                    app.show_preview = !app.show_preview;
                }
                KeyCode::Enter => {
                    if app.show_preview {
                        app.preview_column = Some(app.filter_column);
                        app.preview_scroll = 0;
                    }
                }
                KeyCode::Char('J') => {
                    app.preview_scroll += 1;
                }
                KeyCode::Char('K') => {
                    app.preview_scroll = app.preview_scroll.saturating_sub(1);
                }
                KeyCode::Char('g') => {
                    app.selected = 0;
                    app.scroll_offset = 0;
                    app.preview_scroll = 0;
                }
                KeyCode::Char('G') => {
                    let max = app.rows.len().saturating_sub(1);
                    app.selected = max;
                    let visible = app.table_height.saturating_sub(1);
                    app.scroll_offset = max.saturating_sub(visible);
                    app.preview_scroll = 0;
                }
                KeyCode::Char('x') => {
                    app.toggle_exclude_empty();
                }
                KeyCode::Char('C') => {
                    app.clear_all_filters();
                }
                KeyCode::Char('v') => {
                    app.enter_columns_mode();
                }
                KeyCode::Char('t') => {
                    app.cycle_theme();
                }
                KeyCode::Char('w') => {
                    let Some(file) = app.file.clone() else {
                        continue;
                    };
                    let stem = file.file_stem().and_then(|s| s.to_str()).unwrap_or("output");
                    let dir = file.parent().unwrap_or(std::path::Path::new("."));
                    let default_path = dir.join(format!("{}_filtered.parquet", stem));
                    app.export_path = default_path.to_string_lossy().to_string();
                    app.export_cursor = app.export_path.len();
                    app.mode = Mode::Export;
                }
                _ => {}
            },

            Mode::Search => match key.code {
                KeyCode::Esc => {
                    app.mode = Mode::Browse;
                }
                KeyCode::Enter => {
                    app.reset_results();
                    app.execute_query();
                    app.mode = Mode::Browse;
                }
                KeyCode::BackTab => {
                    if !app.columns.is_empty() {
                        app.search_column = if app.search_column == 0 {
                            app.columns.len() - 1
                        } else {
                            app.search_column - 1
                        };
                    }
                }
                KeyCode::Tab => {
                    if !app.columns.is_empty() {
                        app.search_column = (app.search_column + 1) % app.columns.len();
                    }
                }
                KeyCode::Backspace => {
                    if app.search_cursor > 0 {
                        app.search_cursor -= 1;
                        app.search_query.remove(app.search_cursor);
                    }
                }
                KeyCode::Left => {
                    if app.search_cursor > 0 {
                        app.search_cursor -= 1;
                    }
                }
                KeyCode::Right => {
                    if app.search_cursor < app.search_query.len() {
                        app.search_cursor += 1;
                    }
                }
                KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    app.search_query.clear();
                    app.search_cursor = 0;
                }
                KeyCode::Char(c) => {
                    app.search_query.insert(app.search_cursor, c);
                    app.search_cursor += 1;
                }
                _ => {}
            },

            Mode::Columns => handle_columns_key(&mut app, key.code, key.modifiers),

            Mode::Filter => handle_filter_key(&mut app, key.code, key.modifiers),

            Mode::Export => match key.code {
                KeyCode::Esc => {
                    app.mode = Mode::Browse;
                }
                KeyCode::Enter => {
                    let Some(file) = app.file.clone() else {
                        app.mode = Mode::Browse;
                        continue;
                    };
                    let path = PathBuf::from(&app.export_path);
                    let filters: HashMap<String, Vec<String>> = app
                        .filters
                        .iter()
                        .filter(|(_, v)| !v.is_empty())
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect();
                    let search = if app.search_query.is_empty() {
                        None
                    } else {
                        Some(app.columns[app.search_column].as_str())
                    };
                    let visible = app.display_columns();
                    match search::export(
                        &file,
                        &path,
                        &filters,
                        &app.exclude_empty,
                        search,
                        &app.search_query,
                        &visible,
                    ) {
                        Ok(count) => {
                            app.flash = Some((
                                format!("Exported {} rows to {}", count, app.export_path),
                                Instant::now(),
                            ));
                        }
                        Err(e) => {
                            app.flash = Some((format!("Export error: {}", e), Instant::now()));
                        }
                    }
                    app.mode = Mode::Browse;
                }
                KeyCode::Backspace => {
                    if app.export_cursor > 0 {
                        app.export_cursor -= 1;
                        app.export_path.remove(app.export_cursor);
                    }
                }
                KeyCode::Left => {
                    if app.export_cursor > 0 {
                        app.export_cursor -= 1;
                    }
                }
                KeyCode::Right => {
                    if app.export_cursor < app.export_path.len() {
                        app.export_cursor += 1;
                    }
                }
                KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    app.export_path.clear();
                    app.export_cursor = 0;
                }
                KeyCode::Char(c) => {
                    app.export_path.insert(app.export_cursor, c);
                    app.export_cursor += 1;
                }
                _ => {}
            },

            Mode::FilePicker => match key.code {
                KeyCode::Esc => {
                    if app.file.is_some() {
                        app.mode = Mode::Browse;
                    } else {
                        break;
                    }
                }
                KeyCode::Enter => {
                    app.pick_current_file();
                }
                KeyCode::Up => {
                    if app.picker_idx > 0 {
                        app.picker_idx -= 1;
                    }
                }
                KeyCode::Down => {
                    if app.picker_idx + 1 < app.picker_matches.len() {
                        app.picker_idx += 1;
                    }
                }
                KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    if app.picker_idx > 0 {
                        app.picker_idx -= 1;
                    }
                }
                KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    if app.picker_idx + 1 < app.picker_matches.len() {
                        app.picker_idx += 1;
                    }
                }
                KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    let page = popup_page_size(app.popup_visible_height);
                    let len = app.picker_matches.len();
                    popup_page_down(&mut app.picker_idx, len, page);
                }
                KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    let page = popup_page_size(app.popup_visible_height);
                    popup_page_up(&mut app.picker_idx, page);
                }
                KeyCode::Backspace => {
                    if app.picker_cursor > 0 {
                        app.picker_cursor -= 1;
                        app.picker_query.remove(app.picker_cursor);
                        app.refresh_picker_matches();
                    }
                }
                KeyCode::Left => {
                    if app.picker_cursor > 0 {
                        app.picker_cursor -= 1;
                    }
                }
                KeyCode::Right => {
                    if app.picker_cursor < app.picker_query.len() {
                        app.picker_cursor += 1;
                    }
                }
                KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    app.picker_query.clear();
                    app.picker_cursor = 0;
                    app.refresh_picker_matches();
                }
                KeyCode::Char(c) => {
                    app.picker_query.insert(app.picker_cursor, c);
                    app.picker_cursor += 1;
                    app.refresh_picker_matches();
                }
                _ => {}
            },
        }
    }

    terminal::disable_raw_mode()?;
    tty.execute(LeaveAlternateScreen)?;
    Ok(())
}

fn handle_filter_key(app: &mut App, code: KeyCode, mods: KeyModifiers) {
    let ctrl = mods.contains(KeyModifiers::CONTROL);
    let page = popup_page_size(app.popup_visible_height);
    let len = app.popup_matches.len();
    if app.popup_searching {
        match code {
            KeyCode::Esc => app.popup_searching = false,
            KeyCode::Enter | KeyCode::Tab => app.toggle_filter_value(),
            KeyCode::Up | KeyCode::BackTab => popup_nav_up(&mut app.filter_cursor_idx),
            KeyCode::Down => popup_nav_down(&mut app.filter_cursor_idx, len),
            KeyCode::Char('p') if ctrl => popup_nav_up(&mut app.filter_cursor_idx),
            KeyCode::Char('n') if ctrl => popup_nav_down(&mut app.filter_cursor_idx, len),
            KeyCode::Char('f') if ctrl => popup_page_down(&mut app.filter_cursor_idx, len, page),
            KeyCode::Char('b') if ctrl => popup_page_up(&mut app.filter_cursor_idx, page),
            KeyCode::Char('u') if ctrl => app.popup_clear_query(),
            KeyCode::Backspace => app.popup_backspace(),
            KeyCode::Left => {
                if app.popup_query_cursor > 0 {
                    app.popup_query_cursor -= 1;
                }
            }
            KeyCode::Right => {
                if app.popup_query_cursor < app.popup_query.len() {
                    app.popup_query_cursor += 1;
                }
            }
            KeyCode::Char(c) => app.popup_insert_char(c),
            _ => {}
        }
    } else {
        match code {
            KeyCode::Char('f') if ctrl => popup_page_down(&mut app.filter_cursor_idx, len, page),
            KeyCode::Char('b') if ctrl => popup_page_up(&mut app.filter_cursor_idx, page),
            KeyCode::Esc | KeyCode::Char('f') => app.mode = Mode::Browse,
            KeyCode::Char('/') => app.popup_searching = true,
            KeyCode::Char(' ') | KeyCode::Enter => app.toggle_filter_value(),
            KeyCode::Up | KeyCode::Char('k') => popup_nav_up(&mut app.filter_cursor_idx),
            KeyCode::Down | KeyCode::Char('j') => popup_nav_down(&mut app.filter_cursor_idx, len),
            KeyCode::Char('u') if ctrl => app.popup_clear_query(),
            _ => {}
        }
    }
}

fn handle_columns_key(app: &mut App, code: KeyCode, mods: KeyModifiers) {
    let ctrl = mods.contains(KeyModifiers::CONTROL);
    let page = popup_page_size(app.popup_visible_height);
    let len = app.popup_matches.len();
    if app.popup_searching {
        match code {
            KeyCode::Esc => app.popup_searching = false,
            KeyCode::Enter | KeyCode::Tab => app.toggle_column_visibility(),
            KeyCode::Up | KeyCode::BackTab => popup_nav_up(&mut app.column_picker_idx),
            KeyCode::Down => popup_nav_down(&mut app.column_picker_idx, len),
            KeyCode::Char('p') if ctrl => popup_nav_up(&mut app.column_picker_idx),
            KeyCode::Char('n') if ctrl => popup_nav_down(&mut app.column_picker_idx, len),
            KeyCode::Char('f') if ctrl => popup_page_down(&mut app.column_picker_idx, len, page),
            KeyCode::Char('b') if ctrl => popup_page_up(&mut app.column_picker_idx, page),
            KeyCode::Char('a') if ctrl => app.select_all_columns(),
            KeyCode::Char('d') if ctrl => app.deselect_all_columns(),
            KeyCode::Char('u') if ctrl => app.popup_clear_query(),
            KeyCode::Backspace => app.popup_backspace(),
            KeyCode::Left => {
                if app.popup_query_cursor > 0 {
                    app.popup_query_cursor -= 1;
                }
            }
            KeyCode::Right => {
                if app.popup_query_cursor < app.popup_query.len() {
                    app.popup_query_cursor += 1;
                }
            }
            KeyCode::Char(c) => app.popup_insert_char(c),
            _ => {}
        }
    } else {
        match code {
            KeyCode::Char('f') if ctrl => popup_page_down(&mut app.column_picker_idx, len, page),
            KeyCode::Char('b') if ctrl => popup_page_up(&mut app.column_picker_idx, page),
            KeyCode::Esc | KeyCode::Char('v') => app.mode = Mode::Browse,
            KeyCode::Char('/') => app.popup_searching = true,
            KeyCode::Char(' ') | KeyCode::Enter => app.toggle_column_visibility(),
            KeyCode::Char('a') => app.select_all_columns(),
            KeyCode::Char('d') => app.deselect_all_columns(),
            KeyCode::Char('u') if ctrl => app.popup_clear_query(),
            KeyCode::Up | KeyCode::Char('k') => popup_nav_up(&mut app.column_picker_idx),
            KeyCode::Down | KeyCode::Char('j') => popup_nav_down(&mut app.column_picker_idx, len),
            _ => {}
        }
    }
}

fn popup_nav_up(cursor: &mut usize) {
    if *cursor > 0 {
        *cursor -= 1;
    }
}

fn popup_nav_down(cursor: &mut usize, len: usize) {
    if *cursor + 1 < len {
        *cursor += 1;
    }
}

fn popup_page_size(visible: usize) -> usize {
    visible.saturating_sub(2).max(1)
}

fn popup_page_up(cursor: &mut usize, page: usize) {
    *cursor = cursor.saturating_sub(page);
}

fn popup_page_down(cursor: &mut usize, len: usize, page: usize) {
    if len == 0 {
        return;
    }
    *cursor = (*cursor + page).min(len - 1);
}
