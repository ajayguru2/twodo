use crate::app::{App, Focus, Mode};
use crate::model::Status;
use chrono::{Local, Utc};
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

fn fmt_tok(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1e6)
    } else if n >= 1_000 {
        format!("{:.1}k", n as f64 / 1e3)
    } else {
        n.to_string()
    }
}

pub fn draw(f: &mut Frame, app: &App) {
    let root = Layout::vertical([Constraint::Min(3), Constraint::Length(1)]).split(f.area());
    let cols = Layout::horizontal([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(root[0]);
    // The agents pane takes the lion's share once you focus it.
    let right = if app.focus == Focus::Agents {
        Layout::vertical([Constraint::Length(6), Constraint::Min(8)]).split(cols[1])
    } else {
        Layout::vertical([Constraint::Min(5), Constraint::Length(11)]).split(cols[1])
    };

    draw_tasks(f, app, cols[0]);
    draw_notes(f, app, right[0]);
    draw_agents(f, app, right[1]);
    draw_status(f, app, root[1]);

    if app.mode == Mode::Help {
        draw_help(f);
    }
    if app.mode == Mode::AgentDetail {
        draw_agent_detail(f, app);
    }
    if app.mode == Mode::PickKind {
        draw_kind_picker(f, app);
    }
}

fn draw_kind_picker(f: &mut Frame, app: &App) {
    let area = centered(38, (crate::herdr::KINDS.len() + 4) as u16, f.area());
    f.render_widget(Clear, area);
    let items: Vec<ListItem> = crate::herdr::KINDS
        .iter()
        .enumerate()
        .map(|(i, k)| {
            let sel = i == app.kind_sel;
            ListItem::new(Line::from(Span::styled(
                format!(" {} {}", if sel { "▌" } else { " " }, k),
                if sel {
                    Style::default().bg(Color::Rgb(40, 44, 52)).fg(ACCENT)
                } else {
                    Style::default()
                },
            )))
        })
        .collect();
    f.render_widget(
        List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(ACCENT))
                .title(" launch agent · enter · esc "),
        ),
        area,
    );
}

