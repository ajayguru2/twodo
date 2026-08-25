use crate::app::{App, Mode};
use crate::model::Status;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
use std::path::Path;

const INK: Color = Color::Rgb(10, 12, 19);
const SURFACE: Color = Color::Rgb(18, 21, 32);
const TEXT: Color = Color::Rgb(232, 234, 242);
const MUTED: Color = Color::Rgb(126, 139, 164);
const VIOLET: Color = Color::Rgb(174, 143, 255);
const MINT: Color = Color::Rgb(115, 218, 176);
const CORAL: Color = Color::Rgb(255, 123, 125);
const SELECTED: Color = Color::Rgb(35, 31, 53);

pub fn draw(f: &mut Frame, app: &App) {
    f.render_widget(Block::default().style(Style::default().bg(INK)), f.area());
    let area = workbench(f.area());

    if app.mode == Mode::EditNotes {
        draw_note(f, app, area);
    } else {
        draw_index(f, app, area);
    }

    match app.mode {
        Mode::AddTask => draw_text_prompt(f, area, "new task", &app.input),
        Mode::EditTitle => draw_text_prompt(f, area, "rename task", &app.input),
        Mode::ConfirmDelete => draw_delete_prompt(f, app, area),
        Mode::Normal | Mode::EditNotes => {}
    }
}

fn draw_index(f: &mut Frame, app: &App, area: Rect) {
    let visible = app.visible();
    let done = visible
        .iter()
        .filter(|&&i| app.store.tasks[i].status == Status::Done)
        .count();
    let open = visible.len() - done;
    let project = Path::new(&app.project)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(&app.project);
    let footer_height = if area.width >= 72 { 2 } else { 3 };
    let rows = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(1),
        Constraint::Length(footer_height),
    ])
    .split(area);

    let header = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(
                "twodo",
                Style::default().fg(VIOLET).add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!("  /  {project}"), Style::default().fg(MUTED)),
        ]),
        Line::from(vec![
            Span::styled(format!("{open} open"), Style::default().fg(TEXT)),
            Span::styled("  ·  ", Style::default().fg(MUTED)),
            Span::styled(format!("{done} done"), Style::default().fg(MINT)),
        ]),
    ])
    .block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(SURFACE)),
    );
    f.render_widget(header, rows[0]);

    if visible.is_empty() {
        f.render_widget(
            Paragraph::new(vec![
                Line::from(""),
                Line::from(vec![
                    Span::styled("▌ ", Style::default().fg(VIOLET)),
                    Span::styled("No tasks yet.", Style::default().fg(TEXT)),
                ]),
                Line::from(vec![
                    Span::raw("  Press "),
                    Span::styled(
                        "n",
                        Style::default().fg(VIOLET).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(" to add one.", Style::default().fg(MUTED)),
                ]),
            ])
            .style(Style::default().bg(INK).fg(MUTED)),
            rows[1],
        );
    } else {
        let items: Vec<ListItem> = visible
            .iter()
            .enumerate()
            .map(|(row, &i)| {
                let task = &app.store.tasks[i];
                let selected = row == app.sel;
                let (mark, mark_color) = match task.status {
                    Status::Todo | Status::Doing => ("○", MUTED),
                    Status::Done => ("✓", MINT),
                };
                let title_style = if task.status == Status::Done {
                    Style::default()
                        .fg(MUTED)
                        .add_modifier(Modifier::CROSSED_OUT)
                } else {
                    Style::default().fg(TEXT)
                };

                ListItem::new(Line::from(vec![
                    Span::styled(
                        if selected { "▌ " } else { "  " },
                        Style::default().fg(VIOLET),
                    ),
                    Span::styled(format!("{:>3}  ", row + 1), Style::default().fg(MUTED)),
                    Span::styled(format!("{mark}  "), Style::default().fg(mark_color)),
                    Span::styled(task.title.clone(), title_style),
                ]))
            })
            .collect();
        let list = List::new(items)
            .highlight_style(Style::default().bg(SELECTED).add_modifier(Modifier::BOLD));
        let mut state = ListState::default();
        if app.sel < visible.len() {
            state.select(Some(app.sel));
        }
        f.render_stateful_widget(list, rows[1], &mut state);
    }

    f.render_widget(index_footer(area.width), rows[2]);
}

