use crate::model::{Status, Store};

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum Mode {
    Normal,
    AddTask,
    EditTitle,
    EditNotes,
    ConfirmDelete,
}

pub struct App {
    pub store: Store,
    pub project: String,
    pub sel: usize,
    pub mode: Mode,
    pub input: String,
    pub notes_cursor: usize,
    pub quit: bool,
}

impl App {
    pub fn new(store: Store, project: String) -> App {
        App {
            store,
            project,
            sel: 0,
            mode: Mode::Normal,
            input: String::new(),
            notes_cursor: 0,
            quit: false,
        }
    }

    /// Indices into `store.tasks` that belong to the current project.
    pub fn visible(&self) -> Vec<usize> {
        self.store
            .tasks
            .iter()
            .enumerate()
            .filter(|(_, t)| t.project == self.project)
            .map(|(i, _)| i)
            .collect()
    }

    pub fn sel_task(&self) -> Option<usize> {
        self.visible().get(self.sel).copied()
    }

    pub fn clamp(&mut self) {
        let n = self.visible().len();
        if n == 0 {
            self.sel = 0;
        } else if self.sel >= n {
            self.sel = n - 1;
        }
    }

    pub fn toggle_done(&mut self) {
        if let Some(i) = self.sel_task() {
            let next = if self.store.tasks[i].status == Status::Done {
                Status::Todo
            } else {
                Status::Done
            };
            self.store.set_status(i, next);
            let _ = self.store.save();
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
