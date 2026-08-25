use crate::app::{App, Focus, Mode};
use crate::model::Status;
use chrono::Utc;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};

const ACCENT: Color = Color::Cyan;
const DIM: Color = Color::DarkGray;

fn fmt_dur(secs: i64) -> String {
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else {
        format!("{}h{}m", secs / 3600, (secs % 3600) / 60)
    }
}

pub fn draw(f: &mut Frame, app: &App) {
    let root = Layout::vertical([Constraint::Min(3), Constraint::Length(1)]).split(f.area());
    let cols = Layout::horizontal([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(root[0]);

    draw_tasks(f, app, cols[0]);
    draw_notes(f, app, cols[1]);
    draw_status(f, app, root[1]);

    if app.mode == Mode::Help {
        draw_help(f);
    }
}

fn draw_tasks(f: &mut Frame, app: &App, area: Rect) {
    let now = Utc::now();
    let vis = app.visible();
    let items: Vec<ListItem> = vis
        .iter()
        .map(|&i| {
            let t = &app.store.tasks[i];
            let color = match t.status {
                Status::Todo => Color::White,
                Status::Doing => Color::Yellow,
                Status::Done => DIM,
            };
            let mut spans = vec![
                Span::styled(format!("{} ", t.status.glyph()), Style::default().fg(color)),
                Span::styled(
                    t.title.clone(),
                    Style::default().fg(color).add_modifier(
                        if t.status == Status::Done {
                            Modifier::CROSSED_OUT
                        } else {
                            Modifier::empty()
                        },
                    ),
                ),
            ];
            let secs = t.active_secs(now);
            if secs > 0 {
                spans.push(Span::styled(
                    format!("  {}", fmt_dur(secs)),
                    Style::default().fg(DIM),
                ));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();

    let title = if app.show_all_projects {
        " tasks · all projects ".to_string()
    } else {
        let p = app.project.rsplit('/').next().unwrap_or(&app.project);
        format!(" tasks · {} ", p)
    };
    let border = if app.focus == Focus::List { ACCENT } else { DIM };
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border))
                .title(title),
        )
        .highlight_style(Style::default().bg(Color::Rgb(40, 44, 52)).fg(ACCENT))
        .highlight_symbol("▌");
    let mut st = ListState::default();
    if !vis.is_empty() {
        st.select(Some(app.sel));
    }
    f.render_stateful_widget(list, area, &mut st);
}

fn draw_notes(f: &mut Frame, app: &App, area: Rect) {
    let border = if app.focus == Focus::Notes { ACCENT } else { DIM };
    let editing = app.mode == Mode::EditNotes;
    let text = match app.sel_task() {
        Some(i) => {
            let n = &app.store.tasks[i].notes;
            if editing {
                let c = app.notes_cursor.min(n.len());
                format!("{}▏{}", &n[..c], &n[c..])
            } else if n.is_empty() {
                "— no notes · press i to write —".to_string()
            } else {
                n.clone()
            }
        }
        None => "— no task selected —".to_string(),
    };
    let title = if editing {
        " notes · INSERT (esc to save) "
    } else {
        " notes "
    };
    f.render_widget(
        Paragraph::new(text)
            .wrap(Wrap { trim: false })
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(border))
                    .title(title),
            ),
        area,
    );
}

fn draw_status(f: &mut Frame, app: &App, area: Rect) {
    let (label, body, color) = match app.mode {
        Mode::AddTask => ("new task", app.input.clone() + "▏", Color::Green),
        Mode::EditTitle => ("rename", app.input.clone() + "▏", Color::Yellow),
        Mode::Search => ("search", app.search.clone() + "▏", Color::Magenta),
        Mode::ConfirmDelete => ("delete", "press d again to confirm, esc to cancel".into(), Color::Red),
        _ => ("twodo", app.status.clone(), ACCENT),
    };
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                format!(" {} ", label),
                Style::default().bg(color).fg(Color::Black).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::raw(body),
        ])),
        area,
    );
}

fn draw_help(f: &mut Frame) {
    let area = centered(70, 14, f.area());
    f.render_widget(Clear, area);
    let help = "
  j / k, ↓ ↑     move            a          add task
  space          cycle status    e          rename task
  1 / 2 / 3      todo/doing/done i          edit notes
  tab / shift-tab cycle panes    esc        back / save
  /              search          d d        delete task
  p         toggle all projects  q          quit

  Time in ◐ Doing is tracked per task and shown in the list.";
    f.render_widget(
        Paragraph::new(help).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(ACCENT))
                .title(" help "),
        ),
        area,
    );
}

fn centered(w: u16, h: u16, r: Rect) -> Rect {
    let x = r.x + r.width.saturating_sub(w) / 2;
    let y = r.y + r.height.saturating_sub(h) / 2;
    Rect {
        x,
        y,
        width: w.min(r.width),
        height: h.min(r.height),
    }
}
