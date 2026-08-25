use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    Todo,
    Doing,
    Done,
}

impl Status {
    pub fn glyph(&self) -> &'static str {
        match self {
            Status::Todo => "○",
            Status::Doing => "◐",
            Status::Done => "●",
        }
    }
    pub fn next(&self) -> Status {
        match self {
            Status::Todo => Status::Doing,
            Status::Doing => Status::Done,
            Status::Done => Status::Todo,
        }
    }
}

/// A span during which a task was actively being worked on.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Window {
    pub start: DateTime<Utc>,
    pub end: Option<DateTime<Utc>>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Task {
    pub id: u64,
    pub title: String,
    pub status: Status,
    /// Absolute path of the directory this task's work happens in.
    pub project: String,
    pub created: DateTime<Utc>,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub windows: Vec<Window>,
}

impl Task {
    /// Total wall time spent in Doing, including a currently open window.
    pub fn active_secs(&self, now: DateTime<Utc>) -> i64 {
        self.windows
            .iter()
            .map(|w| (w.end.unwrap_or(now) - w.start).num_seconds().max(0))
            .sum()
    }

}

#[derive(Serialize, Deserialize, Default)]
pub struct Store {
    pub next_id: u64,
    pub tasks: Vec<Task>,
}

pub fn data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("twodo")
}

pub fn store_path() -> PathBuf {
    data_dir().join("store.json")
}

impl Store {
    pub fn load() -> Result<Store> {
        let p = store_path();
        if !p.exists() {
            return Ok(Store::default());
        }
        Ok(serde_json::from_slice(&std::fs::read(p)?)?)
    }

    pub fn save(&self) -> Result<()> {
        std::fs::create_dir_all(data_dir())?;
        let tmp = store_path().with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(self)?)?;
        std::fs::rename(tmp, store_path())?;
        Ok(())
    }

    pub fn add(&mut self, title: String, project: String) -> u64 {
        self.next_id += 1;
        let id = self.next_id;
        self.tasks.push(Task {
            id,
            title,
            status: Status::Todo,
            project,
            created: Utc::now(),
            notes: String::new(),
            windows: Vec::new(),
        });
        id
    }

    /// Moves a task to `status`, opening or closing its active-time window.
    pub fn set_status(&mut self, idx: usize, status: Status) {
        let now = Utc::now();
        let Some(t) = self.tasks.get_mut(idx) else {
            return;
        };
        let was_doing = t.status == Status::Doing;
        t.status = status;
        match (was_doing, status == Status::Doing) {
            (false, true) => t.windows.push(Window {
                start: now,
                end: None,
            }),
            (true, false) => {
                if let Some(w) = t.windows.last_mut() {
                    if w.end.is_none() {
                        w.end = Some(now);
                    }
                }
            }
            _ => {}
        }
    }

    /// Closes any window left open by a previous run that outlived the process.
    pub fn close_stale_windows(&mut self) {
        for t in &mut self.tasks {
            if t.status != Status::Doing {
                if let Some(w) = t.windows.last_mut() {
                    if w.end.is_none() {
                        w.end = Some(w.start);
                    }
                }
            }
        }
    }
}
