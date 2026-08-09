use crate::app::{App, Mode};
use crate::input;
use crate::picker;
use crate::theme::Theme;
use ratatui::prelude::*;
use ratatui::widgets::*;
use std::collections::HashSet;
use unicode_width::UnicodeWidthStr;

pub fn draw(f: &mut Frame, app: &mut App) {
    let area = f.area();
    f.render_widget(
        Block::default().style(Style::default().bg(app.theme.background)),
        area,
    );

    if app.mode == Mode::Filter {
        draw_with_popup(f, app, area, DrawPopup::Filter);
        return;
    }

    if app.mode == Mode::Columns {
        draw_with_popup(f, app, area, DrawPopup::Columns);
        return;
    }

    if app.mode == Mode::FilePicker {
        draw_file_picker_popup(f, app, area);
        return;
    }

    if app.mode == Mode::ThemePicker {
        draw_theme_picker_popup(f, app, area);
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

    if app.mode == Mode::Export {
        draw_export_bar(f, app, area);
    }
}

enum DrawPopup {
    Filter,
    Columns,
}

fn draw_with_popup(f: &mut Frame, app: &mut App, area: Rect, popup: DrawPopup) {
    let mut constraints = vec![Constraint::Length(3), Constraint::Length(3)];
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

fn draw_file_picker_popup(f: &mut Frame, app: &mut App, area: Rect) {
    let t = &app.theme;
    let popup_area = picker_rect(area);
    let surface = t.background;
    let chrome = t.background_dark;
    let selection = t.cursor_bg;

    f.render_widget(
        Block::default().style(Style::default().bg(t.background_deep)),
        area,
    );
    f.render_widget(Clear, popup_area);

    let title = format!(" {} ", shorten_path(&app.picker_root.display().to_string()));

    let block = Block::default()
        .title(title)
        .style(Style::default().bg(surface).fg(t.text))
        .title_style(Style::default().fg(t.picker_accent).bg(surface).bold())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(t.picker_border));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    if inner.height < 3 || inner.width < 4 {
        return;
    }

    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .split(inner);

    let filter_line = if app.picker_query.is_empty() {
        Line::from(Span::styled(
            " type to filter...",
            Style::default().fg(t.text_dim).bg(chrome),
        ))
    } else {
        Line::from(vec![
            Span::styled(" > ", Style::default().fg(t.picker_accent).bg(chrome)),
            Span::styled(
                app.picker_query.as_str(),
                Style::default().fg(t.text).bg(chrome),
            ),
        ])
    };
    f.render_widget(
        Paragraph::new(filter_line).style(Style::default().bg(chrome)),
        rows[0],
    );

    if !app.picker_query.is_empty() {
        f.set_cursor_position((
            rows[0].x + 3 + input::display_width(&app.picker_query, app.picker_cursor),
            rows[0].y,
        ));
    }

    let visible_height = rows[1].height as usize;
    app.popup_visible_height = visible_height;
    let recent_heading_index = if app.picker_query.is_empty() {
        app.picker_matches.iter().position(|candidate_index| {
            app.picker_is_recent
                .get(*candidate_index)
                .copied()
                .unwrap_or(false)
        })
    } else {
        None
    };
    let mut scroll = 0;
    while scroll < app.picker_idx {
        let entries = app.picker_idx - scroll + 1;
        let includes_heading =
            recent_heading_index.is_some_and(|index| index >= scroll && index <= app.picker_idx);
        if entries + usize::from(includes_heading) <= visible_height.max(1) {
            break;
        }
        scroll += 1;
    }

    app.picker_scroll = scroll;
    let mut lines: Vec<Line> = Vec::with_capacity(visible_height);
    for (position, &candidate_index) in app.picker_matches.iter().enumerate().skip(scroll) {
        if Some(position) == recent_heading_index && lines.len() + 1 < visible_height {
            lines.push(picker_recent_heading_line(
                rows[1].width as usize,
                surface,
                t,
            ));
        }
        if lines.len() >= visible_height {
            break;
        }
        let Some(candidate) = app.picker_strs.get(candidate_index) else {
            continue;
        };
        let Some(path) = app.picker_paths.get(candidate_index) else {
            continue;
        };
        let is_recent = app
            .picker_is_recent
            .get(candidate_index)
            .copied()
            .unwrap_or(false);
        lines.push(picker_entry_line(
            candidate,
            path,
            is_recent,
            &app.picker_query,
            position == app.picker_idx,
            rows[1].width as usize,
            t,
        ));
    }
    if lines.is_empty() {
        let message = if app.picker_strs.is_empty() {
            "   No Parquet files found"
        } else {
            "   No matches"
        };
        lines.push(Line::from(Span::styled(
            message,
            Style::default().fg(t.text_dim).bg(surface),
        )));
    }
    f.render_widget(
        Paragraph::new(lines).style(Style::default().bg(surface)),
        rows[1],
    );

    let position = if app.picker_matches.is_empty() {
        0
    } else {
        app.picker_idx + 1
    };
    let action = if app.file.is_some() { "cancel" } else { "quit" };
    f.render_widget(
        Paragraph::new(picker_hint_line(
            &[("enter", "open"), ("esc", action)],
            format!("{position}/{}", app.picker_matches.len()),
            chrome,
            selection,
            t,
        ))
        .style(Style::default().bg(chrome)),
        rows[2],
    );
}

fn picker_recent_heading_line(width: usize, surface: Color, t: &Theme) -> Line<'static> {
    let label = " Most Recent ";
    let mut spans = vec![
        Span::styled("  ", Style::default().bg(surface)),
        Span::styled(
            label,
            Style::default().fg(t.picker_recent).bg(surface).bold(),
        ),
    ];
    let used = 2 + label.chars().count();
    if used < width {
        spans.push(Span::styled(
            "─".repeat(width - used),
            Style::default().fg(t.picker_border).bg(surface),
        ));
    }
    Line::from(spans)
}