fn draw_note(f: &mut Frame, app: &App, area: Rect) {
    let footer_height = if area.width >= 48 { 2 } else { 3 };
    let rows = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(1),
        Constraint::Length(footer_height),
    ])
    .split(area);
    let Some(i) = app.sel_task() else {
        return;
    };
    let task = &app.store.tasks[i];
    let cursor = cursor_boundary(&task.notes, app.notes_cursor);

    f.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled(
                    "twodo",
                    Style::default().fg(VIOLET).add_modifier(Modifier::BOLD),
                ),
                Span::styled("  note", Style::default().fg(MUTED)),
            ]),
            Line::from(Span::styled(
                task.title.clone(),
                Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
            )),
        ])
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(SURFACE)),
        ),
        rows[0],
    );

    let cursor_row = task.notes[..cursor].bytes().filter(|&b| b == b'\n').count();
    let viewport = usize::from(rows[1].height.max(1));
    let scroll = cursor_row.saturating_sub(viewport.saturating_sub(1));
    f.render_widget(
        Paragraph::new(note_lines(&task.notes, cursor))
            .style(Style::default().bg(INK).fg(TEXT))
            .wrap(Wrap { trim: false })
            .scroll((scroll.min(u16::MAX as usize) as u16, 0))
            .block(
                Block::default()
                    .borders(Borders::LEFT)
                    .border_style(Style::default().fg(VIOLET)),
            ),
        rows[1],
    );

    let footer = if area.width >= 48 {
        Line::from(vec![
            key("esc"),
            label(" save & back   "),
            key("tab"),
            label(" indent"),
        ])
    } else {
        Line::from(vec![
            key("esc"),
            label(" save   "),
            key("tab"),
            label(" indent"),
        ])
    };
    f.render_widget(
        Paragraph::new(footer).block(
            Block::default()
                .borders(Borders::TOP)
                .border_style(Style::default().fg(SURFACE)),
        ),
        rows[2],
    );
}

fn note_lines(note: &str, cursor: usize) -> Vec<Line<'static>> {
    if note.is_empty() {
        return vec![Line::from(vec![
            Span::raw("  "),
            Span::styled(
                "▏",
                Style::default().fg(VIOLET).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" Write anything.", Style::default().fg(MUTED)),
        ])];
    }

    let before = &note[..cursor];
    let cursor_row = before.bytes().filter(|&b| b == b'\n').count();
    let cursor_column = before.rsplit('\n').next().map_or(0, str::len);
    note.split('\n')
        .enumerate()
        .map(|(row, line)| {
            if row == cursor_row {
                let (left, right) = line.split_at(cursor_column);
                Line::from(vec![
                    Span::raw("  "),
                    Span::styled(left.to_owned(), Style::default().fg(TEXT)),
                    Span::styled(
                        "▏",
                        Style::default().fg(VIOLET).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(right.to_owned(), Style::default().fg(TEXT)),
                ])
            } else {
                Line::from(vec![
                    Span::raw("  "),
                    Span::styled(line.to_owned(), Style::default().fg(TEXT)),
                ])
            }
        })
        .collect()
}

fn draw_text_prompt(f: &mut Frame, area: Rect, title: &str, input: &str) {
    let prompt = centered(area, 56, 5);
    f.render_widget(Clear, prompt);
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(input.to_owned(), Style::default().fg(TEXT)),
            Span::styled(
                "▏",
                Style::default().fg(VIOLET).add_modifier(Modifier::BOLD),
            ),
        ]))
        .style(Style::default().bg(SURFACE))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(VIOLET))
                .title(Span::styled(
                    format!(" {title} "),
                    Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
                ))
                .title_bottom(
                    Line::from(vec![
                        key("enter"),
                        label(" save  "),
                        key("esc"),
                        label(" cancel"),
                    ])
                    .centered(),
                ),
        ),
        prompt,
    );
}

