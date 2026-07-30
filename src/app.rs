use crate::model::{Status, Store};
use crate::scan::{AgentRun, TaskAgents};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender};

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum Focus {
    List,
    Notes,
    Agents,
}

impl Focus {
    pub fn next(self) -> Focus {
        match self {
            Focus::List => Focus::Notes,
            Focus::Notes => Focus::Agents,
            Focus::Agents => Focus::List,
        }
    }
    pub fn prev(self) -> Focus {
        self.next().next()
    }
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum Mode {
    Normal,
    AddTask,
    EditTitle,
    EditNotes,
    Search,
    Help,
    ConfirmDelete,
    AgentDetail,
    PickKind,
}

/// Messages from background herdr work back to the UI thread.
pub enum HerdrMsg {
    Launched(u64, crate::model::HerdrTab),
    Failed(u64, String),
    Statuses(HashMap<String, String>),
}

pub struct App {
    pub store: Store,
    pub project: String,
    pub sel: usize,
    pub mode: Mode,
    pub focus: Focus,
    pub input: String,
    pub search: String,
    pub agents: HashMap<u64, TaskAgents>,
    pub scanning: bool,
    pub status: String,
    pub notes_cursor: usize,
    pub agent_sel: usize,
    pub show_all_projects: bool,
    pub rx: Receiver<Vec<AgentRun>>,
    pub tx: Sender<Vec<AgentRun>>,
    /// tab_id -> live herdr agent status.
    pub herdr_status: HashMap<String, String>,
    pub herdr_ok: bool,
    pub kind_sel: usize,
    pub hrx: Receiver<HerdrMsg>,
    pub htx: Sender<HerdrMsg>,
    pub quit: bool,
}

impl App {
    pub fn new(store: Store, project: String) -> App {
        let (tx, rx) = std::sync::mpsc::channel();
        let (htx, hrx) = std::sync::mpsc::channel();
        App {
            store,
            project,
            sel: 0,
            mode: Mode::Normal,
            focus: Focus::List,
            input: String::new(),
            search: String::new(),
            agents: HashMap::new(),
            scanning: false,
            status: "r rescan · ? help".into(),
            notes_cursor: 0,
            agent_sel: 0,
            show_all_projects: false,
            rx,
            tx,
            herdr_status: HashMap::new(),
            herdr_ok: crate::herdr::available(),
            kind_sel: 0,
            hrx,
            htx,
            quit: false,
        }
    }

    /// Indices into `store.tasks` that pass the project + search filters.
    pub fn visible(&self) -> Vec<usize> {
        let q = self.search.to_lowercase();
        self.store
            .tasks
            .iter()
            .enumerate()
            .filter(|(_, t)| self.show_all_projects || t.project == self.project)
            .filter(|(_, t)| {
                q.is_empty()
                    || t.title.to_lowercase().contains(&q)
                    || t.notes.to_lowercase().contains(&q)
            })
            .map(|(i, _)| i)
            .collect()
    }

    pub fn sel_task(&self) -> Option<usize> {
        self.visible().get(self.sel).copied()
    }

    /// Agent runs credited to the selected task, newest first.
    pub fn runs(&self) -> &[crate::scan::AgentRun] {
        self.sel_task()
            .and_then(|i| self.agents.get(&self.store.tasks[i].id))
            .map(|a| a.runs.as_slice())
            .unwrap_or(&[])
    }

    pub fn sel_run(&self) -> Option<&crate::scan::AgentRun> {
        self.runs().get(self.agent_sel)
    }

    pub fn move_agent_sel(&mut self, delta: isize) {
        let n = self.runs().len();
        if n == 0 {
            self.agent_sel = 0;
            return;
        }
        let cur = self.agent_sel.min(n - 1) as isize;
        self.agent_sel = (cur + delta).rem_euclid(n as isize) as usize;
    }

    pub fn clamp(&mut self) {
        let n = self.visible().len();
        if n == 0 {
            self.sel = 0;
        } else if self.sel >= n {
            self.sel = n - 1;
        }
    }

    /// Earliest window start across all tasks — the scanner's file cutoff.
    pub fn scan_since(&self) -> Option<DateTime<Utc>> {
        self.store
            .tasks
            .iter()
            .flat_map(|t| t.windows.iter().map(|w| w.start))
            .min()
    }

    pub fn start_scan(&mut self) {
        if self.scanning {
            return;
        }
        self.scanning = true;
        let tx = self.tx.clone();
        let since = self.scan_since();
        std::thread::spawn(move || {
            let runs = crate::scan::scan_all(since).unwrap_or_default();
            let _ = tx.send(runs);
        });
    }

    pub fn poll_scan(&mut self) {
        while let Ok(runs) = self.rx.try_recv() {
            self.agents = crate::scan::attribute(&self.store.tasks, &runs);
            self.scanning = false;
            let total: u32 = self.agents.values().map(|a| a.total_agents()).sum();
            self.status = format!("{} attributed · r rescan · ? help", crate::ui::plural(total, "agent"));
        }
    }