fn picker_entry_line(
    candidate: &str,
    path: &std::path::Path,
    is_recent: bool,
    query: &str,
    selected: bool,
    width: usize,
    t: &Theme,
) -> Line<'static> {
    let background = if selected { t.cursor_bg } else { t.background };
    let matches = picker::match_indices(query, candidate);
    let basename_start = candidate
        .char_indices()
        .rev()
        .find(|(_, character)| *character == '/')
        .map_or(0, |(index, _)| candidate[..=index].chars().count());
    let mut spans = vec![Span::styled(
        if selected { "▌ " } else { "  " },
        Style::default().fg(t.picker_accent).bg(background),
    )];
    for (index, character) in candidate.chars().enumerate() {
        let matched = matches.binary_search(&index).is_ok();
        let foreground = if matched {
            t.picker_matched
        } else if index < basename_start {
            t.picker_directory
        } else if is_recent {
            t.picker_recent
        } else {
            t.text
        };
        let mut style = Style::default().fg(foreground).bg(background);
        if matched || (selected && index >= basename_start) {
            style = style.bold();
        }
        spans.push(Span::styled(character.to_string(), style));
    }
    let mut line = Line::from(spans);
    let mut used = line.width();
    if is_recent
        && query.is_empty()
        && let Some(parent) = path.parent()
    {
        let parent = shorten_path(&parent.to_string_lossy());
        let available = width.saturating_sub(used + 2);
        if available >= 3 {
            let parent = truncate_left(&parent, available);
            let parent_width = parent.width();
            let gap = width.saturating_sub(used + parent_width);
            line.spans.push(Span::styled(
                " ".repeat(gap),
                Style::default().bg(background),
            ));
            line.spans.push(Span::styled(
                parent,
                Style::default().fg(t.text_dim).bg(background),
            ));
            used = width;
        }
    }
    if used < width {
        line.spans.push(Span::styled(
            " ".repeat(width - used),
            Style::default().bg(background),
        ));
    }
    line
}

