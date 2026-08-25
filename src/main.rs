mod app;
mod model;
mod ui;

use anyhow::Result;
use app::{App, Mode};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use model::Store;
use std::time::Duration;

fn main() -> Result<()> {
    let mut store = Store::load()?;
    store.close_stale_windows();

    let project = std::env::args()
        .nth(1)
        .map(std::path::PathBuf::from)
        .unwrap_or(std::env::current_dir()?);
    let project = std::fs::canonicalize(&project)
        .unwrap_or(project)
        .to_string_lossy()
        .to_string();

    let mut app = App::new(store, project);

    std::io::stdout().execute(EnterAlternateScreen)?;
    enable_raw_mode()?;
    let mut term =
        ratatui::Terminal::new(ratatui::backend::CrosstermBackend::new(std::io::stdout()))?;
    term.clear()?;

    let res = run(&mut term, &mut app);

    disable_raw_mode()?;
    std::io::stdout().execute(LeaveAlternateScreen)?;
    let _ = app.store.save();
    res
}

fn run<B: ratatui::backend::Backend>(term: &mut ratatui::Terminal<B>, app: &mut App) -> Result<()> {
    loop {
        term.draw(|f| ui::draw(f, app))?;
        if !event::poll(Duration::from_millis(200))? {
            continue;
        }
        if let Event::Key(k) = event::read()? {
            if k.kind != KeyEventKind::Press {
                continue;
            }
            if k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('c') {
                return Ok(());
            }
            handle(app, k.code);
            if app.quit {
                return Ok(());
            }
        }
    }
}

fn handle(app: &mut App, code: KeyCode) {
    match app.mode {
        Mode::Normal => normal(app, code),
        Mode::ConfirmDelete => match code {
            KeyCode::Char('d') => {
                app.delete_selected();
                app.mode = Mode::Normal;
            }
            _ => app.mode = Mode::Normal,
        },
        Mode::AddTask | Mode::EditTitle => text_prompt(app, code),
        Mode::EditNotes => notes_edit(app, code),
    }
}

fn normal(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Char('q') => app.quit = true,
        KeyCode::Char('j') | KeyCode::Down => {
            let n = app.visible().len();
            if n > 0 {
                app.sel = (app.sel + 1) % n;
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            let n = app.visible().len();
            if n > 0 {
                app.sel = (app.sel + n - 1) % n;
            }
        }
        KeyCode::Char('g') => app.sel = 0,
        KeyCode::Char('G') => app.sel = app.visible().len().saturating_sub(1),
        KeyCode::Char(' ') => app.toggle_done(),
        KeyCode::Char('a') | KeyCode::Char('n') => {
            app.mode = Mode::AddTask;
            app.input.clear();
        }
        KeyCode::Char('e') => {
            if let Some(i) = app.sel_task() {
                app.input = app.store.tasks[i].title.clone();
                app.mode = Mode::EditTitle;
            }
        }
        KeyCode::Char('i') | KeyCode::Enter => {
            if let Some(i) = app.sel_task() {
                app.notes_cursor = app.store.tasks[i].notes.len();
                app.mode = Mode::EditNotes;
            }
        }
        KeyCode::Char('d') if app.sel_task().is_some() => app.mode = Mode::ConfirmDelete,
        _ => {}
    }
}

fn text_prompt(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Esc => app.mode = Mode::Normal,
        KeyCode::Enter => {
            let text = app.input.trim().to_string();
            if !text.is_empty() {
                if app.mode == Mode::AddTask {
                    let project = app.project.clone();
                    app.store.add(text, project);
                    let vis = app.visible();
                    app.sel = vis.len().saturating_sub(1);
                } else if let Some(i) = app.sel_task() {
                    app.store.tasks[i].title = text;
                }
                let _ = app.store.save();
            }
            app.input.clear();
            app.mode = Mode::Normal;
        }
        KeyCode::Backspace => {
            app.input.pop();
        }
        KeyCode::Char(c) => app.input.push(c),
        _ => {}
    }
}

