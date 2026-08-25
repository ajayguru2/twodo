import { expect, test } from "bun:test";
import { App } from "./app";
import { parseKeys } from "./keys";
import { Store } from "./model";
import { handle, moveVertical } from "./main";
import { render, setTheme, plain } from "./ui";

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

test("cmd+backspace clears the line and option+backspace clears a word", () => {
  expect(parseKeys("\x15\x1b\x7f\x17")).toEqual([
    { type: "kill-line" },
    { type: "kill-word" },
    { type: "kill-word" },
  ]);

  const s = new Store();
  s.add("t", "/proj");
  const app = new App(s, "/proj");
  app.mode = "EditTitle";
  app.sel = 0;
  app.input = "fix the parser ";
  handle(app, { type: "kill-word" });
  expect(app.input).toBe("fix the ");

  s.tasks[0]!.notes = "line one\nsecond line";
  app.mode = "EditNotes";
  app.notesCursor = s.tasks[0]!.notes.length;
  handle(app, { type: "kill-line" });
  expect(s.tasks[0]!.notes).toBe("line one\n");
  handle(app, { type: "kill-word" });
  expect(s.tasks[0]!.notes).toBe("line ");
  expect(app.notesCursor).toBe(5);
});

test("the light theme repaints on a paper background", () => {
  const store = new Store();
  store.add("Fix flaky parser", "/proj");
  const app = new App(store, "/proj");
  const dark = render(app, 64, 20).join("");
  setTheme(true);
  const light = render(app, 64, 20).join("");
  setTheme(false);
  expect(light).toContain("48;2;253;246;227"); // solarized base3
  expect(dark).toContain("48;2;10;12;19");
  expect(plain(app, 64, 20)).toContain("Fix flaky parser");
});

test("a parent directory lists sub-projects and descends into them", () => {
  const store = new Store();
  store.add("root chore", "/work");
  store.add("parser bug", "/work/twodo/src");
  store.add("other", "/elsewhere");
  const app = new App(store, "/work");

  expect(app.rows()).toEqual([
    { kind: "dir", path: "/work/twodo" },
    { kind: "task", idx: 0 },
  ]);
  const top = plain(app, 80, 20);
  expect(top).toContain("twodo/");
  expect(top).toContain("root chore");
  expect(top).not.toContain("parser bug"); // nested: shown after descending
  expect(top).not.toContain("other");

  handle(app, { type: "enter" }); // into /work/twodo
  expect(app.project).toBe("/work/twodo");
  handle(app, { type: "enter" }); // into /work/twodo/src
  expect(app.project).toBe("/work/twodo/src");
  expect(plain(app, 80, 20)).toContain("parser bug");

  handle(app, { type: "esc" });
  handle(app, { type: "esc" });
  expect(app.project).toBe("/work");
  expect(app.sel).toBe(0); // back on the sub-project we came from
  handle(app, { type: "esc" }); // already at the root: stays put
  expect(app.project).toBe("/work");
});

test("a project folder shows only its own tasks", () => {
  const store = new Store();
  store.add("parser bug", "/work/twodo");
  store.add("root chore", "/work");
  const app = new App(store, "/work/twodo");
  expect(app.rows()).toEqual([{ kind: "task", idx: 0 }]);
  handle(app, { type: "enter" });
  expect(app.mode).toBe("EditNotes"); // enter on a task still opens the note
});
