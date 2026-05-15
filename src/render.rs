use crate::app::{App, Mode};
use ratatui::prelude::*;
use ratatui::widgets::*;
use std::collections::HashSet;
use unicode_width::UnicodeWidthStr;

const HIGHLIGHT_COLOR: Color = Color::Rgb(122, 162, 247);
const SEARCH_COLOR: Color = Color::Rgb(224, 175, 104);
const FILTER_ACTIVE_COLOR: Color = Color::Rgb(158, 206, 106);
const DIM_COLOR: Color = Color::Rgb(120, 120, 140);
const BORDER_COLOR: Color = Color::Rgb(68, 68, 100);
const SELECTED_BG: Color = Color::Rgb(40, 42, 60);
const FOCUSED_COL_BG: Color = Color::Rgb(50, 52, 72);

pub fn draw(f: &mut Frame, app: &mut App) {
    let area = f.area();

    if app.mode == Mode::Filter {
        draw_with_popup(f, app, area, DrawPopup::Filter);
        return;
    }

    if app.mode == Mode::Columns {
        draw_with_popup(f, app, area, DrawPopup::Columns);
        return;
    }

    let mut constraints = vec![
        Constraint::Length(3), // filter bar
        Constraint::Length(3), // search bar
    ];
    if app.show_preview {
        constraints.push(Constraint::Percentage(50));
        constraints.push(Constraint::Percentage(50));
    } else {
        constraints.push(Constraint::Min(1));
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    draw_filter_bar(f, app, chunks[0]);
    draw_search_bar(f, app, chunks[1]);
    draw_results_table(f, app, chunks[2]);

    if app.show_preview && chunks.len() > 3 {
        draw_preview(f, app, chunks[3]);
    }
}

enum DrawPopup {
    Filter,
    Columns,
}

fn draw_with_popup(f: &mut Frame, app: &mut App, area: Rect, popup: DrawPopup) {
    let mut constraints = vec![
        Constraint::Length(3),
        Constraint::Length(3),
    ];
    if app.show_preview {
        constraints.push(Constraint::Percentage(50));
        constraints.push(Constraint::Percentage(50));
    } else {
        constraints.push(Constraint::Min(1));
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    draw_filter_bar(f, app, chunks[0]);
    draw_search_bar(f, app, chunks[1]);
    draw_results_table(f, app, chunks[2]);

    if app.show_preview && chunks.len() > 3 {
        draw_preview(f, app, chunks[3]);
    }

    match popup {
        DrawPopup::Filter => draw_filter_popup(f, app, area),
        DrawPopup::Columns => draw_columns_popup(f, app, area),
    }
}

fn draw_filter_popup(f: &mut Frame, app: &App, area: Rect) {
    let col_name = &app.columns[app.filter_column];
    let popup_area = centered_rect(50, 70, area);
    f.render_widget(Clear, popup_area);

    let selected_count = app.filter_selected.len();
    let bottom = if selected_count > 0 {
        format!(" {} selected | Space toggle | Esc close ", selected_count)
    } else {
        " Space toggle | Esc close ".to_string()
    };

    let block = Block::default()
        .title(format!(" Filter: {} ", col_name))
        .title_style(Style::default().fg(FILTER_ACTIVE_COLOR).bold())
        .title_bottom(bottom)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(FILTER_ACTIVE_COLOR));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    if app.filter_suggestions.is_empty() {
        let msg = Paragraph::new("No values found")
            .style(Style::default().fg(DIM_COLOR))
            .alignment(Alignment::Center);
        f.render_widget(msg, inner);
        return;
    }

    draw_checklist(
        f,
        inner,
        &app.filter_suggestions,
        &app.filter_selected,
        app.filter_cursor_idx,
        FILTER_ACTIVE_COLOR,
    );
}

fn draw_columns_popup(f: &mut Frame, app: &App, area: Rect) {
    let popup_area = centered_rect(50, 70, area);
    f.render_widget(Clear, popup_area);

    let visible_count = app.visible_columns.len();
    let bottom = format!(
        " {}/{} shown | Space toggle | a all | d none | Esc close ",
        visible_count,
        app.columns.len()
    );

    let block = Block::default()
        .title(" Columns ")
        .title_style(Style::default().fg(HIGHLIGHT_COLOR).bold())
        .title_bottom(bottom)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(HIGHLIGHT_COLOR));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    draw_checklist(
        f,
        inner,
        &app.columns,
        &app.visible_columns,
        app.column_picker_idx,
        HIGHLIGHT_COLOR,
    );
}

