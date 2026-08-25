import { Store } from "./model";

export type Mode = "Normal" | "AddTask" | "EditTitle" | "EditNotes" | "ConfirmDelete";

export class App {
  sel = 0;
  mode: Mode = "Normal";
  input = "";
  notesCursor = 0;
  quit = false;

  constructor(
    public store: Store,
    public project: string,
  ) {}

  /** Indices into `store.tasks` that belong to the current project. */
  visible(): number[] {
    const out: number[] = [];
    this.store.tasks.forEach((t, i) => {
      if (t.project === this.project) out.push(i);
    });
    return out;
  }

  selTask(): number | undefined {
    return this.visible()[this.sel];
  }

  clamp(): void {
    const n = this.visible().length;
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
