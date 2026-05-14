# prview

A TUI for searching and filtering large Parquet files. Built for exploring text-heavy datasets without loading everything into memory.

Uses Polars lazy scanning to push filters and searches down to the Parquet reader, and it handles multi-GB files without breaking a sweat.

## Install

```
just install
```

Or manually:

```
cargo build --release
cp target/release/prview ~/.cargo/bin/
```

## Usage

```
prview <file.parquet>
```

## Keybindings

### Browse mode

| Key | Action |
|-----|--------|
| `h` / `l` | Move column focus |
| `j` / `k` | Navigate rows |
| `f` | Open filter for focused column |
| `/` or `s` | Search focused column |
| `n` / `p` | Next / previous page |
| `Enter` | Set preview to focused column |
| `J` / `K` | Scroll preview pane |
| `Tab` | Toggle preview pane |
| `C` | Clear all filters and search |
| `q` / `Esc` | Quit |

### Filter mode

Filters subset the data by exact value (e.g. filter by gender, department). Multiple values can be selected per column (OR within column, AND across columns).

| Key | Action |
|-----|--------|
| `j` / `k` | Navigate values |
| `Space` / `Enter` | Toggle value (takes effect immediately) |
| `Esc` / `f` | Close filter |

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
