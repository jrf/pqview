use crate::background::{QueryRequest, QueryResponse, QueryWorker};
use crate::input;
use crate::picker;
use crate::recent;
use crate::render;
use crate::search;
use crate::terminal_session;
use crate::theme::{self, Theme};
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use nucleo_matcher::{Config, Matcher};
use ratatui::prelude::*;
use std::collections::{HashMap, HashSet};
use std::io::stdout;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

const PAGE_SIZE: u32 = 1000;
const PICKER_MAX_DEPTH: usize = 6;

#[derive(Clone, Debug, PartialEq)]
pub enum Mode {
    Browse,
    Search,
    Filter,
    Columns,
    Export,
    FilePicker,
    ThemePicker,
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
    pub picker_is_recent: Vec<bool>,
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
    pub filter_values_loading: bool,
    pub flash: Option<(String, Instant)>,
    pub theme: Theme,
    pub themes: Vec<(String, Theme)>,
    pub theme_idx: usize,
    theme_original_idx: Option<usize>,
    query_worker: QueryWorker,
    next_query_id: u64,
    active_query_id: u64,
    filter_suggestions_rx: Option<mpsc::Receiver<Result<Vec<String>>>>,
    export_rx: Option<mpsc::Receiver<(PathBuf, Result<usize>)>>,
    pub exporting: bool,
}

impl App {
    #[cfg(test)]
    pub(crate) fn new() -> Self {
        Self::with_themes(vec![("default".into(), theme::default_theme())], 0)
    }