fn notes_edit(app: &mut App, code: KeyCode) {
    let Some(i) = app.sel_task() else {
        app.mode = Mode::Normal;
        return;
    };
    let n = &mut app.store.tasks[i].notes;
    let cur = app.notes_cursor.min(n.len());
    match code {
        KeyCode::Esc => {
            let _ = app.store.save();
            app.mode = Mode::Normal;
        }
        KeyCode::Char(c) => {
            n.insert(cur, c);
            app.notes_cursor = cur + c.len_utf8();
        }
        KeyCode::Enter => {
            n.insert(cur, '\n');
            app.notes_cursor = cur + 1;
        }
        KeyCode::Backspace => {
            if cur > 0 {
                let prev = n[..cur]
                    .char_indices()
                    .next_back()
                    .map(|(i, _)| i)
                    .unwrap_or(0);
                n.remove(prev);
                app.notes_cursor = prev;
            }
        }
        KeyCode::Left => {
            app.notes_cursor = n[..cur]
                .char_indices()
                .next_back()
                .map(|(i, _)| i)
                .unwrap_or(0);
        }
        KeyCode::Right => {
            app.notes_cursor = n[cur..]
                .char_indices()
                .nth(1)
                .map(|(o, _)| cur + o)
                .unwrap_or(n.len());
        }
        KeyCode::Up => app.notes_cursor = move_vertical(n, cur, false),
        KeyCode::Down => app.notes_cursor = move_vertical(n, cur, true),
        KeyCode::Home => app.notes_cursor = line_start(n, cur),
        KeyCode::End => app.notes_cursor = line_end(n, cur),
        KeyCode::Delete if cur < n.len() => {
            n.remove(cur);
        }
        KeyCode::Tab => {
            n.insert_str(cur, "  ");
            app.notes_cursor = cur + 2;
        }
        _ => {}
    }
}

fn line_start(text: &str, cursor: usize) -> usize {
    text[..cursor].rfind('\n').map_or(0, |i| i + 1)
}

fn line_end(text: &str, cursor: usize) -> usize {
    text[cursor..].find('\n').map_or(text.len(), |i| cursor + i)
}

fn move_vertical(text: &str, cursor: usize, down: bool) -> usize {
    let start = line_start(text, cursor);
    let column = text[start..cursor].chars().count();
    let (target_start, target_end) = if down {
        let end = line_end(text, cursor);
        if end == text.len() {
            return cursor;
        }
        let target_start = end + 1;
        (target_start, line_end(text, target_start))
    } else {
        if start == 0 {
            return cursor;
        }
        let target_end = start - 1;
        (line_start(text, target_end), target_end)
    };
    let line = &text[target_start..target_end];
    target_start
        + line
            .char_indices()
            .nth(column)
            .map_or(line.len(), |(i, _)| i)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration as Dur, Utc};
    use model::{Status, Window};

    fn store_with_windows() -> Store {
        let now = Utc::now();
        let mut s = Store::default();
        s.add("older".into(), "/proj".into());
        s.add("newer".into(), "/proj".into());
        s.add("other project".into(), "/elsewhere".into());
        s.tasks[0].windows.push(Window {
            start: now - Dur::minutes(60),
            end: None,
        });
        s.tasks[1].windows.push(Window {
            start: now - Dur::minutes(30),
            end: None,
        });
        s.tasks[2].windows.push(Window {
            start: now - Dur::minutes(60),
            end: None,
        });
        s
    }

    #[test]
    fn doing_opens_a_window_and_leaving_doing_closes_it() {
        let mut s = Store::default();
        s.add("t".into(), "/proj".into());
        s.set_status(0, Status::Doing);
        assert_eq!(s.tasks[0].windows.len(), 1);
        s.set_status(0, Status::Doing); // already doing: no second window
        assert_eq!(s.tasks[0].windows.len(), 1);
        s.set_status(0, Status::Done);
        assert!(s.tasks[0].windows[0].end.is_some(), "window must close");
        let end = s.tasks[0].windows[0].end;
        s.set_status(0, Status::Todo);
        assert_eq!(s.tasks[0].windows[0].end, end, "closed time is fixed");
    }

    #[test]
    fn a_window_left_open_by_a_crash_does_not_accrue_time() {
        let mut s = store_with_windows();
        s.tasks[0].status = Status::Todo; // open window, not Doing
        s.close_stale_windows();
        assert_eq!(s.tasks[0].windows[0].end, Some(s.tasks[0].windows[0].start));
    }

    #[test]
    fn list_opens_the_selected_tasks_note() {
        let mut app = App::new(store_with_windows(), "/proj".into());
        app.store.tasks[0].notes = "Trace the parser before changing it.".into();

        let backend = ratatui::backend::TestBackend::new(100, 30);
        let mut term = ratatui::Terminal::new(backend).unwrap();
        term.draw(|f| ui::draw(f, &app)).unwrap();
        let list: String = term
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect();

        assert!(list.contains("older") && list.contains("newer"));
        assert!(
            !list.contains("other project"),
            "other projects must be filtered out"
        );

        handle(&mut app, KeyCode::Enter);
        assert_eq!(app.mode, Mode::EditNotes);
        term.draw(|f| ui::draw(f, &app)).unwrap();
        let note: String = term
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect();

        assert!(note.contains("older"));
        assert!(note.contains("Trace the parser before changing it."));
    }

    #[test]
    fn vertical_note_movement_uses_character_columns() {
        let note = "aéx\n1234";
        let first_line = "aé".len();
        let second_line = "aéx\n12".len();

        assert_eq!(move_vertical(note, first_line, true), second_line);
        assert_eq!(move_vertical(note, second_line, false), first_line);
    }
}