fn draw_checklist(
    f: &mut Frame,
    area: Rect,
    items: &[String],
    selected: &HashSet<String>,
    cursor: usize,
    active_color: Color,
) {
    let visible_height = area.height as usize;
    let scroll = if cursor >= visible_height {
        cursor - visible_height + 1
    } else {
        0
    };

    let list_items: Vec<ListItem> = items
        .iter()
        .enumerate()
        .skip(scroll)
        .take(visible_height)
        .map(|(i, val)| {
            let is_selected = selected.contains(val);
            let is_cursor = i == cursor;

            let checkbox = if is_selected { "[x] " } else { "[ ] " };
            let text = format!("{}{}", checkbox, val);

            let style = if is_cursor && is_selected {
                Style::default()
                    .fg(active_color)
                    .bg(FOCUSED_COL_BG)
                    .bold()
            } else if is_cursor {
                Style::default()
                    .fg(Color::White)
                    .bg(FOCUSED_COL_BG)
                    .bold()
            } else if is_selected {
                Style::default().fg(active_color)
            } else {
                Style::default().fg(DIM_COLOR)
            };

            ListItem::new(text).style(style)
        })
        .collect();

    f.render_widget(List::new(list_items), area);
}

fn draw_filter_bar(f: &mut Frame, app: &App, area: Rect) {
    let filter_count = app
        .active_filters()
        .values()
        .filter(|v| !v.is_empty())
        .count()
        + app.exclude_empty.len();

    let border_style = if app.mode == Mode::Filter {
        Style::default().fg(FILTER_ACTIVE_COLOR)
    } else {
        Style::default().fg(BORDER_COLOR)
    };

    let title = if filter_count > 0 {
        format!(" Filters ({} active) ", filter_count)
    } else {
        " Filters — h/l select column, f to filter ".to_string()
    };

    let block = Block::default()
        .title(title)
        .title_style(Style::default().fg(if filter_count > 0 {
            FILTER_ACTIVE_COLOR
        } else {
            DIM_COLOR
        }))
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.width < 4 || inner.height < 1 {
        return;
    }

    let visible_cols = app.display_columns();
    let mut spans: Vec<Span> = Vec::new();
    for (vi, col) in visible_cols.iter().enumerate() {
        if vi > 0 {
            spans.push(Span::styled(" | ", Style::default().fg(BORDER_COLOR)));
        }

        let all_idx = app.columns.iter().position(|c| c == col).unwrap_or(0);
        let filter_display = app.filter_display(col);
        let is_focused = all_idx == app.filter_column;
        let is_search_col =
            all_idx == app.search_column && !app.search_query.is_empty();
        let is_excluding_empty = app.exclude_empty.contains(col);

        let display = match (&filter_display, is_excluding_empty) {
            (Some(vals), true) => format!("{}={} !null", col, vals),
            (Some(vals), false) => format!("{}={}", col, vals),
            (None, true) => format!("{} !null", col),
            (None, false) => col.clone(),
        };

        let fg = if is_search_col {
            SEARCH_COLOR
        } else if filter_display.is_some() || is_excluding_empty {
            FILTER_ACTIVE_COLOR
        } else if is_focused {
            HIGHLIGHT_COLOR
        } else {
            DIM_COLOR
        };

        let style = if is_focused {
            Style::default().fg(fg).bg(FOCUSED_COL_BG).bold()
        } else {
            Style::default().fg(fg)
        };

        spans.push(Span::styled(display, style));
    }

    let line = Line::from(spans);
    f.render_widget(
        Paragraph::new(line).scroll((0, compute_filter_scroll(app, inner.width))),
        inner,
    );
}

fn draw_search_bar(f: &mut Frame, app: &App, area: Rect) {
    let is_searching = app.mode == Mode::Search;

    let border_style = if is_searching {
        Style::default().fg(SEARCH_COLOR)
    } else {
        Style::default().fg(BORDER_COLOR)
    };

    let col_name = app.search_column_name();
    let title = if !app.search_query.is_empty() {
        format!(" Search: {} ", col_name)
    } else if is_searching {
        format!(" Search: {} (Tab to change column) ", col_name)
    } else {
        " Search — / to search ".to_string()
    };

    let block = Block::default()
        .title(title)
        .title_style(Style::default().fg(if is_searching || !app.search_query.is_empty() {
            SEARCH_COLOR
        } else {
            DIM_COLOR
        }))
        .borders(Borders::ALL)
        .border_style(border_style);

    let input_text = if app.search_query.is_empty() && !is_searching {
        Span::styled("", Style::default().fg(DIM_COLOR))
    } else {
        Span::styled(&app.search_query, Style::default().fg(Color::White))
    };

    let paragraph = Paragraph::new(Line::from(vec![input_text])).block(block);
    f.render_widget(paragraph, area);

    if is_searching {
        f.set_cursor_position((area.x + 1 + app.search_cursor as u16, area.y + 1));
    }
}

