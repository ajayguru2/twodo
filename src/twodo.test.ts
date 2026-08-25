import { expect, test } from "bun:test";
import { App } from "./app";
import { parseKeys } from "./keys";
import { Store } from "./model";
import { handle, moveVertical } from "./main";
import { plain } from "./ui";

function storeWithWindows(): Store {
  const now = Date.now();
  const iso = (minutesAgo: number) => new Date(now - minutesAgo * 60_000).toISOString();
  const s = new Store();
  s.add("older", "/proj");
  s.add("newer", "/proj");
  s.add("other project", "/elsewhere");
  s.tasks[0]!.windows.push({ start: iso(60), end: null });
  s.tasks[1]!.windows.push({ start: iso(30), end: null });
  s.tasks[2]!.windows.push({ start: iso(60), end: null });
  return s;
}

test("doing opens a window and leaving doing closes it", () => {
  const s = new Store();
  s.add("t", "/proj");
  s.setStatus(0, "Doing");
  expect(s.tasks[0]!.windows.length).toBe(1);
  s.setStatus(0, "Doing"); // already doing: no second window
  expect(s.tasks[0]!.windows.length).toBe(1);
  s.setStatus(0, "Done");
  expect(s.tasks[0]!.windows[0]!.end).not.toBeNull();
  const end = s.tasks[0]!.windows[0]!.end;
  s.setStatus(0, "Todo");
  expect(s.tasks[0]!.windows[0]!.end).toBe(end); // closed time is fixed
});

test("a window left open by a crash does not accrue time", () => {
  const s = storeWithWindows();
  s.tasks[0]!.status = "Todo"; // open window, not Doing
  s.closeStaleWindows();
  expect(s.tasks[0]!.windows[0]!.end).toBe(s.tasks[0]!.windows[0]!.start);
});

test("the store round-trips through disk", () => {
  const path = `${import.meta.dir}/../.test-store.json`;
  const s = storeWithWindows();
  s.tasks[0]!.notes = "note\nlines";
  s.save(path);
  const back = Store.load(path);
  expect(back.next_id).toBe(s.next_id);
  expect(back.tasks).toEqual(s.tasks);
  Bun.file(path).delete();
});

test("list opens the selected task's note", () => {
  const app = new App(storeWithWindows(), "/proj");
  app.store.tasks[0]!.notes = "Trace the parser before changing it.";

  const list = plain(app, 100, 30);
  expect(list).toContain("older");
  expect(list).toContain("newer");
  expect(list).not.toContain("other project"); // other projects are filtered out

  handle(app, { type: "enter" });
  expect(app.mode).toBe("EditNotes");
  const note = plain(app, 100, 30);
  expect(note).toContain("older");
  expect(note).toContain("Trace the parser before changing it.");
});

test("vertical note movement uses character columns", () => {
  const note = "aéx\n1234";
  const firstLine = "aé".length;
  const secondLine = "aéx\n12".length;
  expect(moveVertical(note, firstLine, true)).toBe(secondLine);
  expect(moveVertical(note, secondLine, false)).toBe(firstLine);
});

test("a compact workbench keeps the task and note controls visible", () => {
  const project = "/work/twodo";
  const store = new Store();
  store.add("Fix flaky parser", project);
  const app = new App(store, project);

  const index = plain(app, 60, 16);
  expect(index).toContain("Fix flaky parser");
  for (const hint of ["enter", "space", "quit"]) expect(index).toContain(hint);

  app.store.tasks[0]!.notes = Array.from({ length: 20 }, (_, i) => `note line ${i + 1}`).join("\n");
  app.notesCursor = app.store.tasks[0]!.notes.length;
  app.mode = "EditNotes";
  const note = plain(app, 60, 16);
  expect(note).toContain("note line 20");
  expect(note).toContain("save & back");
});

test("every rendered row is exactly the terminal width", () => {
  const store = new Store();
  store.add("Fix flaky parser", "/proj");
  const app = new App(store, "/proj");
  app.mode = "ConfirmDelete";
  const rows = plain(app, 64, 20).split("\n");
  expect(rows.length).toBe(20);
  expect(new Set(rows.map((r) => [...r].length))).toEqual(new Set([64]));
  expect(rows.join("\n")).toContain("Delete this task?");
});

test("raw input is decoded into key events", () => {
  expect(parseKeys("a\x1b[Bé\r\x1b\x1b[3~\x03")).toEqual([
    { type: "char", value: "a" },
    { type: "down" },
    { type: "char", value: "é" },
    { type: "enter" },
    { type: "esc" },
    { type: "delete" },
    { type: "ctrl-c" },
  ]);
});