fn truncate_left(value: &str, max_width: usize) -> String {
    if value.width() <= max_width {
        return value.to_string();
    }
    if max_width == 0 {
        return String::new();
    }

    let suffix_width = max_width.saturating_sub(1);
    let mut suffix = Vec::new();
    let mut used = 0;
    for character in value.chars().rev() {
        let width = unicode_width::UnicodeWidthChar::width(character).unwrap_or(0);
        if used + width > suffix_width {
            break;
        }
        suffix.push(character);
        used += width;
    }
    suffix.reverse();
    format!("…{}", suffix.into_iter().collect::<String>())
}

fn picker_hint_line(
    bindings: &[(&str, &str)],
    status: String,
    chrome: Color,
    selection: Color,
    t: &Theme,
) -> Line<'static> {
    let mut spans = vec![Span::styled(" ", Style::default().bg(chrome))];
    for (key, action) in bindings {
        spans.push(Span::styled(
            format!(" {key} "),
            Style::default().fg(t.key).bg(selection).bold(),
        ));
        spans.push(Span::styled(
            format!(" {action}  "),
            Style::default().fg(t.text_dim).bg(chrome),
        ));
    }
    spans.push(Span::styled(
        status,
        Style::default().fg(t.text_dim).bg(chrome),
    ));
    Line::from(spans)
}

fn draw_theme_picker_popup(f: &mut Frame, app: &mut App, area: Rect) {
    let t = &app.theme;
    let popup_area = picker_rect(area);
    let surface = t.background;
    let chrome = t.background_dark;
    let selection = t.cursor_bg;

    f.render_widget(
        Block::default().style(Style::default().bg(t.background_deep)),
        area,
    );
    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(" Themes ")
        .style(Style::default().bg(surface).fg(t.text))
        .title_style(Style::default().fg(t.picker_accent).bg(surface).bold())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(t.picker_border));
    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    if inner.height < 2 || inner.width < 4 {
        return;
    }

    let rows = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).split(inner);
    let visible_height = rows[0].height as usize;
    app.popup_visible_height = visible_height;
    let scroll = if app.theme_idx >= visible_height {
        app.theme_idx - visible_height + 1
    } else {
        0
    };
    let lines = app
        .themes
        .iter()
        .enumerate()
        .skip(scroll)
        .take(visible_height)
        .map(|(index, (name, _))| {
            let selected = index == app.theme_idx;
            let background = if selected { selection } else { surface };
            let marker = if selected { "▌ " } else { "  " };
            let mut line = Line::from(vec![
                Span::styled(marker, Style::default().fg(t.picker_accent).bg(background)),
                Span::styled(
                    name.clone(),
                    Style::default().fg(t.text).bg(background).bold(),
                ),
            ]);
            let used = line.width();
            let width = rows[0].width as usize;
            if used < width {
                line.spans.push(Span::styled(
                    " ".repeat(width - used),
                    Style::default().bg(background),
                ));
            }
            line
        })
        .collect::<Vec<_>>();
    f.render_widget(
        Paragraph::new(lines).style(Style::default().bg(surface)),
        rows[0],
    );

    f.render_widget(
        Paragraph::new(picker_hint_line(
            &[("j/k", "select"), ("enter", "apply"), ("esc", "cancel")],
            format!("{}/{}", app.theme_idx + 1, app.themes.len()),
            chrome,
            selection,
            t,
        ))
        .style(Style::default().bg(chrome)),
        rows[1],
    );
}

fn picker_rect(area: Rect) -> Rect {
    let width = if area.width > 4 {
        (area.width * 3 / 4).max(50).min(area.width - 4)
    } else {
        area.width.max(1)
    };
    let height = if area.height > 4 {
        (area.height * 3 / 4).max(6).min(area.height - 2)
    } else {
        area.height.max(1)
    };
    Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    )
}

