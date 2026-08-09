# pqview

A TUI for searching and filtering large Parquet files. Built for exploring text-heavy datasets with paginated reads and streaming exports.

Uses Polars lazy scanning to push filters and searches down to the Parquet reader.

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
| `g` / `G` | First / last result |
| `Ctrl+F` / `Ctrl+B` | Move by one visible page, crossing result pages |
| `Enter` | Set preview to focused column |
| `J` / `K` | Scroll preview pane |
| `Tab` | Toggle preview pane |
| `x` | Exclude null and empty values in the focused column |
| `v` | Choose visible columns |
| `w` | Export filtered rows to Parquet |
| `t` | Open the theme picker (live preview) |
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

### Theme picker

| Key | Action |
|-----|--------|
| `j` / `k` or arrows | Preview themes (wraps at either end) |
| `Home` / `End` | Preview first / last theme |
| `Enter` | Apply the previewed theme for this session |
| `Esc` | Cancel and restore the previous theme |

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

### Export

Press `w` in browse mode to edit the output path, then press `Enter`. Export runs in the background, writes only visible columns, and does not overwrite an existing file.

## Themes

Without configuration, pqview offers its five built-in themes. To share themes with pdfterm and MDR, create `~/.config/pqview/config.toml` (or `$XDG_CONFIG_HOME/pqview/config.toml`):

```toml
theme = "~/.config/themes/tokyo-night-moon.toml"
theme_catalog = "~/.config/themes/catalog.toml"
```

`theme` selects the startup theme. `theme_catalog` supplies the picker entries through an explicit list; pqview does not scan the containing directory:

```toml
themes = [
  "~/.config/themes/tokyo-night-moon.toml",
  "~/.config/themes/catppuccin-mocha.toml",
]
```

Theme files use the shared pdfterm/MDR semantic schema. Values in `[ui]` refer to names from `[colors]`; omitted roles use the Tokyo Night Moon fallback palette.

```toml
[colors]
bg = "#222436"
bg_dark = "#1e2030"
blue = "#82aaff"
magenta = "#c099ff"
text = "#c8d3f5"
muted = "#636da6"

[ui]
background = "bg"
background_dark = "bg_dark"
accent = "magenta"
selection = "blue"
text = "text"
text_muted = "muted"
picker_accent = "blue"
picker_matched = "magenta"
```

## Architecture

- **Polars LazyFrame** for all data access -- filters and searches are pushed down before scanning
- **Ratatui** + **crossterm** for the terminal interface
- A coalescing background-query worker that discards stale results
- Background filter-value discovery and streaming Parquet exports
- Pagination (1,000 rows per page) to bound displayed result memory
- Scope-based terminal cleanup that restores raw mode and the alternate screen on errors
