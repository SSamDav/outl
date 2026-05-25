//! ratatui rendering: turn the current `App` into a frame.
//!
//! This is the *only* place in the crate that produces `ratatui::Line`
//! and `Span` values. Anything else (state, actions, key handling) is
//! UI-agnostic. The block decomposition itself lives upstream in
//! [`outl_md::view`] so Tauri and mobile clients consume the same
//! row classification and only need to swap this file out.

use crate::actions::truncate_for_snippet;
use crate::outline_ops::path_for_index;
use crate::state::{
    App, AutocompleteKind, AutocompleteState, CommandState, ErrorState, Mode, Overlay,
    QuickSwitchState, SearchState, SlashState, SwitchKind, View, HELP_HINT_INSERT,
    HELP_HINT_NORMAL, HELP_HINT_VISUAL,
};
use crate::theme::Theme;
use outl_md::inline::{byte_index_for_char, tokenize, InlineTok};
use outl_md::parse::{OutlineNode, ParsedPage};
use outl_md::view::{block_to_rows, BlockRowKind};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph};

pub(crate) fn render_app(f: &mut ratatui::Frame<'_>, app: &mut App) {
    let area = f.area();
    let backlinks = app.index.backlinks(&app.current_slug()).to_vec();
    let show_bl = app.show_backlinks && !backlinks.is_empty();

    // Layout: outline takes the whole width when backlinks are off,
    // outline + backlinks panel when the current page has incoming
    // refs and the user hasn't toggled it off with `B`.
    let constraints: Vec<Constraint> = if show_bl {
        vec![Constraint::Min(40), Constraint::Length(32)]
    } else {
        vec![Constraint::Min(40)]
    };
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(constraints)
        .split(area);
    if show_bl {
        render_backlinks_panel(f, cols[1], app, &backlinks);
    }
    render_main(f, cols[0], app);

    // Overlays draw on top of everything else.
    match &app.overlay {
        Some(Overlay::QuickSwitch(qs)) => render_quick_switch(f, area, app, qs),
        Some(Overlay::Search(s)) => render_search_overlay(f, area, app, s),
        Some(Overlay::Command(c)) => render_command_bar(f, area, app, c),
        Some(Overlay::Error(e)) => render_error_overlay(f, area, app, e),
        Some(Overlay::Slash(s)) => render_slash_overlay(f, area, app, s),
        None => {}
    }

    if let Some(ac) = &app.autocomplete {
        render_autocomplete(f, area, app, ac);
    }

    if app.show_help {
        render_help_popup(f, area, app);
    }
}

fn render_autocomplete(f: &mut ratatui::Frame<'_>, full: Rect, app: &App, ac: &AutocompleteState) {
    let height = (ac.candidates.len() as u16 + 2).min(10);
    if height < 3 {
        return;
    }
    let width = 36u16.min(full.width.saturating_sub(4));
    // Bottom-right anchor so it doesn't fight with the outline.
    let area = Rect {
        x: full.x + full.width.saturating_sub(width + 2),
        y: full.y + full.height.saturating_sub(height + 2),
        width,
        height,
    };
    f.render_widget(Clear, area);
    let title = match ac.kind {
        AutocompleteKind::PageRef => format!("[[{}]]", ac.query),
        AutocompleteKind::Tag => format!("#{}", ac.query),
        AutocompleteKind::SlashCommand => format!("/{}", ac.query),
    };
    let items: Vec<ListItem<'_>> = ac
        .candidates
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let style = if i == ac.selected {
                app.theme.list_selected
            } else {
                Style::default()
            };
            // Decorate the candidate row according to its kind. For
            // pages/tags we prepend the page's `icon::` (display-only);
            // for slash commands we append a dim description so the
            // popup doubles as in-context help.
            match ac.kind {
                AutocompleteKind::PageRef | AutocompleteKind::Tag => {
                    let icon = match ac.kind {
                        AutocompleteKind::PageRef => {
                            app.index.by_title(c).and_then(|p| p.icon.clone())
                        }
                        AutocompleteKind::Tag => app.index.by_slug(c).and_then(|p| p.icon.clone()),
                        _ => None,
                    };
                    let label = match icon {
                        Some(ic) => format!("{ic} {c}"),
                        None => c.clone(),
                    };
                    ListItem::new(Line::from(Span::styled(label, style)))
                }
                AutocompleteKind::SlashCommand => {
                    let cmd = app.command_registry.get(c);
                    let description = cmd.as_ref().map(|cmd| cmd.description()).unwrap_or("");
                    let needs_args = cmd.as_ref().map(|cmd| cmd.needs_args()).unwrap_or(false);
                    let suffix = if needs_args { " …" } else { "" };
                    ListItem::new(Line::from(vec![
                        Span::styled(format!("{c}{suffix}  "), style),
                        Span::styled(description.to_string(), app.theme.dim),
                    ]))
                }
            }
        })
        .collect();
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(app.theme.border)
                .title(Span::styled(title, app.theme.help_title)),
        )
        .style(Style::default().bg(app.theme.popup_bg));
    f.render_widget(list, area);
}

