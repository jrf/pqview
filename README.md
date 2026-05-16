# pqview

A TUI for searching and filtering large Parquet files. Built for exploring text-heavy datasets without loading everything into memory.

Uses Polars lazy scanning to push filters and searches down to the Parquet reader, and it handles multi-GB files without breaking a sweat.

## Install

```
just install
```

Or manually:

```
cargo build --release
cp target/release/pqview ~/.cargo/bin/
```

## Usage

```
pqview [file.parquet]
```

If no file is given, the app opens a fuzzy file picker rooted at the current directory (recursive, depth 6, skips hidden / `target` / `node_modules`).

## Keybindings

### Browse mode

| Key | Action |
|-----|--------|
| `h` / `l` | Move column focus |
| `j` / `k` | Navigate rows |
| `f` | Open filter for focused column |
| `/` or `s` | Search focused column |
| `o` | Open another Parquet file (fuzzy picker) |
| `n` / `p` | Next / previous page |
| `Enter` | Set preview to focused column |
| `J` / `K` | Scroll preview pane |
| `Tab` | Toggle preview pane |
| `C` | Clear all filters and search |
| `q` / `Esc` | Quit |

### File picker

Powered by [nucleo](https://crates.io/crates/nucleo-matcher).

| Key | Action |
|-----|--------|
| Type | Fuzzy-filter candidates |
| `Up` / `Down` (or `Ctrl+P` / `Ctrl+N`) | Move selection |
| `Ctrl+F` / `Ctrl+B` | Page down / up |
| `Enter` | Open file |
| `Ctrl+U` | Clear query |
| `Esc` | Cancel (or quit if no file loaded yet) |

### Filter mode

Filters subset the data by exact value (e.g. filter by gender, department). Multiple values can be selected per column (OR within column, AND across columns).

| Key | Action |
|-----|--------|
| `j` / `k` | Navigate values |
| `Ctrl+F` / `Ctrl+B` | Page down / up |
| `Space` / `Enter` | Toggle value (takes effect immediately) |
| `/` | Start fuzzy search across values |
| `Ctrl+U` | Clear fuzzy query |
| `Esc` / `f` | Close filter |

Inside fuzzy search: type to filter (nucleo-ranked), `↑` / `↓` (or `Ctrl+P` / `Ctrl+N`) to navigate, `Enter` or `Tab` to toggle, `Esc` returns to nav (keeping the filtered view).

### Columns mode

| Key | Action |
|-----|--------|
| `j` / `k` | Navigate |
| `Ctrl+F` / `Ctrl+B` | Page down / up |
| `Space` / `Enter` | Toggle column visibility |
| `a` / `d` | Show all / hide all (within current matches) |
| `/` | Start fuzzy search across columns |
| `Esc` / `v` | Close |

Inside fuzzy search: typing filters the list; `Ctrl+A` / `Ctrl+D` show/hide all *visible* matches (so you can fuzz "id" and bulk-toggle); other bindings mirror filter mode.

### Search mode

Search does case-insensitive substring matching within the filtered results. Matches are highlighted in the preview pane.

| Key | Action |
|-----|--------|
| Type | Enter search text |
| `Tab` / `Shift+Tab` | Change search column |
| `Enter` | Execute search |
| `Ctrl+U` | Clear search text |
| `Esc` | Cancel |

## Architecture

- **Polars LazyFrame** for all data access -- filters and searches are pushed down before scanning
- **Ratatui** + **crossterm** for the terminal interface
- Background queries via `std::sync::mpsc` to keep the UI responsive
- Pagination (200 rows per page) to avoid loading large result sets into memory