fn shorten_path(path: &str) -> String {
    std::env::var_os("HOME")
        .and_then(|home| {
            path.strip_prefix(home.to_string_lossy().as_ref())
                .map(|suffix| format!("~{suffix}"))
        })
        .unwrap_or_else(|| path.to_string())
}

fn draw_filter_popup(f: &mut Frame, app: &mut App, area: Rect) {
    let t = &app.theme;
    let col_name = &app.columns[app.filter_column];
    let popup_area = centered_rect(50, 70, area);
    f.render_widget(Clear, popup_area);

    let total = app.filter_suggestions.len();
    let shown = app.popup_matches.len();
    let selected_count = app.filter_selected.len();
    let count_part = if app.popup_query.is_empty() {
        format!("{}", total)
    } else {
        format!("{}/{}", shown, total)
    };
    let sel_part = if selected_count > 0 {
        format!(" {} selected |", selected_count)
    } else {
        String::new()
    };
    let nav_part = if app.popup_searching {
        " ↑↓ nav | Enter toggle | Esc nav "
    } else {
        " j/k nav | Space toggle | / search | Esc close "
    };
    let bottom = format!(" {} |{}{}", count_part, sel_part, nav_part);

    let block = Block::default()
        .title(format!(" Filter: {} ", col_name))
        .title_style(Style::default().fg(t.accent).bold())
        .title_bottom(bottom)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(t.accent));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    let list_area = draw_popup_query_row(f, app, inner, t.accent);
    app.popup_visible_height = list_area.height as usize;

    if app.filter_suggestions.is_empty() {
        let message = if app.filter_values_loading {
            "Loading values..."
        } else {
            "No values found"
        };
        let msg = Paragraph::new(message)
            .style(Style::default().fg(t.text_dim))
            .alignment(Alignment::Center);
        f.render_widget(msg, list_area);
        return;
    }

    if app.popup_matches.is_empty() {
        let msg = Paragraph::new("No matches")
            .style(Style::default().fg(t.text_dim))
            .alignment(Alignment::Center);
        f.render_widget(msg, list_area);
        return;
    }

    draw_checklist(
        f,
        list_area,
        &app.filter_suggestions,
        &app.popup_matches,
        &app.filter_selected,
        app.filter_cursor_idx,
        (t.accent, t),
    );
}

fn draw_columns_popup(f: &mut Frame, app: &mut App, area: Rect) {
    let t = &app.theme;
    let popup_area = centered_rect(50, 70, area);
    f.render_widget(Clear, popup_area);

    let visible_count = app.visible_columns.len();
    let shown = app.popup_matches.len();
    let count_part = if app.popup_query.is_empty() {
        format!("{}/{} shown", visible_count, app.columns.len())
    } else {
        format!(
            "{}/{} shown | {} match",
            visible_count,
            app.columns.len(),
            shown
        )
    };
    let nav_part = if app.popup_searching {
        " ↑↓ nav | Enter toggle | ^A all | ^D none | Esc nav "
    } else {
        " j/k nav | Space toggle | a all | d none | / search | Esc close "
    };
    let bottom = format!(" {} |{}", count_part, nav_part);

    let block = Block::default()
        .title(" Columns ")
        .title_style(Style::default().fg(t.accent).bold())
        .title_bottom(bottom)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(t.accent));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    let list_area = draw_popup_query_row(f, app, inner, t.accent);
    app.popup_visible_height = list_area.height as usize;

    if app.popup_matches.is_empty() && !app.columns.is_empty() {
        let msg = Paragraph::new("No matches")
            .style(Style::default().fg(t.text_dim))
            .alignment(Alignment::Center);
        f.render_widget(msg, list_area);
        return;
    }

    draw_checklist(
        f,
        list_area,
        &app.columns,
        &app.popup_matches,
        &app.visible_columns,
        app.column_picker_idx,
        (t.accent, t),
    );
}

