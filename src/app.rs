use crate::render;
use crate::search;
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::ExecutableCommand;
use ratatui::prelude::*;
use std::collections::{HashMap, HashSet};
use std::io::stdout;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

const PAGE_SIZE: u32 = 200;

#[derive(Clone, PartialEq)]
pub enum Mode {
    Browse,
    Search,
    Filter,
}

pub struct App {
    pub mode: Mode,
    pub file: PathBuf,
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

    pub rows: Vec<Vec<String>>,
    pub total_matches: usize,
    pub selected: usize,
    pub offset: u32,
    pub scroll_offset: usize,
    pub preview_scroll: usize,
    pub preview_column: Option<usize>,
    pub show_preview: bool,
    pub loading: bool,
    pub flash: Option<(String, Instant)>,
    query_rx: Option<mpsc::Receiver<Result<search::SearchResult>>>,
}

impl App {
    fn new(file: PathBuf, columns: Vec<String>) -> Self {
        Self {
            mode: Mode::Browse,
            file,
            columns,
            search_query: String::new(),
            search_cursor: 0,
            search_column: 0,
            filters: HashMap::new(),
            filter_column: 0,
            filter_suggestions: Vec::new(),
            filter_selected: HashSet::new(),
            filter_cursor_idx: 0,
            filter_scroll: 0,
            rows: Vec::new(),
            total_matches: 0,
            selected: 0,
            offset: 0,
            scroll_offset: 0,
            preview_scroll: 0,
            preview_column: None,
            show_preview: true,
            loading: false,
            flash: None,
            query_rx: None,
        }
    }

    pub fn active_filters(&self) -> &HashMap<String, Vec<String>> {
        &self.filters
    }

    pub fn has_active_filters(&self) -> bool {
        self.filters.values().any(|v| !v.is_empty())
    }

    pub fn filter_display(&self, col: &str) -> Option<String> {
        self.filters.get(col).map(|vals| vals.join(", "))
    }

    pub fn search_column_name(&self) -> &str {
        &self.columns[self.search_column]
    }

    fn execute_query(&mut self) {
        let path = self.file.clone();
        let filters: HashMap<String, Vec<String>> = self
            .filters
            .iter()
            .filter(|(_, v)| !v.is_empty())
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let columns = self.columns.clone();
        let offset = self.offset;
        let search_col = self.columns[self.search_column].clone();
        let search_text = self.search_query.clone();

        self.loading = true;

        let (tx, rx) = mpsc::channel();
        self.query_rx = Some(rx);

        std::thread::spawn(move || {
            let search = if search_text.is_empty() {
                None
            } else {
                Some(search_col.as_str())
            };
            let result =
                search::query(&path, &filters, search, &search_text, &columns, PAGE_SIZE, offset);
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
        self.mode = Mode::Filter;
    }

    fn load_filter_suggestions(&mut self) {
        let col_name = self.columns[self.filter_column].clone();
        // Load unique values ignoring this column's own filter so all options are visible
        let other_filters: HashMap<String, Vec<String>> = self
            .filters
            .iter()
            .filter(|(k, v)| *k != &col_name && !v.is_empty())
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        match search::unique_values(&self.file, &col_name, &other_filters, 500) {
            Ok(values) => self.filter_suggestions = values,
            Err(_) => self.filter_suggestions = Vec::new(),
        }
    }

    fn toggle_filter_value(&mut self) {
        if self.filter_suggestions.is_empty() {
            return;
        }
        let value = self.filter_suggestions[self.filter_cursor_idx].clone();
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
}

pub fn run(file: PathBuf, columns: Vec<String>) -> Result<()> {
    let mut app = App::new(file, columns.clone());

    // Synchronous initial load so data is visible on first frame
    match search::query(
        &app.file,
        &HashMap::new(),
        None,
        "",
        &columns,
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

        terminal.draw(|f| render::draw(f, &app))?;

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
                KeyCode::Char('/') | KeyCode::Char('s') => {
                    app.search_column = app.filter_column;
                    app.mode = Mode::Search;
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
                    }
                }
                KeyCode::Left | KeyCode::Char('h') => {
                    if app.filter_column > 0 {
                        app.filter_column -= 1;
                    }
                }
                KeyCode::Right | KeyCode::Char('l') => {
                    if app.filter_column < app.columns.len() - 1 {
                        app.filter_column += 1;
                    }
                }
                KeyCode::Char('n') => app.next_page(),
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
                KeyCode::Char('C') => {
                    app.clear_all_filters();
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
                    app.search_column = if app.search_column == 0 {
                        app.columns.len() - 1
                    } else {
                        app.search_column - 1
                    };
                }
                KeyCode::Tab => {
                    app.search_column = (app.search_column + 1) % app.columns.len();
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

            Mode::Filter => match key.code {
                KeyCode::Esc | KeyCode::Char('f') => {
                    app.mode = Mode::Browse;
                }
                KeyCode::Char(' ') | KeyCode::Enter => {
                    app.toggle_filter_value();
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if app.filter_cursor_idx > 0 {
                        app.filter_cursor_idx -= 1;
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if app.filter_cursor_idx < app.filter_suggestions.len().saturating_sub(1) {
                        app.filter_cursor_idx += 1;
                    }
                }
                _ => {}
            },
        }
    }

    terminal::disable_raw_mode()?;
    tty.execute(LeaveAlternateScreen)?;
    Ok(())
}