    fn with_themes(themes: Vec<(String, Theme)>, theme_idx: usize) -> Self {
        let picker_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let theme_idx = theme_idx.min(themes.len().saturating_sub(1));
        let active_theme = themes[theme_idx].1;
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
            picker_is_recent: Vec::new(),
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
            filter_values_loading: false,
            flash: None,
            theme: active_theme,
            themes,
            theme_idx,
            theme_original_idx: None,
            query_worker: QueryWorker::new(PAGE_SIZE),
            next_query_id: 0,
            active_query_id: 0,
            filter_suggestions_rx: None,
            export_rx: None,
            exporting: false,
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

    fn enter_theme_picker(&mut self) {
        self.theme_original_idx = Some(self.theme_idx);
        self.mode = Mode::ThemePicker;
    }

    fn preview_theme(&mut self, index: usize) {
        if let Some((_, theme)) = self.themes.get(index) {
            self.theme_idx = index;
            self.theme = *theme;
        }
    }

    fn confirm_theme(&mut self) {
        self.theme_original_idx = None;
        self.mode = Mode::Browse;
        let name = &self.themes[self.theme_idx].0;
        self.flash = Some((format!("Theme: {name}"), Instant::now()));
    }

    fn cancel_theme_picker(&mut self) {
        if let Some(original) = self.theme_original_idx.take() {
            self.preview_theme(original);
        }
        self.mode = Mode::Browse;
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
        if !self
            .visible_columns
            .contains(&self.columns[self.filter_column])
        {
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
        let columns = self.columns.clone();
        let offset = self.offset;
        let criteria = self.search_criteria();
        self.next_query_id = self.next_query_id.wrapping_add(1);
        self.active_query_id = self.next_query_id;
        self.loading = true;
        if self
            .query_worker
            .requests
            .send(QueryRequest {
                id: self.active_query_id,
                path,
                criteria,
                columns,
                offset,
            })
            .is_err()
        {
            self.loading = false;
            self.flash = Some(("Query worker stopped".into(), Instant::now()));
        }
    }

    fn check_query_result(&mut self) {
        loop {
            match self.query_worker.responses.try_recv() {
                Ok(response) if response.id != self.active_query_id => continue,
                Ok(QueryResponse {
                    result: Ok(result), ..
                }) => {
                    self.rows = result.rows;
                    self.total_matches = result.total_matches;
                    self.loading = false;
                    if self.selected >= self.rows.len() && !self.rows.is_empty() {
                        self.selected = self.rows.len() - 1;
                    }
                    let visible = self.table_height.saturating_sub(1);
                    if visible > 0 && !self.rows.is_empty() {
                        if self.selected >= self.scroll_offset + visible {
                            self.scroll_offset = self.selected + 1 - visible;
                        } else if self.selected < self.scroll_offset {
                            self.scroll_offset = self.selected;
                        }
                    } else {
                        self.scroll_offset = 0;
                    }
                    break;
                }
                Ok(QueryResponse {
                    result: Err(error), ..
                }) => {
                    self.flash = Some((format!("Query error: {error}"), Instant::now()));
                    self.loading = false;
                    break;
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.loading = false;
                    break;
                }
            }
        }
    }

    fn search_criteria(&self) -> search::SearchCriteria {
        search::SearchCriteria {
            filters: self
                .filters
                .iter()
                .filter(|(_, values)| !values.is_empty())
                .map(|(column, values)| (column.clone(), values.clone()))
                .collect(),
            exclude_empty: self.exclude_empty.clone(),
            column: (!self.search_query.is_empty())
                .then(|| self.columns.get(self.search_column).cloned())
                .flatten(),
            text: self.search_query.clone(),
        }
    }

    fn check_filter_suggestions(&mut self) {
        let Some(receiver) = &self.filter_suggestions_rx else {
            return;
        };
        match receiver.try_recv() {
            Ok(Ok(values)) => {
                self.filter_suggestions = values;
                self.filter_suggestions_rx = None;
                self.filter_values_loading = false;
                if self.mode == Mode::Filter {
                    self.refresh_popup_matches_for(PopupSource::FilterValues);
                }
            }
            Ok(Err(error)) => {
                self.filter_suggestions_rx = None;
                self.filter_values_loading = false;
                self.flash = Some((format!("Filter values error: {error}"), Instant::now()));
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                self.filter_suggestions_rx = None;
                self.filter_values_loading = false;
            }
        }
    }

    fn check_export_result(&mut self) {
        let Some(receiver) = &self.export_rx else {
            return;
        };
        match receiver.try_recv() {
            Ok((path, Ok(count))) => {
                self.export_rx = None;
                self.exporting = false;
                self.flash = Some((
                    format!("Exported {count} rows to {}", path.display()),
                    Instant::now(),
                ));
            }
            Ok((_, Err(error))) => {
                self.export_rx = None;
                self.exporting = false;
                self.flash = Some((format!("Export error: {error}"), Instant::now()));
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                self.export_rx = None;
                self.exporting = false;
            }
        }
    }

    fn has_next_page(&self) -> bool {
        (self.offset + PAGE_SIZE) < self.total_matches as u32
    }

    fn has_prev_page(&self) -> bool {
        self.offset >= PAGE_SIZE
    }

    fn next_page(&mut self) {
        if self.has_next_page() {
            self.offset += PAGE_SIZE;
            self.selected = 0;
            self.scroll_offset = 0;
            self.preview_scroll = 0;
            self.execute_query();
        }
    }

    fn prev_page(&mut self) {
        if self.has_prev_page() {
            self.offset -= PAGE_SIZE;
            self.selected = 0;
            self.scroll_offset = 0;
            self.preview_scroll = 0;
            self.execute_query();
        }
    }

    fn prev_page_to_bottom(&mut self) {
        if self.has_prev_page() {
            self.offset -= PAGE_SIZE;
            self.selected = usize::MAX; // clamped after query loads
            self.preview_scroll = 0;
            self.execute_query();
        }
    }

    fn goto_first(&mut self) {
        self.preview_scroll = 0;
        if self.offset != 0 {
            self.offset = 0;
            self.selected = 0;
            self.scroll_offset = 0;
            self.execute_query();
        } else {
            self.selected = 0;
            self.scroll_offset = 0;
        }
    }

    fn goto_last(&mut self) {
        if self.total_matches == 0 {
            return;
        }
        let total = self.total_matches as u32;
        let last_page_offset = ((total - 1) / PAGE_SIZE) * PAGE_SIZE;
        self.preview_scroll = 0;
        if last_page_offset != self.offset {
            self.offset = last_page_offset;
            self.selected = usize::MAX; // clamped after query loads
            self.execute_query();
        } else if !self.rows.is_empty() {
            self.selected = self.rows.len() - 1;
            let visible = self.table_height.saturating_sub(1);
            self.scroll_offset = if visible > 0 && self.selected >= visible {
                self.selected + 1 - visible
            } else {
                0
            };
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
            PopupSource::FilterValues => picker::rank(
                &mut self.matcher,
                &self.popup_query,
                &self.filter_suggestions,
            ),
            PopupSource::Columns => {
                picker::rank(&mut self.matcher, &self.popup_query, &self.columns)
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
        input::insert(&mut self.popup_query, &mut self.popup_query_cursor, c);
        self.reset_popup_cursor();
        self.refresh_popup_matches_current();
    }

    fn popup_backspace(&mut self) {
        if self.popup_query_cursor > 0 {
            input::backspace(&mut self.popup_query, &mut self.popup_query_cursor);
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
            self.filter_values_loading = false;
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
        self.filter_suggestions.clear();
        self.filter_values_loading = true;
        let (sender, receiver) = mpsc::channel();
        self.filter_suggestions_rx = Some(receiver);
        std::thread::spawn(move || {
            let result = search::unique_values(&file, &col_name, &other_filters, 500);
            let _ = sender.send(result);
        });
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
        self.picker_is_recent.clear();
        picker::walk_parquet_files(
            &self.picker_root,
            &self.picker_root,
            PICKER_MAX_DEPTH,
            &mut self.picker_paths,
            &mut self.picker_strs,
        );
        // Stable alphabetical baseline for empty query
        let mut order: Vec<usize> = (0..self.picker_strs.len()).collect();
        order.sort_by(|a, b| self.picker_strs[*a].cmp(&self.picker_strs[*b]));
        let paths: Vec<PathBuf> = order
            .iter()
            .map(|i| self.picker_paths[*i].clone())
            .collect();
        let strs: Vec<String> = order.iter().map(|i| self.picker_strs[*i].clone()).collect();
        self.picker_paths = paths;
        self.picker_strs = strs;
        self.picker_is_recent = picker::prepend_recents(
            &mut self.picker_paths,
            &mut self.picker_strs,
            recent::load(),
        );
        self.picker_query.clear();
        self.picker_cursor = 0;
        self.picker_idx = 0;
        self.picker_scroll = 0;
        self.refresh_picker_matches();
        self.mode = Mode::FilePicker;
    }

    fn refresh_picker_matches(&mut self) {
        self.picker_matches =
            picker::rank(&mut self.matcher, &self.picker_query, &self.picker_strs);
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
                if let Some(path) = &self.file {
                    recent::record(path);
                }
                self.mode = Mode::Browse;
            }
            Err(e) => {
                self.flash = Some((format!("Load error: {}", e), Instant::now()));
            }
        }
    }

    fn load_file(&mut self, path: PathBuf) -> Result<()> {
        let path = path.canonicalize()?;
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
        self.reset_results();
        self.execute_query();
        Ok(())
    }
}

enum PopupSource {
    FilterValues,
    Columns,
}

pub(crate) fn run(file: Option<PathBuf>, config: &crate::config::Config) -> Result<()> {
    let (themes, selected_theme) = theme::configured_themes(config);
    let mut app = App::with_themes(themes, selected_theme);

    if let Some(path) = file {
        app.load_file(path)?;
        if let Some(path) = &app.file {
            recent::record(path);
        }
    } else {
        app.enter_file_picker();
    }

    let _terminal_session = terminal_session::Session::enter()?;

    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;

    loop {
        app.check_query_result();
        app.check_filter_suggestions();
        app.check_export_result();

        terminal.draw(|f| render::draw(f, &mut app))?;

        let timeout = if app.loading || app.exporting || app.filter_suggestions_rx.is_some() {
            Duration::from_millis(50)
        } else if app.flash.is_some() {
            Duration::from_millis(100)
        } else {
            Duration::from_secs(60)
        };

        if !event::poll(timeout)? {
            if let Some((_, time)) = &app.flash
                && time.elapsed() > Duration::from_secs(2)
            {
                app.flash = None;
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
            if app.exporting {
                app.flash = Some((
                    "Export in progress; wait before quitting".into(),
                    Instant::now(),
                ));
                continue;
            }
            break;
        }

        match app.mode {
            Mode::Browse => match key.code {
                KeyCode::Char('q') | KeyCode::Esc if !app.exporting => break,
                KeyCode::Char('q') | KeyCode::Esc => {
                    app.flash = Some((
                        "Export in progress; wait before quitting".into(),
                        Instant::now(),
                    ));
                }
                KeyCode::Char('o') => {
                    app.enter_file_picker();
                }
                KeyCode::Char('/') | KeyCode::Char('s') if app.file.is_some() => {
                    app.search_column = app.filter_column;
                    app.mode = Mode::Search;
                }
                KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    let page = app.table_height.saturating_sub(2);
                    let max = app.rows.len().saturating_sub(1);
                    if app.selected < max {
                        app.selected = (app.selected + page).min(max);
                        app.scroll_offset = (app.scroll_offset + page).min(max);
                        app.preview_scroll = 0;
                    } else if app.has_next_page() {
                        app.next_page();
                    }
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
                    } else if app.has_prev_page() {
                        app.prev_page_to_bottom();
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
                    } else if app.has_next_page() {
                        app.next_page();
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
                    if app.selected > 0 {
                        app.selected = app.selected.saturating_sub(page);
                        app.scroll_offset = app.scroll_offset.saturating_sub(page);
                        app.preview_scroll = 0;
                    } else if app.has_prev_page() {
                        app.prev_page_to_bottom();
                    }
                }
                KeyCode::Char('p') => app.prev_page(),
                KeyCode::Tab => {
                    app.show_preview = !app.show_preview;
                }
                KeyCode::Enter if app.show_preview => {
                    app.preview_column = Some(app.filter_column);
                    app.preview_scroll = 0;
                }
                KeyCode::Char('J') => {
                    app.preview_scroll += 1;
                }
                KeyCode::Char('K') => {
                    app.preview_scroll = app.preview_scroll.saturating_sub(1);
                }
                KeyCode::Char('g') => app.goto_first(),
                KeyCode::Char('G') => app.goto_last(),
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
                    app.enter_theme_picker();
                }
                KeyCode::Char('w') => {
                    if app.exporting {
                        app.flash = Some(("Export already in progress".into(), Instant::now()));
                        continue;
                    }
                    let Some(file) = app.file.clone() else {
                        continue;
                    };
                    let stem = file
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("output");
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
                KeyCode::BackTab if !app.columns.is_empty() => {
                    app.search_column = if app.search_column == 0 {
                        app.columns.len() - 1
                    } else {
                        app.search_column - 1
                    };
                }
                KeyCode::Tab if !app.columns.is_empty() => {
                    app.search_column = (app.search_column + 1) % app.columns.len();
                }
                KeyCode::Backspace => {
                    input::backspace(&mut app.search_query, &mut app.search_cursor);
                }
                KeyCode::Left => {
                    input::move_left(&app.search_query, &mut app.search_cursor);
                }
                KeyCode::Right => {
                    input::move_right(&app.search_query, &mut app.search_cursor);
                }
                KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    app.search_query.clear();
                    app.search_cursor = 0;
                }
                KeyCode::Char(c) => {
                    input::insert(&mut app.search_query, &mut app.search_cursor, c);
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
                    if path.exists() {
                        app.flash = Some((
                            format!("Export path already exists: {}", path.display()),
                            Instant::now(),
                        ));
                        app.mode = Mode::Browse;
                        continue;
                    }
                    let criteria = app.search_criteria();
                    let visible = app.display_columns();
                    let (sender, receiver) = mpsc::channel();
                    app.export_rx = Some(receiver);
                    app.exporting = true;
                    std::thread::spawn(move || {
                        let result = search::export(&file, &path, &criteria, &visible);
                        let _ = sender.send((path, result));
                    });
                    app.mode = Mode::Browse;
                }
                KeyCode::Backspace => {
                    input::backspace(&mut app.export_path, &mut app.export_cursor);
                }
                KeyCode::Left => {
                    input::move_left(&app.export_path, &mut app.export_cursor);
                }
                KeyCode::Right => {
                    input::move_right(&app.export_path, &mut app.export_cursor);
                }
                KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    app.export_path.clear();
                    app.export_cursor = 0;
                }
                KeyCode::Char(c) => {
                    input::insert(&mut app.export_path, &mut app.export_cursor, c);
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
                KeyCode::Up if app.picker_idx > 0 => {
                    app.picker_idx -= 1;
                }
                KeyCode::Down if app.picker_idx + 1 < app.picker_matches.len() => {
                    app.picker_idx += 1;
                }
                KeyCode::Char('p')
                    if key.modifiers.contains(KeyModifiers::CONTROL) && app.picker_idx > 0 =>
                {
                    app.picker_idx -= 1;
                }
                KeyCode::Char('n')
                    if key.modifiers.contains(KeyModifiers::CONTROL)
                        && app.picker_idx + 1 < app.picker_matches.len() =>
                {
                    app.picker_idx += 1;
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
                    input::backspace(&mut app.picker_query, &mut app.picker_cursor);
                    app.refresh_picker_matches();
                }
                KeyCode::Left => {
                    input::move_left(&app.picker_query, &mut app.picker_cursor);
                }
                KeyCode::Right => {
                    input::move_right(&app.picker_query, &mut app.picker_cursor);
                }
                KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    app.picker_query.clear();
                    app.picker_cursor = 0;
                    app.refresh_picker_matches();
                }
                KeyCode::Char(c) => {
                    input::insert(&mut app.picker_query, &mut app.picker_cursor, c);
                    app.refresh_picker_matches();
                }
                _ => {}
            },

            Mode::ThemePicker => match key.code {
                KeyCode::Esc => app.cancel_theme_picker(),
                KeyCode::Enter => app.confirm_theme(),
                KeyCode::Home => app.preview_theme(0),
                KeyCode::End if !app.themes.is_empty() => {
                    app.preview_theme(app.themes.len() - 1);
                }
                KeyCode::PageUp => {
                    let page = app.popup_visible_height.max(1);
                    app.preview_theme(app.theme_idx.saturating_sub(page));
                }
                KeyCode::PageDown if !app.themes.is_empty() => {
                    let page = app.popup_visible_height.max(1);
                    app.preview_theme((app.theme_idx + page).min(app.themes.len() - 1));
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    let index = if app.theme_idx == 0 {
                        app.themes.len().saturating_sub(1)
                    } else {
                        app.theme_idx - 1
                    };
                    app.preview_theme(index);
                }
                KeyCode::Down | KeyCode::Char('j') if !app.themes.is_empty() => {
                    app.preview_theme((app.theme_idx + 1) % app.themes.len());
                }
                _ => {}
            },
        }
    }

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
                input::move_left(&app.popup_query, &mut app.popup_query_cursor);
            }
            KeyCode::Right => {
                input::move_right(&app.popup_query, &mut app.popup_query_cursor);
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
                input::move_left(&app.popup_query, &mut app.popup_query_cursor);
            }
            KeyCode::Right => {
                input::move_right(&app.popup_query, &mut app.popup_query_cursor);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_picker_cancel_restores_original_theme() {
        let default = theme::default_theme();
        let mut alternate = default;
        alternate.accent = Color::Red;
        let mut app = App::with_themes(
            vec![("default".into(), default), ("alternate".into(), alternate)],
            0,
        );
        let original = app.theme;

        app.enter_theme_picker();
        app.preview_theme(1);
        assert_ne!(app.theme, original);
        app.cancel_theme_picker();

        assert_eq!(app.theme, original);
        assert_eq!(app.mode, Mode::Browse);
    }

    #[test]
    fn theme_picker_confirm_keeps_previewed_theme() {
        let default = theme::default_theme();
        let mut alternate = default;
        alternate.accent = Color::Red;
        let mut app = App::with_themes(
            vec![("default".into(), default), ("alternate".into(), alternate)],
            0,
        );

        app.enter_theme_picker();
        app.preview_theme(1);
        let previewed = app.theme;
        app.confirm_theme();

        assert_eq!(app.theme, previewed);
        assert_eq!(app.mode, Mode::Browse);
    }
}
