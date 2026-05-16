# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Run

```bash
just build          # cargo build --release
just install        # build + copy to ~/.cargo/bin/pqview
just check          # cargo check
just test           # cargo test
cargo test <name>   # run a single test by name
```

Binary name is `pqview`. Usage: `pqview <file.parquet>`

## Architecture

Three-module Rust TUI for exploring large Parquet files without loading them into memory.

- **main.rs** — CLI entry point. Parses an optional file path and hands it to `app::run`; if no file is given the app starts in the file picker.
- **app.rs** — Application state (`App` struct) and the event loop. Six modes: `Browse`, `Search`, `Filter`, `Columns`, `Export`, `FilePicker`. Handles all keybindings and manages background query lifecycle. The file picker walks the cwd (depth 6) and ranks `.parquet` paths with `nucleo-matcher`.
- **search.rs** — Polars query layer. All data access goes through `LazyFrame::scan_parquet` so filters and searches are pushed down before scanning. Two main entry points: `query()` for paginated filtered/searched results, `unique_values()` for filter suggestion lists.
- **render.rs** — Ratatui rendering. Draws filter bar, search bar, results table, preview pane, and popup overlays (filter checklist, column picker). Takes `&mut App` to write back `table_height` for scroll calculations.

### Key patterns

- **Background queries**: `App::execute_query()` spawns a thread, sends results through `mpsc::channel`. The event loop calls `check_query_result()` each frame with `try_recv()`. Initial load is synchronous so data is visible on the first frame.
- **Pagination**: 200 rows per page (`PAGE_SIZE`). Offset-based via Polars `slice()`.
- **Filter vs Search**: Filters are exact-value multi-select per column (`is_in`), applied immediately on toggle. Search is case-insensitive substring match (`str().contains()`) on a single column, applied on Enter. They compose: search runs within filtered results.
- **Column visibility**: `visible_columns: HashSet<String>` controls which columns appear in the filter bar and results table. `filter_column` indexes into the full `app.columns` list but h/l navigation skips hidden columns.
- **Popup reuse**: `draw_checklist()` is a generic checklist renderer shared by both the Filter and Columns popups. It takes `matches: &[usize]` so the same widget renders fuzzy-filtered subsets.
- **Popup fuzzy search**: Filter and Columns popups share a single piece of state (`popup_query`, `popup_searching`, `popup_matches`) and use the `rank_against` helper backed by `nucleo-matcher`. Entered with `/` (modal — `j`/`k` still nav until `/` is pressed). `popup_matches` indices point into the source list (`filter_suggestions` or `columns`), and the popup cursors (`filter_cursor_idx`, `column_picker_idx`) index into `popup_matches`, not the source.

### Polars specifics

Edition 2024. Polars 0.46 uses `collect_schema()` (not `schema()`) on LazyFrame. Required feature flags: `lazy`, `parquet`, `regex`, `strings`, `is_in`.