fn filter_col_width(app: &App, col: &str) -> usize {
    let base = match app.filter_display(col) {
        Some(vals) => col.len() + 1 + vals.len(),
        None => col.len(),
    };
    if app.exclude_empty.contains(col) {
        base + 6 // " !null"
    } else {
        base
    }
}

fn compute_filter_scroll(app: &App, visible_width: u16) -> u16 {
    let visible_cols = app.display_columns();
    let total_width: usize = visible_cols
        .iter()
        .map(|c| filter_col_width(app, c))
        .sum::<usize>()
        + visible_cols.len().saturating_sub(1) * 3;

    if total_width <= visible_width as usize {
        return 0;
    }

    let focused_col = app.columns.get(app.filter_column).map(|s| s.as_str());
    let mut offset = 0u16;
    for col in &visible_cols {
        if Some(col.as_str()) == focused_col {
            break;
        }
        offset += filter_col_width(app, col) as u16 + 3;
    }
    offset.saturating_sub(visible_width / 3)
}

fn draw_results_table(f: &mut Frame, app: &mut App, area: Rect) {
    let status = if app.loading {
        " loading... ".to_string()
    } else {
        let page_start = app.offset as usize + 1;
        let page_end = (app.offset as usize + app.rows.len()).min(app.total_matches);
        if app.total_matches > 0 {
            format!(" {}-{} of {} ", page_start, page_end, app.total_matches)
        } else if app.has_active_filters() || !app.search_query.is_empty() {
            " no matches ".to_string()
        } else {
            " no data ".to_string()
        }
    };

    let block = Block::default()
        .title(" Results ")
        .title_style(Style::default().fg(HIGHLIGHT_COLOR))
        .title_bottom(status)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(BORDER_COLOR));

    if app.rows.is_empty() {
        let empty_msg = if app.loading {
            "Searching..."
        } else if app.has_active_filters() || !app.search_query.is_empty() {
            "No results match current filters/search"
        } else {
            "No data"
        };
        let paragraph = Paragraph::new(empty_msg)
            .style(Style::default().fg(DIM_COLOR))
            .block(block)
            .alignment(Alignment::Center);
        f.render_widget(paragraph, area);
        return;
    }

    // header(1) + border(2) = 3 lines of overhead
    let visible_rows = area.height.saturating_sub(3) as usize;
    app.table_height = visible_rows;

    let visible_columns = compute_visible_columns(app, area.width.saturating_sub(2) as usize);

    let header_cells: Vec<Cell> = visible_columns
        .iter()
        .map(|(name, _)| {
            let has_filter = app
                .active_filters()
                .get(name)
                .is_some_and(|v| !v.is_empty());
            let is_search_col = name == app.search_column_name() && !app.search_query.is_empty();
            let style = if has_filter && is_search_col {
                Style::default().fg(FILTER_ACTIVE_COLOR).bold().underlined()
            } else if has_filter {
                Style::default().fg(FILTER_ACTIVE_COLOR).bold()
            } else if is_search_col {
                Style::default().fg(SEARCH_COLOR).bold()
            } else {
                Style::default().fg(HIGHLIGHT_COLOR).bold()
            };
            Cell::from(name.as_str()).style(style)
        })
        .collect();
    let header = Row::new(header_cells).height(1);

    let rows: Vec<Row> = app
        .rows
        .iter()
        .enumerate()
        .skip(app.scroll_offset)
        .take(visible_rows)
        .map(|(i, row)| {
            let style = if i == app.selected {
                Style::default().bg(SELECTED_BG).fg(Color::White)
            } else {
                Style::default().fg(Color::White)
            };

            let cells: Vec<Cell> = visible_columns
                .iter()
                .map(|(name, width)| {
                    let col_idx = app.columns.iter().position(|c| c == name).unwrap_or(0);
                    let val = row.get(col_idx).map(|s| s.as_str()).unwrap_or("");
                    let truncated = truncate_str(val, *width);
                    Cell::from(truncated)
                })
                .collect();

            Row::new(cells).style(style)
        })
        .collect();

    let widths: Vec<Constraint> = visible_columns
        .iter()
        .map(|(_, w)| Constraint::Length(*w as u16))
        .collect();

    let table = Table::new(rows, widths)
        .header(header)
        .block(block)
        .row_highlight_style(Style::default().bg(SELECTED_BG));

    f.render_widget(table, area);

    let help =
        " j/k nav | h/l column | f filter | / search | x !null | v columns | n/p page | Tab preview | C clear | q quit ";
    let help_span = Span::styled(help, Style::default().fg(DIM_COLOR));
    let help_area = Rect::new(area.x + 1, area.bottom() - 1, help.width() as u16, 1);
    if help_area.right() < area.right() {
        f.render_widget(Paragraph::new(help_span), help_area);
    }
}

