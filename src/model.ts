import { mkdirSync, renameSync, existsSync, readFileSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

export type Status = "Todo" | "Doing" | "Done";

/** A span during which a task was actively being worked on. */
export interface Window {
  start: string;
  end: string | null;
}

export interface Task {
  id: number;
  title: string;
  status: Status;
  /** Absolute path of the directory this task's work happens in. */
  project: string;
  created: string;
  notes: string;
  windows: Window[];
}

export function dataDir(): string {
  if (process.platform === "darwin") {
    return join(homedir(), "Library", "Application Support", "twodo");
  }
  return join(process.env.XDG_DATA_HOME || join(homedir(), ".local", "share"), "twodo");
}

export function storePath(): string {
  return join(dataDir(), "store.json");
}

export class Store {
  next_id = 0;
  tasks: Task[] = [];

  static load(path = storePath()): Store {
    const store = new Store();
    if (!existsSync(path)) return store;
    const raw = JSON.parse(readFileSync(path, "utf8")) as Partial<Store>;
    store.next_id = raw.next_id ?? 0;
    store.tasks = (raw.tasks ?? []).map((t) => ({ ...t, notes: t.notes ?? "", windows: t.windows ?? [] }));
    return store;
  }

  save(path = storePath()): void {
    mkdirSync(join(path, ".."), { recursive: true });
    const tmp = `${path}.tmp`;
    writeFileSync(tmp, JSON.stringify({ next_id: this.next_id, tasks: this.tasks }, null, 2));
    renameSync(tmp, path);
  }

  add(title: string, project: string): number {
    const id = ++this.next_id;
    this.tasks.push({
      id,
      title,
      status: "Todo",
      project,
      created: new Date().toISOString(),
      notes: "",
      windows: [],
    });
    return id;
  }

  /** Moves a task to `status`, opening or closing its active-time window. */
  setStatus(idx: number, status: Status): void {
    const now = new Date().toISOString();
    const t = this.tasks[idx];
    if (!t) return;
    const wasDoing = t.status === "Doing";
    t.status = status;
    if (!wasDoing && status === "Doing") {
      t.windows.push({ start: now, end: null });
    } else if (wasDoing && status !== "Doing") {
      const w = t.windows.at(-1);
      if (w && w.end === null) w.end = now;
    }
  }

  /** Closes any window left open by a previous run that outlived the process. */
  closeStaleWindows(): void {
    for (const t of this.tasks) {
      if (t.status === "Doing") continue;
      const w = t.windows.at(-1);
      if (w && w.end === null) w.end = w.start;
    }
  }
}