fn draw_tasks(f: &mut Frame, app: &App, area: Rect) {
    let now = Utc::now();
    let vis = app.visible();
    let items: Vec<ListItem> = vis
        .iter()
        .map(|&i| {
            let t = &app.store.tasks[i];
            let agents = app.agents.get(&t.id);
            let n = agents.map(|a| a.total_agents()).unwrap_or(0);
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
            // Live herdr tab state, if this task has one running.
            if let Some(state) = app.herdr_state(t) {
                let (glyph, c) = match state {
                    "working" => ("◉", Color::Green),
                    "done" => ("✔", Color::Magenta),
                    "idle" => ("◎", Color::Blue),
                    _ => ("◌", DIM),
                };
                spans.push(Span::styled(
                    format!("  {}", glyph),
                    Style::default().fg(c).add_modifier(Modifier::BOLD),
                ));
            }
            if n > 0 {
                spans.push(Span::styled(
                    format!("  ⛁{}", n),
                    Style::default().fg(ACCENT),
                ));
            }
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

fn draw_agents(f: &mut Frame, app: &App, area: Rect) {
    let focused = app.focus == Focus::Agents;
    let mut lines: Vec<Line> = Vec::new();
    match app.sel_task().map(|i| &app.store.tasks[i]) {
        Some(t) => match app.agents.get(&t.id) {
            Some(a) => {
                lines.push(Line::from(vec![
                    Span::styled(
                        plural(a.total_agents(), "agent"),
                        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled("  ", Style::default().fg(DIM)),
                    Span::raw(format!("{}  ", plural(a.sessions, "session"))),
                    Span::raw(plural(a.subagents, "subagent")),
                ]));
                lines.push(Line::from(Span::styled(
                    format!(
                        "{}  ·  {} in / {} out tokens",
                        plural(a.tool_calls, "tool call"),
                        fmt_tok(a.input_tokens),
                        fmt_tok(a.output_tokens)
                    ),
                    Style::default().fg(DIM),
                )));
                lines.push(Line::from(""));
                if focused {
                    // Focused: the individual runs, newest first, selectable.
                    let rows = (area.height as usize).saturating_sub(5);
                    let start = app.agent_sel.saturating_sub(rows.saturating_sub(1));
                    for (i, r) in a.runs.iter().enumerate().skip(start).take(rows) {
                        let sel = i == app.agent_sel;
                        let mark = if r.is_sub { "└" } else { "◆" };
                        let style = if sel {
                            Style::default().bg(Color::Rgb(40, 44, 52)).fg(ACCENT)
                        } else {
                            Style::default()
                        };
                        lines.push(Line::from(vec![
                            Span::styled(
                                format!(
                                    "{} {} {:<24}",
                                    if sel { "▌" } else { " " },
                                    mark,
                                    truncate(&format!("{}:{}", r.source, r.kind), 24)
                                ),
                                style,
                            ),
                            Span::styled(
                                format!(
                                    "{}  {:>4}  {:>6} tok",
                                    r.start.with_timezone(&Local).format("%m-%d %H:%M"),
                                    r.tool_calls,
                                    fmt_tok(r.input_tokens + r.output_tokens)
                                ),
                                Style::default().fg(DIM),
                            ),
                        ]));
                    }
                    if a.runs.len() > rows {
                        lines.push(Line::from(Span::styled(
                            format!("  … {} more", a.runs.len() - rows),
                            Style::default().fg(DIM),
                        )));
                    }
                } else {
                    let max = a.by_kind.first().map(|k| k.1).unwrap_or(1).max(1);
                    for (kind, n) in a.by_kind.iter().take(6) {
                        let w = ((*n as f64 / max as f64) * 18.0).round() as usize;
                        lines.push(Line::from(vec![
                            Span::styled(format!("{:<22}", truncate(kind, 22)), Style::default()),
                            Span::styled("█".repeat(w.max(1)), Style::default().fg(ACCENT)),
                            Span::styled(format!(" {}", n), Style::default().fg(DIM)),
                        ]));
                    }
                }
            }
            None => lines.push(Line::from(Span::styled(
                if t.windows.is_empty() {
                    "No agent activity yet. Mark the task ◐ Doing (space) to start attributing."
                } else {
                    "No agent activity recorded in this task's windows."
                },
                Style::default().fg(DIM),
            ))),
        },
        None => lines.push(Line::from(Span::styled(
            "— no task selected —",
            Style::default().fg(DIM),
        ))),
    }
    f.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(if focused { ACCENT } else { DIM }))
                .title(if focused {
                    " agents · enter for detail "
                } else {
                    " agents "
                }),
        ),
        area,
    );
}

fn draw_agent_detail(f: &mut Frame, app: &App) {
    let Some(r) = app.sel_run() else { return };
    let area = centered(88, 16, f.area());
    f.render_widget(Clear, area);
    let dur = (r.end - r.start).num_seconds().max(0);
    // Split so a long log path never wraps mid-word.
    let (log_dir, log_name) = match r.file.rsplit_once('/') {
        Some((d, n)) => (format!("{}/", d), n.to_string()),
        None => (String::new(), r.file.clone()),
    };
    let body = format!(
        "\n  {}  {}\n\n  \
         session   {}\n  \
         started   {}\n  \
         ended     {}\n  \
         duration  {}\n  \
         cwd       {}\n  \
         activity  {} · {} in / {} out tokens\n\n  \
         log       {}\n            {}\n",
        r.source,
        if r.is_sub {
            format!("subagent · {}", r.kind)
        } else {
            "main session".to_string()
        },
        r.session_id,
        r.start.with_timezone(&Local).format("%Y-%m-%d %H:%M:%S"),
        r.end.with_timezone(&Local).format("%Y-%m-%d %H:%M:%S"),
        fmt_dur(dur),
        tilde(&r.cwd),
        plural(r.tool_calls, "tool call"),
        fmt_tok(r.input_tokens),
        fmt_tok(r.output_tokens),
        tilde(&log_dir),
        log_name,
    );
    f.render_widget(
        Paragraph::new(body).wrap(Wrap { trim: false }).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(ACCENT))
                .title(" agent run · y copy log path · esc close "),
        ),
        area,
    );
}

/// Shortens `$HOME` to `~` so long log paths fit on one line.
fn tilde(p: &str) -> String {
    match dirs::home_dir() {
        Some(h) => p.replacen(&h.to_string_lossy().to_string(), "~", 1),
        None => p.to_string(),
    }
}

pub fn plural(n: u32, word: &str) -> String {
    if n == 1 {
        format!("{} {}", n, word)
    } else {
        format!("{} {}s", n, word)
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        s.chars().take(n - 1).collect::<String>() + "…"
    }
}

fn draw_status(f: &mut Frame, app: &App, area: Rect) {
    let (label, body, color) = match app.mode {
        Mode::AddTask => ("new task", app.input.clone() + "▏", Color::Green),
        Mode::EditTitle => ("rename", app.input.clone() + "▏", Color::Yellow),
        Mode::Search => ("search", app.search.clone() + "▏", Color::Magenta),
        Mode::ConfirmDelete => ("delete", "press d again to confirm, esc to cancel".into(), Color::Red),
        _ => (
            if app.scanning { "scanning" } else { "twodo" },
            app.status.clone(),
            ACCENT,
        ),
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
    let area = centered(70, 24, f.area());
    f.render_widget(Clear, area);
    let help = "
  j / k, ↓ ↑     move            a          add task
  space          cycle status    e          rename task
  1 / 2 / 3      todo/doing/done i          edit notes
  tab / shift-tab cycle panes    esc        back / save
  /              search          d d        delete task
  r              rescan agents   p          toggle all projects
  A              jump to agents  q          quit

  o   launch (or focus) this task's herdr agent tab
  O   launch with a different agent kind    x  close the tab

  In the agents pane: j / k pick a run, enter shows its detail,
  y copies the session log path.

  Agents are read straight from ~/.claude/projects and
  ~/.codex/sessions. A task collects the agent activity that
  happened in its project while it was marked ◐ Doing.";
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
