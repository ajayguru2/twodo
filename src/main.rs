mod app;
mod herdr;
mod model;
mod scan;
mod ui;

use anyhow::Result;
use app::{App, Focus, Mode};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use model::{Status, Store};
use std::time::Duration;

fn main() -> Result<()> {
    let mut store = Store::load()?;
    store.close_stale_windows();

    let arg = std::env::args().nth(1);
    if arg.as_deref() == Some("--scan") {
        return debug_scan();
    }

    let project = arg
        .map(std::path::PathBuf::from)
        .unwrap_or(std::env::current_dir()?);
    let project = std::fs::canonicalize(&project)
        .unwrap_or(project)
        .to_string_lossy()
        .to_string();

    let mut app = App::new(store, project);
    app.start_scan();
    app.start_herdr_watch();

    std::io::stdout().execute(EnterAlternateScreen)?;
    enable_raw_mode()?;
    let mut term = ratatui::Terminal::new(ratatui::backend::CrosstermBackend::new(
        std::io::stdout(),
    ))?;
    term.clear()?;

    let res = run(&mut term, &mut app);

    disable_raw_mode()?;
    std::io::stdout().execute(LeaveAlternateScreen)?;
    let _ = app.store.save();
    res
}

/// `twodo --scan` — prints what the session scanner sees, without the TUI.
fn debug_scan() -> Result<()> {
    let t0 = std::time::Instant::now();
    let runs = scan::scan_all(None)?;
    let (mut c, mut x, mut sub) = (0, 0, 0);
    for r in &runs {
        if r.is_sub {
            sub += 1;
        }
        if r.source == "claude" {
            c += 1;
        } else {
            x += 1;
        }
    }
    println!(
        "{} runs in {:?}  (claude {}, codex {}, subagents {})",
        runs.len(),
        t0.elapsed(),
        c,
        x,
        sub
    );
    let mut by_cwd: std::collections::HashMap<&str, u32> = Default::default();
    for r in &runs {
        *by_cwd.entry(r.cwd.as_str()).or_default() += 1;
    }
    let mut v: Vec<_> = by_cwd.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1));
    println!("\ntop projects:");
    for (cwd, n) in v.into_iter().take(8) {
        println!("  {:>5}  {}", n, cwd);
    }
    let mut kinds: std::collections::HashMap<String, u32> = Default::default();
    for r in runs.iter().filter(|r| r.is_sub) {
        *kinds.entry(format!("{}:{}", r.source, r.kind)).or_default() += 1;
    }
    let mut k: Vec<_> = kinds.into_iter().collect();
    k.sort_by(|a, b| b.1.cmp(&a.1));
    println!("\nsubagents by kind:");
    for (kind, n) in k.into_iter().take(10) {
        println!("  {:>5}  {}", n, kind);
    }
    Ok(())
}

fn run<B: ratatui::backend::Backend>(
    term: &mut ratatui::Terminal<B>,
    app: &mut App,
) -> Result<()> {
    loop {
        app.poll_scan();
        app.poll_herdr();
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
        Mode::Help => {
            if matches!(code, KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q')) {
                app.mode = Mode::Normal;
            }
        }
        Mode::ConfirmDelete => match code {
            KeyCode::Char('d') => {
                app.delete_selected();
                app.mode = Mode::Normal;
                app.status = "task deleted".into();
            }
            _ => app.mode = Mode::Normal,
        },
        Mode::AgentDetail => match code {
            KeyCode::Char('y') => copy_log_path(app),
            KeyCode::Char('j') | KeyCode::Down => app.move_agent_sel(1),
            KeyCode::Char('k') | KeyCode::Up => app.move_agent_sel(-1),
            _ => app.mode = Mode::Normal,
        },
        Mode::PickKind => match code {
            KeyCode::Esc => app.mode = Mode::Normal,
            KeyCode::Char('j') | KeyCode::Down => {
                app.kind_sel = (app.kind_sel + 1) % herdr::KINDS.len()
            }
            KeyCode::Char('k') | KeyCode::Up => {
                app.kind_sel = (app.kind_sel + herdr::KINDS.len() - 1) % herdr::KINDS.len()
            }
            KeyCode::Enter => {
                let kind = herdr::KINDS[app.kind_sel].to_string();
                app.mode = Mode::Normal;
                // Relaunching with a different kind needs a fresh tab.
                if let Some(i) = app.sel_task() {
                    app.store.tasks[i].herdr = None;
                }
                app.launch_or_focus(&kind);
            }
            _ => {}
        },
        Mode::AddTask | Mode::EditTitle => text_prompt(app, code),
        Mode::Search => match code {
            KeyCode::Esc | KeyCode::Enter => {
                app.mode = Mode::Normal;
                app.clamp();
            }
            KeyCode::Backspace => {
                app.search.pop();
                app.clamp();
            }
            KeyCode::Char(c) => {
                app.search.push(c);
                app.sel = 0;
            }
            _ => {}
        },
        Mode::EditNotes => notes_edit(app, code),
    }
}