fn draw_delete_prompt(f: &mut Frame, app: &App, area: Rect) {
    let prompt = centered(area, 56, 6);
    f.render_widget(Clear, prompt);
    let title = app
        .sel_task()
        .map(|i| app.store.tasks[i].title.as_str())
        .unwrap_or("this task");
    f.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                "Delete this task?",
                Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(title.to_owned(), Style::default().fg(MUTED))),
        ])
        .style(Style::default().bg(SURFACE))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(CORAL))
                .title(Span::styled(
                    " delete ",
                    Style::default().fg(CORAL).add_modifier(Modifier::BOLD),
                ))
                .title_bottom(
                    Line::from(vec![
                        key_danger("d"),
                        label(" confirm  "),
                        key("esc"),
                        label(" cancel"),
                    ])
                    .centered(),
                ),
        ),
        prompt,
    );
}

fn index_footer(width: u16) -> Paragraph<'static> {
    let lines = if width >= 72 {
        vec![Line::from(vec![
            key("↑↓"),
            label(" move   "),
            key("enter"),
            label(" note   "),
            key("n"),
            label(" add   "),
            key("e"),
            label(" rename   "),
            key("space"),
            label(" done   "),
            key("d"),
            label(" delete   "),
            key("q"),
            label(" quit"),
        ])]
    } else {
        vec![
            Line::from(vec![
                key("↑↓"),
                label(" move   "),
                key("enter"),
                label(" note   "),
                key("n"),
                label(" add   "),
                key("space"),
                label(" done"),
            ]),
            Line::from(vec![
                key("e"),
                label(" rename   "),
                key("d"),
                label(" delete   "),
                key("q"),
                label(" quit"),
            ]),
        ]
    };
    Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(SURFACE)),
    )
}

fn key(text: &'static str) -> Span<'static> {
    Span::styled(
        text,
        Style::default().fg(VIOLET).add_modifier(Modifier::BOLD),
    )
}

fn key_danger(text: &'static str) -> Span<'static> {
    Span::styled(
        text,
        Style::default().fg(CORAL).add_modifier(Modifier::BOLD),
    )
}

fn label(text: &'static str) -> Span<'static> {
    Span::styled(text, Style::default().fg(MUTED))
}

fn cursor_boundary(text: &str, cursor: usize) -> usize {
    let mut cursor = cursor.min(text.len());
    while !text.is_char_boundary(cursor) {
        cursor -= 1;
    }
    cursor
}

fn workbench(area: Rect) -> Rect {
    let margin = u16::from(area.height > 12);
    let width = area.width.min(86);
    Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + margin,
        width,
        area.height.saturating_sub(margin * 2),
    )
}

fn centered(area: Rect, max_width: u16, height: u16) -> Rect {
    let width = max_width.min(area.width.saturating_sub(2).max(1));
    let height = height.min(area.height.saturating_sub(2).max(1));
    Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Store;

    #[test]
    fn cursor_stays_on_a_character_boundary() {
        assert_eq!(cursor_boundary("aé", 2), 1);
    }

    #[test]
    fn compact_workbench_keeps_the_task_and_note_controls_visible() {
        let project = "/work/twodo";
        let mut store = Store::default();
        store.add("Fix flaky parser".into(), project.into());
        let mut app = App::new(store, project.into());
        let backend = ratatui::backend::TestBackend::new(60, 16);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();

        terminal.draw(|frame| draw(frame, &app)).unwrap();
        let index: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(index.contains("Fix flaky parser"));
        assert!(index.contains("enter") && index.contains("space") && index.contains("quit"));

        app.store.tasks[0].notes = (1..=20)
            .map(|line| format!("note line {line}"))
            .collect::<Vec<_>>()
            .join("\n");
        app.notes_cursor = app.store.tasks[0].notes.len();
        app.mode = Mode::EditNotes;
        terminal.draw(|frame| draw(frame, &app)).unwrap();
        let note: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(note.contains("note line 20"));
        assert!(note.contains("save & back"));
    }
}