fn draw_preview(f: &mut Frame, app: &App, area: Rect) {
    let preview_col = app.preview_column.unwrap_or(app.filter_column);
    let col_name = app
        .columns
        .get(preview_col)
        .map(|s| s.as_str())
        .unwrap_or("?");

    let block = Block::default()
        .title(format!(" Preview: {} ", col_name))
        .title_style(Style::default().fg(HIGHLIGHT_COLOR))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(BORDER_COLOR));

    if app.rows.is_empty() || app.selected >= app.rows.len() {
        let paragraph = Paragraph::new("No row selected")
            .style(Style::default().fg(DIM_COLOR))
            .block(block);
        f.render_widget(paragraph, area);
        return;
    }

    let row = &app.rows[app.selected];
    let cell_text = row.get(preview_col).map(|s| s.as_str()).unwrap_or("");

    let inner_width = area.width.saturating_sub(2) as usize;
    let wrapped = textwrap::fill(cell_text, inner_width.max(20));

    let is_search_col = preview_col == app.search_column && !app.search_query.is_empty();

    let lines: Vec<Line> = wrapped
        .lines()
        .skip(app.preview_scroll)
        .map(|l| {
            if is_search_col {
                highlight_matches(l, &app.search_query)
            } else {
                Line::from(l.to_string())
            }
        })
        .collect();

    let paragraph = Paragraph::new(lines).block(block).wrap(Wrap { trim: false });
    f.render_widget(paragraph, area);
}

fn highlight_matches<'a>(text: &'a str, query: &str) -> Line<'a> {
    let lower_text = text.to_lowercase();
    let lower_query = query.to_lowercase();
    let mut spans: Vec<Span<'a>> = Vec::new();
    let mut last_end = 0;

    for (start, _) in lower_text.match_indices(&lower_query) {
        if start > last_end {
            spans.push(Span::styled(
                text[last_end..start].to_string(),
                Style::default().fg(Color::White),
            ));
        }
        spans.push(Span::styled(
            text[start..start + query.len()].to_string(),
            Style::default().fg(Color::Black).bg(SEARCH_COLOR),
        ));
        last_end = start + query.len();
    }

    if last_end < text.len() {
        spans.push(Span::styled(
            text[last_end..].to_string(),
            Style::default().fg(Color::White),
        ));
    }

    Line::from(spans)
}

fn compute_visible_columns(app: &App, available_width: usize) -> Vec<(String, usize)> {
    let cols = app.display_columns();
    let num_cols = cols.len();
    if num_cols == 0 {
        return Vec::new();
    }

    let min_col_width: usize = 12;
    let separator_space = num_cols.saturating_sub(1);
    let total_min = num_cols * min_col_width + separator_space;

    if available_width < min_col_width {
        return vec![(cols[0].clone(), available_width)];
    }

    if available_width < total_min {
        let fit_count = (available_width + 1) / (min_col_width + 1);
        let fit_count = fit_count.max(1);
        let per_col = available_width / fit_count.max(1);
        return cols[..fit_count.min(num_cols)]
            .iter()
            .map(|name| (name.clone(), per_col))
            .collect();
    }

    let extra = available_width - total_min;
    let per_col_extra = extra / num_cols;

    cols.iter()
        .map(|name| (name.clone(), min_col_width + per_col_extra))
        .collect()
}

fn truncate_str(s: &str, max_width: usize) -> String {
    if s.width() <= max_width {
        return s.to_string();
    }
    let mut width = 0;
    let mut result = String::new();
    for c in s.chars() {
        let cw = unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
        if width + cw + 1 > max_width {
            result.push('…');
            break;
        }
        result.push(c);
        width += cw;
    }
    result
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