fn draw_checklist(
    f: &mut Frame,
    area: Rect,
    items: &[String],
    matches: &[usize],
    selected: &HashSet<String>,
    cursor: usize,
    colors: (Color, &Theme),
) {
    let (active_color, t) = colors;
    let visible_height = area.height as usize;
    if visible_height == 0 {
        return;
    }
    let scroll = if cursor >= visible_height {
        cursor - visible_height + 1
    } else {
        0
    };

    let list_items: Vec<ListItem> = matches
        .iter()
        .enumerate()
        .skip(scroll)
        .take(visible_height)
        .filter_map(|(visible_idx, &src_idx)| {
            let val = items.get(src_idx)?;
            let is_selected = selected.contains(val);
            let is_cursor = visible_idx == cursor;

            let checkbox = if is_selected { "[x] " } else { "[ ] " };
            let text = format!("{}{}", checkbox, val);

            let style = if is_cursor && is_selected {
                Style::default().fg(active_color).bg(t.cursor_bg).bold()
            } else if is_cursor {
                Style::default().fg(t.text).bg(t.cursor_bg).bold()
            } else if is_selected {
                Style::default().fg(active_color)
            } else {
                Style::default().fg(t.text)
            };

            Some(ListItem::new(text).style(style))
        })
        .collect();

    f.render_widget(List::new(list_items), area);
}

fn draw_popup_query_row(f: &mut Frame, app: &App, area: Rect, active_color: Color) -> Rect {
    let t = &app.theme;
    let show_row = app.popup_searching || !app.popup_query.is_empty();
    if !show_row || area.height < 3 {
        return area;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(1),
        ])
        .split(area);

    let prefix_color = if app.popup_searching {
        active_color
    } else {
        t.text_dim
    };
    let prompt_prefix = Span::styled("/ ", Style::default().fg(prefix_color).bold());
    let query_color = if app.popup_searching {
        t.text
    } else {
        t.text_dim
    };
    let query_span = Span::styled(&app.popup_query, Style::default().fg(query_color));
    f.render_widget(
        Paragraph::new(Line::from(vec![prompt_prefix, query_span])),
        chunks[0],
    );

    if app.popup_searching {
        f.set_cursor_position((
            chunks[0].x + 2 + input::display_width(&app.popup_query, app.popup_query_cursor),
            chunks[0].y,
        ));
    }

    let divider = "─".repeat(chunks[1].width as usize);
    f.render_widget(
        Paragraph::new(Span::styled(divider, Style::default().fg(t.border))),
        chunks[1],
    );

    chunks[2]
}