fn normal(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Char('q') => app.quit = true,
        KeyCode::Char('?') => app.mode = Mode::Help,
        KeyCode::Char('j') | KeyCode::Down => {
            if app.focus == Focus::Agents {
                app.move_agent_sel(1);
            } else {
                let n = app.visible().len();
                if n > 0 {
                    app.sel = (app.sel + 1) % n;
                }
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            if app.focus == Focus::Agents {
                app.move_agent_sel(-1);
            } else {
                let n = app.visible().len();
                if n > 0 {
                    app.sel = (app.sel + n - 1) % n;
                }
            }
        }
        KeyCode::Char('g') => app.sel = 0,
        KeyCode::Char('G') => app.sel = app.visible().len().saturating_sub(1),
        KeyCode::Char(' ') => app.cycle_status(),
        KeyCode::Char('1') => app.set_status(Status::Todo),
        KeyCode::Char('2') => app.set_status(Status::Doing),
        KeyCode::Char('3') => app.set_status(Status::Done),
        KeyCode::Char('a') => {
            app.mode = Mode::AddTask;
            app.input.clear();
        }
        KeyCode::Char('e') => {
            if let Some(i) = app.sel_task() {
                app.input = app.store.tasks[i].title.clone();
                app.mode = Mode::EditTitle;
            }
        }
        KeyCode::Enter if app.focus == Focus::Agents => {
            if app.sel_run().is_some() {
                app.mode = Mode::AgentDetail;
            }
        }
        KeyCode::Char('i') | KeyCode::Enter => {
            if let Some(i) = app.sel_task() {
                app.notes_cursor = app.store.tasks[i].notes.len();
                app.focus = Focus::Notes;
                app.mode = Mode::EditNotes;
            }
        }
        KeyCode::Char('y') if app.focus == Focus::Agents => copy_log_path(app),
        // herdr: o opens (or focuses) this task's agent tab, O picks the kind.
        KeyCode::Char('o') => {
            let kind = app
                .sel_task()
                .and_then(|i| app.store.tasks[i].herdr.as_ref().map(|h| h.kind.clone()))
                .unwrap_or_else(|| herdr::KINDS[0].to_string());
            app.launch_or_focus(&kind);
        }
        KeyCode::Char('O') => {
            if app.sel_task().is_some() {
                app.kind_sel = 0;
                app.mode = Mode::PickKind;
            }
        }
        KeyCode::Char('x') => app.close_tab(),
        KeyCode::Char('A') => {
            app.focus = Focus::Agents;
            app.agent_sel = 0;
        }
        KeyCode::Tab => {
            app.focus = app.focus.next();
            if app.focus == Focus::Agents {
                app.agent_sel = 0;
            }
        }
        KeyCode::BackTab => app.focus = app.focus.prev(),
        KeyCode::Char('/') => {
            app.mode = Mode::Search;
            app.search.clear();
            app.sel = 0;
        }
        KeyCode::Char('d') => {
            if app.sel_task().is_some() {
                app.mode = Mode::ConfirmDelete;
            }
        }
        KeyCode::Char('p') => {
            app.show_all_projects = !app.show_all_projects;
            app.sel = 0;
        }
        KeyCode::Char('r') => {
            app.status = "rescanning sessions…".into();
            app.start_scan();
        }
        KeyCode::Esc => {
            if !app.search.is_empty() {
                app.search.clear();
                app.clamp();
            }
        }
        _ => {}
    }
}

