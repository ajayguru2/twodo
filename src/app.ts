import { basename, dirname, join, relative } from "node:path";
import { Store } from "./model";

export type Mode = "Normal" | "AddTask" | "EditTitle" | "EditNotes" | "ConfirmDelete";

/** One line of the index: a sub-project to descend into, or a task here. */
export type Row = { kind: "dir"; path: string } | { kind: "task"; idx: number };

const under = (dir: string, path: string) => path === dir || path.startsWith(`${dir}/`);

export class App {
  sel = 0;
  mode: Mode = "Normal";
  input = "";
  notesCursor = 0;
  quit = false;
  project: string;

  constructor(
    public store: Store,
    /** Directory twodo was started in; the tree never walks above it. */
    public root: string,
  ) {
    this.project = root;
  }

  /** Immediate child directories of the current one that hold tasks somewhere below. */
  children(): string[] {
    const out = new Set<string>();
    for (const t of this.store.tasks) {
      if (t.project === this.project || !under(this.project, t.project)) continue;
      const rest = relative(this.project, t.project).split("/")[0]!;
      out.add(join(this.project, rest));
    }
    return [...out].sort();
  }

  rows(): Row[] {
    const dirs: Row[] = this.children().map((path) => ({ kind: "dir", path }));
    const tasks: Row[] = [];
    this.store.tasks.forEach((t, idx) => {
      if (t.project === this.project) tasks.push({ kind: "task", idx });
    });
    return [...dirs, ...tasks];
  }

  /** Open / done counts for everything at or below `dir`. */
  counts(dir: string): { open: number; done: number } {
    let open = 0;
    let done = 0;
    for (const t of this.store.tasks) {
      if (!under(dir, t.project)) continue;
      if (t.status === "Done") done++;
      else open++;
    }
    return { open, done };
  }

  selRow(): Row | undefined {
    return this.rows()[this.sel];
  }

  selTask(): number | undefined {
    const row = this.selRow();
    return row?.kind === "task" ? row.idx : undefined;
  }

  /** Descends into the selected sub-project. Returns false if the row is a task. */
  descend(): boolean {
    const row = this.selRow();
    if (row?.kind !== "dir") return false;
    this.project = row.path;
    this.sel = 0;
    return true;
  }

  /** Walks back up one level, stopping at the directory twodo was started in. */
  ascend(): void {
    if (this.project === this.root) return;
    const child = this.project;
    this.project = dirname(this.project);
    this.sel = Math.max(
      this.rows().findIndex((r) => r.kind === "dir" && r.path === child),
      0,
    );
  }

  /** The current directory shown in the header, relative to the root. */
  label(): string {
    const rel = relative(this.root, this.project);
    const name = basename(this.root) || this.root;
    return rel === "" ? name : `${name}/${rel}`;
  }

  clamp(): void {
    const n = this.rows().length;
    this.sel = n === 0 ? 0 : Math.min(this.sel, n - 1);
  }

  toggleDone(): void {
    const i = this.selTask();
    if (i === undefined) return;
    this.store.setStatus(i, this.store.tasks[i]!.status === "Done" ? "Todo" : "Done");
    this.store.save();
  }

  deleteSelected(): void {
    const i = this.selTask();
    if (i === undefined) return;
    this.store.tasks.splice(i, 1);
    this.store.save();
    this.clamp();
  }
}