fn centered_rect(full: Rect, w_pct: u16, h_pct: u16) -> Rect {
    let w = (full.width as u32 * w_pct as u32 / 100) as u16;
    let h = (full.height as u32 * h_pct as u32 / 100) as u16;
    Rect {
        x: full.x + (full.width.saturating_sub(w)) / 2,
        y: full.y + (full.height.saturating_sub(h)) / 2,
        width: w,
        height: h,
    }
}

fn render_quick_switch(f: &mut ratatui::Frame<'_>, full: Rect, app: &App, qs: &QuickSwitchState) {
    let area = centered_rect(full, 60, 60);
    f.render_widget(Clear, area);

    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(3)])
        .split(area);

    let input = Paragraph::new(Line::from(vec![
        Span::styled(" › ", app.theme.help_title),
        Span::raw(qs.query.clone()),
        Span::styled("▏", app.theme.cursor_caret),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(app.theme.border)
            .title(Span::styled("Quick Switcher", app.theme.help_title)),
    )
    .style(Style::default().bg(app.theme.popup_bg));
    f.render_widget(input, outer[0]);

    let items: Vec<ListItem<'_>> = qs
        .candidates
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let icon = match c.kind {
                SwitchKind::Page => "📄 ",
                SwitchKind::Journal => "📅 ",
            };
            let style = if i == qs.selected {
                app.theme.list_selected
            } else {
                Style::default()
            };
            ListItem::new(Line::from(vec![
                Span::raw(icon),
                Span::styled(c.label.clone(), style),
            ]))
        })
        .collect();
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(app.theme.border)
                .title(format!(
                    "{} matches  ↑↓ navigate · Enter open · Esc cancel",
                    qs.candidates.len()
                )),
        )
        .style(Style::default().bg(app.theme.popup_bg));
    f.render_widget(list, outer[1]);
}

fn render_search_overlay(f: &mut ratatui::Frame<'_>, full: Rect, app: &App, s: &SearchState) {
    let area = centered_rect(full, 75, 70);
    f.render_widget(Clear, area);

    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(3)])
        .split(area);

    let input = Paragraph::new(Line::from(vec![
        Span::styled(" / ", app.theme.help_title),
        Span::raw(s.query.clone()),
        Span::styled("▏", app.theme.cursor_caret),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(app.theme.border)
            .title(Span::styled("Search", app.theme.help_title)),
    )
    .style(Style::default().bg(app.theme.popup_bg));
    f.render_widget(input, outer[0]);

    let lines: Vec<Line<'_>> = s
        .hits
        .iter()
        .enumerate()
        .map(|(i, h)| {
            let style = if i == s.selected {
                app.theme.list_selected
            } else {
                Style::default()
            };
            let icon_prefix = h
                .page_icon
                .as_deref()
                .map(|i| format!("{i} "))
                .unwrap_or_default();
            Line::from(vec![
                Span::styled(format!(" {icon_prefix}{} · ", h.page_label), app.theme.dim),
                Span::styled(h.snippet.clone(), style),
            ])
        })
        .collect();
    let list = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(app.theme.border)
                .title(format!(
                    "{} hits  ↑↓ navigate · Enter jump · Esc cancel",
                    s.hits.len()
                )),
        )
        .style(Style::default().bg(app.theme.popup_bg))
        .wrap(ratatui::widgets::Wrap { trim: false });
    f.render_widget(list, outer[1]);
}

