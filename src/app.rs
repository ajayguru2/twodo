use crate::model::{Status, Store};

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum Focus {
    List,
    Notes,
}

impl Focus {
    pub fn next(self) -> Focus {
        match self {
            Focus::List => Focus::Notes,
            Focus::Notes => Focus::List,
        }
    }
    pub fn prev(self) -> Focus {
        self.next()
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
}

pub struct App {
    pub store: Store,
    pub project: String,
    pub sel: usize,
    pub mode: Mode,
    pub focus: Focus,
    pub input: String,
    pub search: String,
    pub status: String,
    pub notes_cursor: usize,
    pub show_all_projects: bool,
    pub quit: bool,
}

impl App {
    pub fn new(store: Store, project: String) -> App {
        App {
            store,
            project,
            sel: 0,
            mode: Mode::Normal,
            focus: Focus::List,
            input: String::new(),
            search: String::new(),
            status: "? help".into(),
            notes_cursor: 0,
            show_all_projects: false,
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

    pub fn clamp(&mut self) {
        let n = self.visible().len();
        if n == 0 {
            self.sel = 0;
        } else if self.sel >= n {
            self.sel = n - 1;
        }
    }

    pub fn cycle_status(&mut self) {
        if let Some(i) = self.sel_task() {
            let next = self.store.tasks[i].status.next();
            self.store.set_status(i, next);
            let _ = self.store.save();
        }
    }

    pub fn set_status(&mut self, s: Status) {
        if let Some(i) = self.sel_task() {
            self.store.set_status(i, s);
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