fn draw_filter_bar(f: &mut Frame, app: &App, area: Rect) {
    let t = &app.theme;
    let filter_count = app
        .active_filters()
        .values()
        .filter(|v| !v.is_empty())
        .count()
        + app.exclude_empty.len();

    let border_style = if app.mode == Mode::Filter {
        Style::default().fg(t.accent)
    } else {
        Style::default().fg(t.border)
    };

    let title = if filter_count > 0 {
        format!(" Filters ({} active) ", filter_count)
    } else {
        " Filters — h/l select column, f to filter ".to_string()
    };

    let block = Block::default()
        .title(title)
        .title_style(Style::default().fg(if filter_count > 0 {
            t.accent
        } else {
            t.text_dim
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
            spans.push(Span::styled(" | ", Style::default().fg(t.border)));
        }

        let all_idx = app.columns.iter().position(|c| c == col).unwrap_or(0);
        let filter_display = app.filter_display(col);
        let is_focused = all_idx == app.filter_column;
        let is_search_col = all_idx == app.search_column && !app.search_query.is_empty();
        let is_excluding_empty = app.exclude_empty.contains(col);

        let display = match (&filter_display, is_excluding_empty) {
            (Some(vals), true) => format!("{}={} !null", col, vals),
            (Some(vals), false) => format!("{}={}", col, vals),
            (None, true) => format!("{} !null", col),
            (None, false) => col.clone(),
        };

        let fg = if is_search_col {
            t.heading
        } else if filter_display.is_some() || is_excluding_empty || is_focused {
            t.accent
        } else {
            t.text_dim
        };

        let style = if is_focused {
            Style::default().fg(fg).bg(t.cursor_bg).bold()
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
    let t = &app.theme;
    let is_searching = app.mode == Mode::Search;

    let border_style = if is_searching {
        Style::default().fg(t.heading)
    } else {
        Style::default().fg(t.border)
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
        .title_style(
            Style::default().fg(if is_searching || !app.search_query.is_empty() {
                t.heading
            } else {
                t.text_dim
            }),
        )
        .borders(Borders::ALL)
        .border_style(border_style);

    let input_text = if app.search_query.is_empty() && !is_searching {
        Span::styled("", Style::default().fg(t.text_dim))
    } else {
        Span::styled(&app.search_query, Style::default().fg(t.text))
    };

    let paragraph = Paragraph::new(Line::from(vec![input_text])).block(block);
    f.render_widget(paragraph, area);

    if is_searching {
        f.set_cursor_position((
            area.x + 1 + input::display_width(&app.search_query, app.search_cursor),
            area.y + 1,
        ));
    }
}

fn draw_export_bar(f: &mut Frame, app: &App, area: Rect) {
    let t = &app.theme;
    let bar_area = Rect::new(area.x, area.bottom().saturating_sub(3), area.width, 3);
    f.render_widget(Clear, bar_area);

    let block = Block::default()
        .title(" Export filtered data ")
        .title_style(Style::default().fg(t.accent).bold())
        .title_bottom(" Enter to save | Esc to cancel ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(t.accent));

    let input = Span::styled(&app.export_path, Style::default().fg(t.text));
    let paragraph = Paragraph::new(Line::from(vec![input])).block(block);
    f.render_widget(paragraph, bar_area);

    f.set_cursor_position((
        bar_area.x + 1 + input::display_width(&app.export_path, app.export_cursor),
        bar_area.y + 1,
    ));
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
    let t = &app.theme;
    let (status, status_color) = if app.loading {
        (" loading... ".to_string(), t.picker_loading)
    } else {
        let page_start = app.offset as usize + 1;
        let page_end = (app.offset as usize + app.rows.len()).min(app.total_matches);
        if app.total_matches > 0 {
            (
                format!(" {}-{} of {} ", page_start, page_end, app.total_matches),
                t.text_dim,
            )
        } else if app.has_active_filters() || !app.search_query.is_empty() {
            (" no matches ".to_string(), t.error)
        } else {
            (" no data ".to_string(), t.text_dim)
        }
    };

    let col_count = app.display_columns().len();
    let title = format!(" Results ({} x {}) ", app.total_matches, col_count);

    let block = Block::default()
        .title(title)
        .title_style(Style::default().fg(t.heading).bold())
        .title_bottom(Line::from(Span::styled(
            status,
            Style::default().fg(status_color),
        )))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(t.border));

    if app.rows.is_empty() {
        let empty_msg = if app.loading {
            "Searching..."
        } else if app.has_active_filters() || !app.search_query.is_empty() {
            "No results match current filters/search"
        } else {
            "No data"
        };
        let paragraph = Paragraph::new(empty_msg)
            .style(Style::default().fg(t.text_dim))
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
            let is_focused = app
                .columns
                .get(app.filter_column)
                .is_some_and(|focused| focused == name);
            let style = if has_filter && is_search_col {
                Style::default().fg(t.key).bold().underlined()
            } else if has_filter {
                Style::default().fg(t.accent).bold()
            } else if is_search_col {
                Style::default().fg(t.heading).bold()
            } else if is_focused {
                Style::default().fg(t.selection).bold()
            } else {
                Style::default().fg(t.text_bright).bold()
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
                Style::default().bg(t.cursor_bg).fg(t.selection)
            } else {
                Style::default().fg(t.text)
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
        .row_highlight_style(Style::default().bg(t.cursor_bg));

    f.render_widget(table, area);

    let help = navigation_help_line(t);
    let help_area = Rect::new(area.x + 1, area.bottom() - 1, help.width() as u16, 1);
    if help_area.right() < area.right() {
        f.render_widget(Paragraph::new(help), help_area);
    }
}

fn navigation_help_line(t: &Theme) -> Line<'static> {
    let bindings = [
        ("j/k", "nav"),
        ("h/l", "column"),
        ("f", "filter"),
        ("/", "search"),
        ("x", "!null"),
        ("w", "export"),
        ("v", "columns"),
        ("o", "open"),
        ("t", "theme"),
        ("n/p", "page"),
        ("Tab", "preview"),
        ("C", "clear"),
        ("q", "quit"),
    ];
    let mut spans = vec![Span::raw(" ")];
    for (index, (key, action)) in bindings.into_iter().enumerate() {
        if index > 0 {
            spans.push(Span::styled(" | ", Style::default().fg(t.border)));
        }
        spans.push(Span::styled(key, Style::default().fg(t.key).bold()));
        spans.push(Span::styled(
            format!(" {action}"),
            Style::default().fg(t.text_dim),
        ));
    }
    spans.push(Span::raw(" "));
    Line::from(spans)
}

fn draw_preview(f: &mut Frame, app: &App, area: Rect) {
    let t = &app.theme;
    let preview_col = app.preview_column.unwrap_or(app.filter_column);
    let col_name = app
        .columns
        .get(preview_col)
        .map(|s| s.as_str())
        .unwrap_or("?");

    let block = Block::default()
        .title(format!(" Preview: {} ", col_name))
        .title_style(Style::default().fg(t.accent))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(t.border));

    if app.rows.is_empty() || app.selected >= app.rows.len() {
        let paragraph = Paragraph::new("No row selected")
            .style(Style::default().fg(t.text_dim))
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
                highlight_matches(l, &app.search_query, t)
            } else {
                Line::from(l.to_string())
            }
        })
        .collect();

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });
    f.render_widget(paragraph, area);
}