/// Puts the selected run's log file path on the clipboard, so you can go read
/// the session that burned the tokens.
fn copy_log_path(app: &mut App) {
    let Some(path) = app.sel_run().map(|r| r.file.clone()) else {
        return;
    };
    if path.is_empty() {
        app.status = "no log path for this run".into();
        return;
    }
    let copier = if cfg!(target_os = "macos") {
        "pbcopy"
    } else {
        "xclip"
    };
    let ok = std::process::Command::new(copier)
        .args(if copier == "xclip" {
            vec!["-selection", "clipboard"]
        } else {
            vec![]
        })
        .stdin(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut c| {
            use std::io::Write;
            c.stdin.take().unwrap().write_all(path.as_bytes())?;
            c.wait()
        })
        .is_ok();
    app.status = if ok {
        "log path copied".into()
    } else {
        format!("copy failed · {}", path)
    };
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
            app.focus = Focus::List;
            app.status = "notes saved".into();
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
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration as Dur, Utc};
    use model::Window;
    use scan::AgentRun;

    fn run_at(cwd: &str, mins_ago: i64, is_sub: bool) -> AgentRun {
        AgentRun {
            source: "claude".into(),
            session_id: "s".into(),
            kind: if is_sub { "Explore".into() } else { "main".into() },
            is_sub,
            cwd: cwd.into(),
            start: Utc::now() - Dur::minutes(mins_ago),
            end: Utc::now() - Dur::minutes(mins_ago),
            tool_calls: 3,
            input_tokens: 100,
            output_tokens: 10,
            file: "/tmp/session.jsonl".into(),
        }
    }

    fn store_with_windows() -> Store {
        let now = Utc::now();
        let mut s = Store::default();
        s.add("older".into(), "/proj".into());
        s.add("newer".into(), "/proj".into());
        s.add("other project".into(), "/elsewhere".into());
        // Overlapping windows: task 1 opened 60m ago, task 2 opened 30m ago.
        s.tasks[0].windows.push(Window { start: now - Dur::minutes(60), end: None });
        s.tasks[1].windows.push(Window { start: now - Dur::minutes(30), end: None });
        s.tasks[2].windows.push(Window { start: now - Dur::minutes(60), end: None });
        s
    }

    #[test]
    fn overlapping_windows_credit_the_most_recently_started_task() {
        let s = store_with_windows();
        let runs = vec![run_at("/proj", 45, false), run_at("/proj", 10, false)];
        let a = scan::attribute(&s.tasks, &runs);
        // 45m ago only task 1's window was open; 10m ago both were, newer wins.
        assert_eq!(a.get(&1).unwrap().sessions, 1);
        assert_eq!(a.get(&2).unwrap().sessions, 1);
        let total: u32 = a.values().map(|x| x.total_agents()).sum();
        assert_eq!(total, 2, "a run must never be double counted");
    }

    #[test]
    fn a_session_already_running_when_the_window_opens_still_counts() {
        let s = store_with_windows();
        // Session started 90m ago, still going. Task 2's window opened 30m ago,
        // i.e. mid-session — the work since then belongs to it.
        let mut r = run_at("/proj", 90, false);
        r.end = Utc::now();
        let a = scan::attribute(&s.tasks, &[r]);
        assert_eq!(a.get(&2).unwrap().sessions, 1);
    }

    #[test]
    fn activity_outside_the_project_or_window_is_ignored() {
        let s = store_with_windows();
        let runs = vec![
            run_at("/proj", 999, false),        // before any window opened
            run_at("/somewhere/else", 10, false), // unrelated project
        ];
        assert!(scan::attribute(&s.tasks, &runs).is_empty());
    }

    #[test]
    fn subdirectories_count_toward_the_project() {
        let s = store_with_windows();
        let runs = vec![run_at("/proj/packages/api", 10, true)];
        let a = scan::attribute(&s.tasks, &runs);
        assert_eq!(a.get(&2).unwrap().subagents, 1);
        assert_eq!(a.get(&2).unwrap().by_kind[0].0, "claude:Explore");
    }

    #[test]
    fn herdr_agent_names_satisfy_the_naming_rules() {
        // herdr: 1-32 chars, lowercase [a-z0-9-_], must start with a letter.
        for tab in ["w1:t9", "w1:tA", "w1:tD", "w99:tZZZ"] {
            let n = herdr::agent_name(tab);
            assert!(!n.is_empty() && n.len() <= 32, "{} -> {} length", tab, n);
            assert!(n.starts_with(|c: char| c.is_ascii_lowercase()), "{}", n);
            assert!(
                n.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
                "{} produced invalid name {}",
                tab,
                n
            );
        }
        // The uppercase tab ids are what broke this in practice.
        assert_eq!(herdr::agent_name("w1:tD"), "twodo-w1-td");
    }

    #[test]
    fn tab_reaches_the_agents_pane_and_lists_individual_runs() {
        let s = store_with_windows();
        let runs = vec![run_at("/proj", 10, false), run_at("/proj", 5, true)];
        let mut app = App::new(s, "/proj".into());
        app.agents = scan::attribute(&app.store.tasks, &runs);
        app.sel = 1; // the task the runs belong to

        // Tab twice: list -> notes -> agents.
        handle(&mut app, KeyCode::Tab);
        handle(&mut app, KeyCode::Tab);
        assert_eq!(app.focus, Focus::Agents);
        assert_eq!(app.runs().len(), 2, "both runs should be listed");

        // j moves within the agents pane, not the task list.
        let task_before = app.sel;
        handle(&mut app, KeyCode::Char('j'));
        assert_eq!(app.agent_sel, 1);
        assert_eq!(app.sel, task_before, "task selection must not move");

        // Newest first, so index 0 is the 5-minutes-ago subagent.
        app.agent_sel = 0;
        assert!(app.sel_run().unwrap().is_sub);

        let render = |app: &App| -> String {
            let mut term =
                ratatui::Terminal::new(ratatui::backend::TestBackend::new(100, 30)).unwrap();
            term.draw(|f| ui::draw(f, app)).unwrap();
            term.backend().buffer().content().iter().map(|c| c.symbol()).collect()
        };

        // The pane lists each run individually, not just the rolled-up totals.
        let pane = render(&app);
        assert!(pane.contains("claude:Explore"), "subagent run should be listed");
        assert!(pane.contains("claude:main"), "session run should be listed");

        handle(&mut app, KeyCode::Enter);
        assert_eq!(app.mode, Mode::AgentDetail);
        let detail = render(&app);
        assert!(detail.contains("agent run"), "detail popup should be open");
        assert!(detail.contains("subagent · Explore"), "detail should name the run");
        // Path is shown as dir + filename on separate lines so it never wraps.
        assert!(detail.contains("/tmp/"), "detail should show the log dir");
        assert!(detail.contains("session.jsonl"), "detail should show the log file");

        // Any other key dismisses the popup.
        handle(&mut app, KeyCode::Esc);
        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn ui_renders_tasks_and_agent_counts() {
        let s = store_with_windows();
        let runs = vec![run_at("/proj", 10, false), run_at("/proj", 10, true)];
        let mut app = App::new(s, "/proj".into());
        app.agents = scan::attribute(&app.store.tasks, &runs);

        let backend = ratatui::backend::TestBackend::new(100, 30);
        let mut term = ratatui::Terminal::new(backend).unwrap();
        term.draw(|f| ui::draw(f, &app)).unwrap();
        let text: String = term.backend().buffer().content().iter().map(|c| c.symbol()).collect();

        assert!(text.contains("older") && text.contains("newer"));
        assert!(!text.contains("other project"), "other projects must be filtered out");
        assert!(text.contains("⛁2"), "agent badge should show 2 agents");
        assert!(text.contains("agents"));
    }
}