    /// Polls herdr for live tab status in the background, forever.
    pub fn start_herdr_watch(&self) {
        if !self.herdr_ok {
            return;
        }
        let tx = self.htx.clone();
        std::thread::spawn(move || loop {
            if tx.send(HerdrMsg::Statuses(crate::herdr::statuses())).is_err() {
                return; // UI gone
            }
            std::thread::sleep(std::time::Duration::from_millis(1500));
        });
    }

    pub fn poll_herdr(&mut self) {
        while let Ok(msg) = self.hrx.try_recv() {
            match msg {
                HerdrMsg::Statuses(s) => {
                    // A tab that vanished from herdr was closed; drop the link
                    // so the next launch creates a fresh one.
                    for t in &mut self.store.tasks {
                        if let Some(h) = &t.herdr {
                            if !s.contains_key(&h.tab_id) && !s.is_empty() {
                                t.herdr = None;
                            }
                        }
                    }
                    self.herdr_status = s;
                }
                HerdrMsg::Launched(id, link) => {
                    if let Some(t) = self.store.tasks.iter_mut().find(|t| t.id == id) {
                        self.status = format!("{} launched in {}", link.kind, link.tab_id);
                        t.herdr = Some(link);
                    }
                    let _ = self.store.save();
                }
                HerdrMsg::Failed(id, e) => {
                    let title = self
                        .store
                        .tasks
                        .iter()
                        .find(|t| t.id == id)
                        .map(|t| t.title.as_str())
                        .unwrap_or("task");
                    self.status = format!("herdr failed for “{}”: {}", title, e);
                }
            }
        }
    }

    /// Focuses the task's tab if it has one, otherwise launches `kind` in a new
    /// tab briefed with the task's title and notes.
    pub fn launch_or_focus(&mut self, kind: &str) {
        if !self.herdr_ok {
            self.status = "herdr not found on PATH".into();
            return;
        }
        let Some(i) = self.sel_task() else { return };
        if let Some(h) = self.store.tasks[i].herdr.clone() {
            match crate::herdr::focus(&h.tab_id) {
                Ok(()) => self.status = format!("focused {}", h.tab_id),
                Err(e) => self.status = format!("herdr: {}", e),
            }
            return;
        }

        // Launching means you're working on it: open the attribution window.
        if self.store.tasks[i].status != Status::Doing {
            self.set_status(Status::Doing);
        }
        let t = &self.store.tasks[i];
        let (id, cwd, label) = (t.id, t.project.clone(), t.title.clone());
        let prompt = if t.notes.trim().is_empty() {
            t.title.clone()
        } else {
            format!("{}\n\n{}", t.title, t.notes)
        };
        let (kind, tx) = (kind.to_string(), self.htx.clone());
        self.status = format!("launching {}…", kind);
        std::thread::spawn(move || {
            let msg = match crate::herdr::launch(&cwd, &label, &kind, &prompt) {
                Ok((tab_id, pane_id, agent_name)) => HerdrMsg::Launched(
                    id,
                    crate::model::HerdrTab {
                        tab_id,
                        pane_id,
                        agent_name,
                        kind,
                    },
                ),
                Err(e) => HerdrMsg::Failed(id, e.to_string()),
            };
            let _ = tx.send(msg);
        });
    }

    /// Closes the task's herdr tab and forgets the link.
    pub fn close_tab(&mut self) {
        let Some(i) = self.sel_task() else { return };
        let Some(h) = self.store.tasks[i].herdr.clone() else {
            self.status = "no agent tab for this task".into();
            return;
        };
        self.status = match crate::herdr::close(&h.tab_id) {
            Ok(()) => format!("closed {}", h.tab_id),
            Err(e) => format!("herdr: {}", e),
        };
        self.store.tasks[i].herdr = None;
        let _ = self.store.save();
    }

    /// Live status of the selected task's tab, if it has one.
    pub fn herdr_state(&self, t: &crate::model::Task) -> Option<&str> {
        t.herdr
            .as_ref()
            .and_then(|h| self.herdr_status.get(&h.tab_id))
            .map(|s| s.as_str())
    }

    pub fn cycle_status(&mut self) {
        if let Some(i) = self.sel_task() {
            let next = self.store.tasks[i].status.next();
            self.store.set_status(i, next);
            let _ = self.store.save();
            self.start_scan();
        }
    }

    pub fn set_status(&mut self, s: Status) {
        if let Some(i) = self.sel_task() {
            self.store.set_status(i, s);
            let _ = self.store.save();
            self.start_scan();
        }
    }

    pub fn delete_selected(&mut self) {
        if let Some(i) = self.sel_task() {
            self.store.tasks.remove(i);
            let _ = self.store.save();
            self.clamp();
        }
    }
}