fn highlight_matches<'a>(text: &'a str, query: &str, t: &Theme) -> Line<'a> {
    let lower_text = text.to_lowercase();
    let lower_query = query.to_lowercase();
    let mut spans: Vec<Span<'a>> = Vec::new();
    let mut last_end = 0;

    for (start, _) in lower_text.match_indices(&lower_query) {
        if start > last_end {
            spans.push(Span::styled(
                text[last_end..start].to_string(),
                Style::default().fg(t.text),
            ));
        }
        spans.push(Span::styled(
            text[start..start + query.len()].to_string(),
            Style::default()
                .fg(t.background_dark)
                .bg(t.selection)
                .bold(),
        ));
        last_end = start + query.len();
    }

    if last_end < text.len() {
        spans.push(Span::styled(
            text[last_end..].to_string(),
            Style::default().fg(t.text),
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

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;

    #[test]
    fn file_picker_uses_pdfterm_panel_layout() {
        let mut app = App::new();
        app.mode = Mode::FilePicker;
        app.picker_root = "/synthetic/root".into();
        app.picker_paths = vec!["/synthetic/root/nested/synthetic.parquet".into()];
        app.picker_strs = vec!["nested/synthetic.parquet".into()];
        app.picker_is_recent = vec![false];
        app.picker_matches = vec![0];

        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal.draw(|frame| draw(frame, &mut app)).unwrap();

        let buffer = terminal.backend().buffer();
        let text = buffer
            .content()
            .chunks(80)
            .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("/synthetic/root"));
        assert!(text.contains("type to filter..."));
        assert!(text.contains("nested/synthetic.parquet"));
        assert!(text.contains("enter  open"));
        assert!(text.contains("esc  quit"));
        assert!(text.contains("1/1"));

        assert_eq!(buffer.cell((0, 0)).unwrap().bg, app.theme.background_deep);
        assert_eq!(buffer.cell((11, 4)).unwrap().bg, app.theme.background_dark);
        assert_eq!(buffer.cell((11, 5)).unwrap().symbol(), "▌");
        assert_eq!(buffer.cell((11, 5)).unwrap().bg, app.theme.cursor_bg);
    }

    #[test]
    fn file_picker_labels_recent_files_with_parent_directory() {
        let mut app = App::new();
        app.mode = Mode::FilePicker;
        app.picker_root = "/synthetic/root".into();
        app.picker_paths = vec!["/synthetic/archive/recent.parquet".into()];
        app.picker_strs = vec!["recent.parquet".into()];
        app.picker_is_recent = vec![true];
        app.picker_matches = vec![0];

        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal.draw(|frame| draw(frame, &mut app)).unwrap();

        let buffer = terminal.backend().buffer();
        let text = buffer
            .content()
            .chunks(80)
            .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("Most Recent"));
        assert!(text.contains("recent.parquet"));
        assert!(text.contains("/synthetic/archive"));
    }

    #[test]
    fn file_picker_handles_compact_terminals() {
        for (width, height) in [(1, 1), (4, 4), (40, 5)] {
            let mut app = App::new();
            app.mode = Mode::FilePicker;
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal.draw(|frame| draw(frame, &mut app)).unwrap();
        }
    }

    #[test]
    fn theme_picker_uses_preview_panel_layout() {
        let mut app = App::new();
        let mut alternate = app.theme;
        alternate.accent = Color::Red;
        app.themes.push(("alternate".into(), alternate));
        app.mode = Mode::ThemePicker;
        app.theme_idx = 1;
        app.theme = app.themes[1].1;

        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal.draw(|frame| draw(frame, &mut app)).unwrap();

        let text = terminal
            .backend()
            .buffer()
            .content()
            .chunks(80)
            .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("Themes"));
        assert!(text.contains("alternate"));
        assert!(text.contains("j/k  select"));
        assert!(text.contains("enter  apply"));
        assert!(text.contains("esc  cancel"));
        assert!(text.contains("2/2"));
    }

    #[test]
    fn browse_view_uses_distinct_semantic_theme_roles() {
        let mut app = App::new();
        app.columns = vec!["alpha".into(), "beta".into()];
        app.visible_columns = app.columns.iter().cloned().collect();
        app.rows = vec![vec!["one".into(), "two".into()]; 2];
        app.total_matches = 2;
        app.show_preview = false;

        let mut terminal = Terminal::new(TestBackend::new(220, 24)).unwrap();
        terminal.draw(|frame| draw(frame, &mut app)).unwrap();

        let buffer = terminal.backend().buffer();
        let results_x = find_in_row(buffer, 6, "Results");
        let alpha_x = find_in_row(buffer, 7, "alpha");
        let beta_x = find_in_row(buffer, 7, "beta");
        let key_x = find_in_row(buffer, 23, "j/k");
        let action_x = find_in_row(buffer, 23, "nav");

        assert_eq!(buffer.cell((results_x, 6)).unwrap().fg, app.theme.heading);
        assert_eq!(buffer.cell((alpha_x, 7)).unwrap().fg, app.theme.selection);
        assert_eq!(buffer.cell((beta_x, 7)).unwrap().fg, app.theme.text_bright);
        assert_eq!(buffer.cell((key_x, 23)).unwrap().fg, app.theme.key);
        assert_eq!(buffer.cell((action_x, 23)).unwrap().fg, app.theme.text_dim);
    }

    fn find_in_row(buffer: &Buffer, y: u16, needle: &str) -> u16 {
        let row = (0..buffer.area.width)
            .map(|x| buffer.cell((x, y)).unwrap().symbol())
            .collect::<String>();
        row.find(needle).unwrap() as u16
    }
}