fn render_slash_overlay(f: &mut ratatui::Frame<'_>, full: Rect, app: &App, s: &SlashState) {
    let area = centered_rect(full, 60, 60);
    f.render_widget(Clear, area);

    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(3)])
        .split(area);

    let input = Paragraph::new(Line::from(vec![
        Span::styled(" / ", app.theme.help_title),
        Span::raw(s.query.clone()),
        Span::styled("▏", app.theme.cursor_caret),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(app.theme.border)
            .title(Span::styled("Commands", app.theme.help_title)),
    )
    .style(Style::default().bg(app.theme.popup_bg));
    f.render_widget(input, outer[0]);

    let items: Vec<ListItem<'_>> = s
        .candidates
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let style = if i == s.selected {
                app.theme.list_selected
            } else {
                Style::default()
            };
            // Two-column-ish: name on the left, description dimmed
            // on the right. The `…` glyph hints at "this one asks
            // for args next".
            let suffix = if c.needs_args { " …" } else { "" };
            ListItem::new(Line::from(vec![
                Span::styled(format!(" {}{suffix}  ", c.name), style),
                Span::styled(c.description.to_string(), app.theme.dim),
            ]))
        })
        .collect();
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(app.theme.border)
                .title(format!(
                    "{} commands  ↑↓ navigate · Enter run · Esc cancel",
                    s.candidates.len()
                )),
        )
        .style(Style::default().bg(app.theme.popup_bg));
    f.render_widget(list, outer[1]);
}

fn render_error_overlay(f: &mut ratatui::Frame<'_>, full: Rect, app: &App, err: &ErrorState) {
    // Auto-size: pick 80% of the viewport, capped so a tiny body
    // doesn't draw a giant empty modal.
    let body_lines = err.body.lines().count().max(1) as u16;
    let popup_w = (full.width as f32 * 0.8) as u16;
    let popup_h = (body_lines + 4).min((full.height as f32 * 0.7) as u16);
    let x = (full.width.saturating_sub(popup_w)) / 2;
    let y = (full.height.saturating_sub(popup_h)) / 2;
    let area = Rect {
        x,
        y,
        width: popup_w,
        height: popup_h,
    };
    f.render_widget(Clear, area);

    let lines: Vec<Line<'_>> = err
        .body
        .lines()
        .map(|l| Line::from(Span::raw(l.to_string())))
        .collect();

    let title = format!(" ✕ {} · press any key to dismiss ", err.title);
    let widget = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(app.theme.status_message)
                .title(Span::styled(title, app.theme.status_message)),
        )
        .style(Style::default().bg(app.theme.popup_bg))
        .wrap(ratatui::widgets::Wrap { trim: false });
    f.render_widget(widget, area);
}

fn render_command_bar(f: &mut ratatui::Frame<'_>, full: Rect, app: &App, c: &CommandState) {
    let h = 3u16;
    let area = Rect {
        x: full.x,
        y: full.y + full.height.saturating_sub(h),
        width: full.width,
        height: h,
    };
    f.render_widget(Clear, area);
    let line = Line::from(vec![
        Span::styled(" : ", app.theme.help_title),
        Span::raw(c.buffer.clone()),
        Span::styled("▏", app.theme.cursor_caret),
    ]);
    let bar = Paragraph::new(line)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(app.theme.border),
        )
        .style(Style::default().bg(app.theme.popup_bg));
    f.render_widget(bar, area);
}

fn render_backlinks_panel(
    f: &mut ratatui::Frame<'_>,
    area: Rect,
    app: &App,
    backlinks: &[outl_md::index::Backlink],
) {
    let mut lines: Vec<Line<'_>> = Vec::new();
    let mut last_source: Option<String> = None;
    for bl in backlinks {
        if last_source.as_deref() != Some(bl.source_slug.as_str()) {
            let header = match &bl.source_icon {
                Some(icon) => format!("{icon} {}", bl.source_title),
                None => bl.source_title.clone(),
            };
            lines.push(Line::from(Span::styled(header, app.theme.heading)));
            last_source = Some(bl.source_slug.clone());
        }
        lines.push(Line::from(vec![
            Span::styled("  • ", app.theme.bullet),
            Span::raw(truncate_for_snippet(&bl.snippet, 60)),
        ]));
    }
    if lines.is_empty() {
        lines.push(Line::from(Span::styled("No backlinks yet", app.theme.dim)));
    }
    let widget = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(app.theme.border)
                .title(format!("Backlinks · {} ref(s)", backlinks.len())),
        )
        .wrap(ratatui::widgets::Wrap { trim: false });
    f.render_widget(widget, area);
}

fn render_main(f: &mut ratatui::Frame<'_>, area: Rect, app: &mut App) {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(area);

    // Header: page title on the left, workspace/index info on the right.
    let workspace_label = app
        .workspace_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("workspace")
        .to_string();
    let stats = format!(
        "  ws:{workspace_label}  pages:{}  blocks:{}",
        app.index.page_count(),
        app.flat_len
    );
    let header = Paragraph::new(Line::from(vec![
        Span::styled(app.current_title(), app.theme.heading),
        Span::styled(stats, app.theme.dim),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(app.theme.border)
            .title(Span::styled(
                format!(" outl · {} ", app.theme.name),
                app.theme.hint,
            )),
    );
    f.render_widget(header, outer[0]);

    let (lines, selected_line) = render_outline(&app.page, app);
    let title = match &app.view {
        View::Journal(_) | View::Page(_) => "Outline",
    };

    // Viewport math: outer[1] is the outline area (borders included).
    // Subtract 2 for top + bottom border lines to get the actually
    // drawable region.
    let viewport_h = outer[1].height.saturating_sub(2);
    app.viewport_height = viewport_h;

    // Auto-scroll: keep the selection visible. If it scrolled off the
    // top, drop the offset down to it; if it scrolled off the bottom,
    // push the offset up so the bullet sits on the last row.
    if let Some(sel) = selected_line {
        let sel = sel as u16;
        if sel < app.scroll_y {
            app.scroll_y = sel;
        } else if viewport_h > 0 && sel >= app.scroll_y + viewport_h {
            app.scroll_y = sel + 1 - viewport_h;
        }
    }
    // Clamp: never scroll past `last_line - viewport_h + 1`.
    let total = lines.len() as u16;
    if total > viewport_h {
        let max_scroll = total - viewport_h;
        if app.scroll_y > max_scroll {
            app.scroll_y = max_scroll;
        }
    } else {
        app.scroll_y = 0;
    }

    let scroll_indicator = if total > viewport_h && viewport_h > 0 {
        format!(
            " ({}/{})",
            app.scroll_y + 1,
            total.saturating_sub(viewport_h) + 1
        )
    } else {
        String::new()
    };
    let outline = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(app.theme.border)
                .title(format!("{title}{scroll_indicator}")),
        )
        .scroll((app.scroll_y, 0));
    // NB: no `.wrap(...)`. Wrap turns one logical line into N visual
    // lines whose count depends on width, which would invalidate our
    // `selected_line` index. We trade off-screen long lines (rare,
    // and you can horizontal-scroll later) for a correct vertical
    // scroll today.
    f.render_widget(outline, outer[1]);

    let (mode_label, mode_style) = match app.mode {
        Mode::Normal => (" NORMAL ", app.theme.status_normal),
        Mode::Insert { .. } => (" INSERT ", app.theme.status_insert),
        Mode::Visual { .. } => (" VISUAL ", app.theme.status_visual),
    };
    let hint = match app.mode {
        Mode::Insert { .. } => HELP_HINT_INSERT,
        Mode::Visual { .. } => HELP_HINT_VISUAL,
        Mode::Normal => HELP_HINT_NORMAL,
    };
    // Backlink count for this view (when it matters).
    let bl_count = app.index.backlinks(&app.current_slug()).len();
    let bl_label = if bl_count == 0 {
        String::new()
    } else {
        format!(
            "  ⇇ {bl_count} backlink{}",
            if bl_count == 1 { "" } else { "s" }
        )
    };
    let footer = Paragraph::new(Line::from(vec![
        Span::styled(mode_label, mode_style),
        Span::raw("  "),
        Span::styled(hint, app.theme.hint),
        Span::styled(bl_label, app.theme.dim),
        Span::raw("  "),
        Span::styled(&app.status, app.theme.status_message),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(app.theme.border),
    );
    f.render_widget(footer, outer[2]);
}

fn render_help_popup(f: &mut ratatui::Frame<'_>, full: Rect, app: &App) {
    let popup_w = (full.width as f32 * 0.7) as u16;
    let popup_h = 34u16.min(full.height.saturating_sub(2));
    let x = (full.width.saturating_sub(popup_w)) / 2;
    let y = (full.height.saturating_sub(popup_h)) / 2;
    let area = Rect {
        x,
        y,
        width: popup_w,
        height: popup_h,
    };
    let body = vec![
        Line::from(Span::styled("NORMAL mode", app.theme.help_title)),
        Line::from("  i           edit current block"),
        Line::from("  I           edit, cursor at start of block"),
        Line::from("  o / O       new block below / above"),
        Line::from("  Enter       open [[ref]] / #tag / journal under cursor"),
        Line::from("              (falls back to edit if nothing matches)"),
        Line::from("  j / k / ↑ ↓ move between blocks"),
        Line::from("  PgDn/PgUp   move one viewport down/up"),
        Line::from("  Ctrl+D / U  half-page down/up"),
        Line::from("  g g / G     first / last block"),
        Line::from("  h / l / ← → move cursor inside the current block"),
        Line::from("  w / b       cursor to next / previous word"),
        Line::from("  0 / $       cursor to start / end of block"),
        Line::from("  Tab / S-Tab indent / outdent"),
        Line::from("  K / J       move block up / down (Alt+↑/↓ too)"),
        Line::from("  dd          delete block"),
        Line::from("  yy / p / P  yank · paste after · paste before"),
        Line::from("  Ctrl+Enter  cycle TODO / DONE / none"),
        Line::from("  u           undo"),
        Line::from("  Ctrl+R      redo"),
        Line::from("  Ctrl+S      force save"),
        Line::from("  Ctrl+L      refresh workspace (re-read from disk)"),
        Line::from("  t           today's journal"),
        Line::from("  [ / ]       previous / next journal"),
        Line::from("  g j         jump to today"),
        Line::from("  g x         run code block under cursor (also `:run`)"),
        Line::from("  B           toggle backlinks panel"),
        Line::from("  ?           toggle this help"),
        Line::from("  q q         quit (chord — single `q` arms it)"),
        Line::from(""),
        Line::from(Span::styled("Overlays", app.theme.help_title)),
        Line::from("  Ctrl+P      quick switcher (pages + journals)"),
        Line::from("  /           slash commands (prop, search, run, ...)"),
        Line::from("  n / N       next / previous search hit"),
        Line::from("  :           vim-style palette (same registry as /)"),
        Line::from(""),
        Line::from(Span::styled("INSERT mode", app.theme.help_title)),
        Line::from("  Esc         commit"),
        Line::from("  Enter       commit + new block below"),
        Line::from("  Ctrl+Enter  cycle TODO / DONE / none (stays in Insert)"),
        Line::from("  Tab / S-Tab indent / outdent (stays in Insert)"),
        Line::from("  [[ / #      autocomplete from existing page titles"),
        Line::from(""),
        Line::from(Span::styled(
            format!("theme: {}", app.theme.name),
            app.theme.dim,
        )),
    ];
    let popup = Paragraph::new(body)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(app.theme.border)
                .title(Span::styled("Help", app.theme.help_title)),
        )
        .style(Style::default().bg(app.theme.popup_bg));
    f.render_widget(Clear, area);
    f.render_widget(popup, area);
}

/// Render the outline into a flat list of `Line`s for ratatui, and
/// report the visual line index where the *selected* block's bullet
/// row landed. The caller uses that index to keep the selection
/// inside the scrolled viewport.
fn render_outline(p: &ParsedPage, app: &App) -> (Vec<Line<'static>>, Option<usize>) {
    let mut out = Vec::new();
    for (k, v) in &p.properties {
        out.push(Line::from(vec![
            Span::styled(format!("{k}:: "), app.theme.property_key),
            Span::styled(v.clone(), app.theme.property_value),
        ]));
    }
    if !p.properties.is_empty() && !p.blocks.is_empty() {
        out.push(Line::from(""));
    }
    let mut cursor = 0usize;
    let mut selected_line: Option<usize> = None;
    for block in &p.blocks {
        render_block(block, 0, &mut cursor, app, &mut out, &mut selected_line);
    }
    (out, selected_line)
}

/// Strip an optional `TODO`/`DONE` prefix off a block's text, returning
/// both the stripped body and a marker describing what was present.
pub(crate) fn split_todo_prefix(text: &str) -> (Option<bool>, &str) {
    if let Some(rest) = text.strip_prefix("TODO ") {
        return (Some(false), rest);
    }
    if let Some(rest) = text.strip_prefix("DONE ") {
        return (Some(true), rest);
    }
    (None, text)
}

fn render_block(
    b: &OutlineNode,
    indent: u32,
    cursor: &mut usize,
    app: &App,
    out: &mut Vec<Line<'static>>,
    selected_line: &mut Option<usize>,
) {
    let is_selected = *cursor == app.selected;
    // Record the visual line where this block's bullet row begins so
    // the caller can scroll the viewport to keep it visible.
    if is_selected && selected_line.is_none() {
        *selected_line = Some(out.len());
    }
    let in_visual_range = app
        .visual_range()
        .is_some_and(|(lo, hi)| *cursor >= lo && *cursor <= hi);
    let editing_here = matches!(&app.mode, Mode::Insert { block_path, .. }
        if path_for_index(&app.page.blocks, *cursor).as_deref() == Some(block_path.as_slice()));

    let bullet_style = if is_selected || in_visual_range {
        app.theme.selected_bullet
    } else {
        app.theme.bullet
    };

    // Determine which text and cursor position to render. Three cases:
    //   1. Editing here       → buffer with caret cursor.
    //   2. Selected in Normal → block text with block-style cursor.
    //   3. Anything else      → block text, no cursor, pretty render.
    let mode = if editing_here {
        if let Mode::Insert { buffer, .. } = &app.mode {
            RenderMode::Editing {
                text: buffer.as_string(),
                cursor_char: buffer.cursor,
            }
        } else {
            unreachable!("editing_here matched but mode isn't Insert")
        }
    } else if is_selected && matches!(app.mode, Mode::Normal) {
        RenderMode::NormalCursor {
            text: b.text.clone(),
            cursor_char: app.cursor_col,
        }
    } else {
        RenderMode::Pretty {
            text: b.text.clone(),
        }
    };

    let has_auto_run = b.properties.iter().any(|(k, _)| k == "auto-run");
    emit_block_lines(indent, bullet_style, &mode, has_auto_run, app, out);

    for (k, v) in &b.properties {
        let mut prop_spans: Vec<Span<'_>> = Vec::new();
        for _ in 0..indent {
            prop_spans.push(Span::styled("│ ", app.theme.dim));
        }
        prop_spans.push(Span::raw("  ".to_string()));
        prop_spans.push(Span::styled(format!("{k}:: "), app.theme.property_key));
        prop_spans.push(Span::styled(v.clone(), app.theme.property_value));
        out.push(Line::from(prop_spans));
    }
    *cursor += 1;
    for child in &b.children {
        render_block(child, indent + 1, cursor, app, out, selected_line);
    }
}

/// Where the cursor sits on a block being rendered, and what style
/// the renderer should use for it. The UI-agnostic decomposition
/// lives in [`outl_md::view`]; this enum carries the *TUI-flavored*
/// detail of "caret vs block cursor".
pub(crate) enum RenderMode {
    /// Insert mode — show the live buffer with a thin caret at
    /// `cursor_char`. Markdown is rendered raw so columns match bytes.
    Editing { text: String, cursor_char: usize },
    /// Normal mode on the selected block — show a vim-style block
    /// cursor on the character under `cursor_char`. Raw render.
    NormalCursor { text: String, cursor_char: usize },
    /// Anything else — markdown is rendered prettily; no cursor.
    Pretty { text: String },
}

/// Emit one or more ratatui [`Line`]s for a block's text.
///
/// Decomposition into visual rows (bullet vs continuation vs code
/// fence marker vs code fence body) is delegated to
/// [`outl_md::view::block_to_rows`] so the Tauri GUI and mobile
/// clients use the same classification. This function is the
/// TUI-specific mapping: each [`outl_md::view::BlockRow`] becomes a
/// `Line` of `Span`s using the active theme.
fn emit_block_lines(
    indent: u32,
    bullet_style: Style,
    mode: &RenderMode,
    has_auto_run: bool,
    app: &App,
    out: &mut Vec<Line<'static>>,
) {
    let (text, cursor_char, cursor_style) = match mode {
        RenderMode::Editing { text, cursor_char } => {
            (text.as_str(), Some(*cursor_char), Some(CursorStyle::Caret))
        }
        RenderMode::NormalCursor { text, cursor_char } => {
            (text.as_str(), Some(*cursor_char), Some(CursorStyle::Block))
        }
        RenderMode::Pretty { text } => (text.as_str(), None, None),
    };
    let pretty = matches!(mode, RenderMode::Pretty { .. });
    let rows = block_to_rows(text, indent, cursor_char);

    // TODO/DONE checkbox decoration only fits on single-line bullets
    // (multi-line ones would have the icon floating above body text).
    let single_line_pretty = pretty && rows.len() == 1;

    for row in &rows {
        let mut spans: Vec<Span<'_>> = Vec::new();
        for _ in 0..row.indent {
            spans.push(Span::styled("│ ", app.theme.dim));
        }
        match row.kind {
            BlockRowKind::Bullet => {
                // Blocks with `auto-run::` get a ⚡ before the bullet
                // so the user can see at a glance which cells re-run
                // themselves on page open.
                if has_auto_run {
                    spans.push(Span::styled("⚡", app.theme.hint));
                }
                spans.push(Span::styled("- ", bullet_style));
            }
            BlockRowKind::Continuation
            | BlockRowKind::CodeFenceMarker
            | BlockRowKind::CodeFenceBody => {
                // Indent the continuation rows by the same width
                // the ⚡ added on the bullet row so columns align.
                if has_auto_run {
                    spans.push(Span::raw(" "));
                }
                spans.push(Span::raw("  "));
            }
        }

        // If the cursor is on this row we always go raw — we want
        // bytes to line up with what the user typed, regardless of
        // fence state.
        if let (Some(col), Some(style)) = (row.cursor_col, cursor_style) {
            emit_row_with_cursor(row.text, col, style, &app.theme, &mut spans);
        } else {
            // A bullet row whose text opens a code fence (`` ```lisp ``)
            // is *both* a bullet and a fence marker — style the text
            // dimly so the fence reads visually like the rest of the
            // code block while keeping the `- ` glyph emitted above.
            let bullet_is_fence_opener = matches!(row.kind, BlockRowKind::Bullet)
                && row.text.trim_start().starts_with("```");
            match row.kind {
                _ if pretty && bullet_is_fence_opener => {
                    spans.push(Span::styled(row.text.to_string(), app.theme.dim));
                }
                BlockRowKind::CodeFenceMarker if pretty => {
                    spans.push(Span::styled(row.text.to_string(), app.theme.dim));
                }
                BlockRowKind::CodeFenceBody if pretty => {
                    spans.push(Span::styled(row.text.to_string(), app.theme.code));
                }
                BlockRowKind::Bullet if single_line_pretty => {
                    let (todo_state, body) = split_todo_prefix(row.text);
                    match todo_state {
                        Some(false) => {
                            spans.push(Span::styled("☐ ", app.theme.todo_open));
                            spans.extend(render_markdown_inline(body, &app.theme, &app.index));
                        }
                        Some(true) => {
                            spans.push(Span::styled("☑ ", app.theme.todo_done));
                            for sp in render_markdown_inline(body, &app.theme, &app.index) {
                                spans.push(Span::styled(
                                    sp.content.into_owned(),
                                    sp.style.patch(app.theme.todo_done_body),
                                ));
                            }
                        }
                        None => {
                            spans.extend(render_markdown_inline(row.text, &app.theme, &app.index))
                        }
                    }
                }
                _ => spans.extend(render_markdown_inline(row.text, &app.theme, &app.index)),
            }
        }
        out.push(Line::from(spans));
    }
}

/// Draw one row with the cursor highlighted at `col` (a char index
/// into `text`). Splits the row in three: left of cursor, the char
/// under the cursor (or a thin caret if past-end), right of cursor.
fn emit_row_with_cursor(
    text: &str,
    col: usize,
    style: CursorStyle,
    theme: &Theme,
    spans: &mut Vec<Span<'static>>,
) {
    let byte = byte_index_for_char(text, col);
    let (left, right) = text.split_at(byte);
    spans.extend(highlight_inline(left, theme));
    let mut right_chars = right.chars();
    match (right_chars.next(), style) {
        (Some(ch), CursorStyle::Caret) => {
            // Thin caret BEFORE the next char.
            spans.push(Span::styled("▏", theme.cursor_caret));
            spans.push(Span::raw(ch.to_string()));
            let rest: String = right_chars.collect();
            spans.extend(highlight_inline(&rest, theme));
        }
        (Some(ch), CursorStyle::Block) => {
            // Inverted-color block cursor on the char under it.
            spans.push(Span::styled(ch.to_string(), theme.cursor_block));
            let rest: String = right_chars.collect();
            spans.extend(highlight_inline(&rest, theme));
        }
        (None, CursorStyle::Caret) => {
            spans.push(Span::styled("▏", theme.cursor_caret));
        }
        (None, CursorStyle::Block) => {
            spans.push(Span::styled("▏", theme.cursor_block));
        }
    }
}

/// Cursor visual style. `Caret` is the thin `▏` (Insert mode);
/// `Block` is the inverted single-char box (Normal mode on the
/// selected block).
#[derive(Debug, Clone, Copy)]
enum CursorStyle {
    Caret,
    Block,
}

/// Render with markdown stripped — bold/italic/code/strike applied as
/// styles, `[[ref]]` / `#tag` / `[text](url)` shown without their
/// delimiters. Used when the block is read-only (not selected, not in
/// Insert mode).
///
/// Looks up `[[ref]]` / `#tag` targets in `index` to prepend the
/// page's `icon::` when one is set. The icon is *display-only* — the
/// underlying `.md` keeps the plain `[[Title]]` / `#tag` text.
///
/// `highlight_inline` (the raw, cursor-bearing render) deliberately
/// does *not* take this path — adding a non-source glyph would
/// break column-to-byte alignment for the visible cursor.
pub(crate) fn render_markdown_inline(
    text: &str,
    theme: &Theme,
    index: &outl_md::index::WorkspaceIndex,
) -> Vec<Span<'static>> {
    let mut out = Vec::new();
    for tok in tokenize(text) {
        match tok {
            InlineTok::Plain(s) => out.push(Span::raw(s.to_string())),
            InlineTok::PageRef { name } => {
                if let Some(icon) = index.by_title(name).and_then(|p| p.icon.as_deref()) {
                    out.push(Span::styled(format!("{icon} "), theme.dim));
                }
                out.push(Span::styled(name.to_string(), theme.ref_link));
            }
            InlineTok::Tag { name } => {
                if let Some(icon) = index.by_slug(name).and_then(|p| p.icon.as_deref()) {
                    out.push(Span::styled(format!("{icon} "), theme.dim));
                }
                out.push(Span::styled(format!("#{name}"), theme.tag_link));
            }
            InlineTok::Bold { inner } => out.push(Span::styled(inner.to_string(), theme.bold)),
            InlineTok::Italic { inner, .. } => {
                out.push(Span::styled(inner.to_string(), theme.italic))
            }
            InlineTok::Strike { inner } => out.push(Span::styled(inner.to_string(), theme.strike)),
            InlineTok::Code { inner } => out.push(Span::styled(inner.to_string(), theme.code)),
            InlineTok::Link { text, .. } => out.push(Span::styled(text.to_string(), theme.md_link)),
        }
    }
    out
}

/// Render with markdown markers visible (dimmed) and inner text styled.
/// Used when the block is selected in Normal mode (so the visible cursor
/// columns match the underlying source bytes) or in Insert mode. The
/// delimiters themselves use a dim style so the formatting markers
/// don't distract.
pub(crate) fn highlight_inline(text: &str, theme: &Theme) -> Vec<Span<'static>> {
    let mut out = Vec::new();
    let dim = theme.dim;

    for tok in tokenize(text) {
        match tok {
            InlineTok::Plain(s) => out.push(Span::raw(s.to_string())),
            InlineTok::PageRef { name } => {
                out.push(Span::styled(format!("[[{name}]]"), theme.ref_link))
            }
            InlineTok::Tag { name } => out.push(Span::styled(format!("#{name}"), theme.tag_link)),
            InlineTok::Bold { inner } => {
                out.push(Span::styled("**".to_string(), dim));
                out.push(Span::styled(inner.to_string(), theme.bold));
                out.push(Span::styled("**".to_string(), dim));
            }
            InlineTok::Italic { inner, marker } => {
                let m = marker.to_string();
                out.push(Span::styled(m.clone(), dim));
                out.push(Span::styled(inner.to_string(), theme.italic));
                out.push(Span::styled(m, dim));
            }
            InlineTok::Strike { inner } => {
                out.push(Span::styled("~~".to_string(), dim));
                out.push(Span::styled(inner.to_string(), theme.strike));
                out.push(Span::styled("~~".to_string(), dim));
            }
            InlineTok::Code { inner } => {
                out.push(Span::styled("`".to_string(), dim));
                out.push(Span::styled(inner.to_string(), theme.code));
                out.push(Span::styled("`".to_string(), dim));
            }
            InlineTok::Link { text, url } => {
                out.push(Span::styled("[".to_string(), dim));
                out.push(Span::styled(text.to_string(), theme.md_link));
                out.push(Span::styled(format!("]({url})"), dim));
            }
        }
    }
    out
}
