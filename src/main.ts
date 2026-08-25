#!/usr/bin/env bun
import { realpathSync } from "node:fs";
import { resolve } from "node:path";
import { App, type Mode } from "./app";
import { parseKeys, type Key } from "./keys";
import { Store } from "./model";
import { render, setTheme } from "./ui";

// ------------------------------------------------------------- note edits ---

export function lineStart(text: string, cursor: number): number {
  return text.slice(0, cursor).lastIndexOf("\n") + 1;
}

export function lineEnd(text: string, cursor: number): number {
  const i = text.indexOf("\n", cursor);
  return i === -1 ? text.length : i;
}

export function moveVertical(text: string, cursor: number, down: boolean): number {
  const start = lineStart(text, cursor);
  const column = [...text.slice(start, cursor)].length;
  let targetStart: number;
  let targetEnd: number;
  if (down) {
    const end = lineEnd(text, cursor);
    if (end === text.length) return cursor;
    targetStart = end + 1;
    targetEnd = lineEnd(text, targetStart);
  } else {
    if (start === 0) return cursor;
    targetEnd = start - 1;
    targetStart = lineStart(text, targetEnd);
  }
  const line = [...text.slice(targetStart, targetEnd)];
  return targetStart + line.slice(0, column).join("").length;
}

/** Byte-safe step to the previous / next code point. */
const prevChar = (text: string, cursor: number) =>
  cursor - ([...text.slice(0, cursor)].at(-1)?.length ?? 0);
const nextChar = (text: string, cursor: number) =>
  cursor + ([...text.slice(cursor)][0]?.length ?? 0);

/** VSCode word-delete boundary: skip trailing whitespace, then the word. */
export const wordStart = (text: string, cursor: number) =>
  text.slice(0, cursor).replace(/\S*\s*$/, "").length;

// ------------------------------------------------------------------ keys ----

export function handle(app: App, k: Key): void {
  switch (app.mode) {
    case "Normal":
      return normal(app, k);
    case "ConfirmDelete":
      if (k.type === "char" && k.value === "d") app.deleteSelected();
      app.mode = "Normal";
      return;
    case "AddTask":
    case "EditTitle":
      return textPrompt(app, k);
    case "EditNotes":
      return notesEdit(app, k);
  }
}

function normal(app: App, k: Key): void {
  const n = app.rows().length;
  const char = k.type === "char" ? k.value : "";
  if (char === "q") app.quit = true;
  else if (char === "j" || k.type === "down") {
    if (n > 0) app.sel = (app.sel + 1) % n;
  } else if (char === "k" || k.type === "up") {
    if (n > 0) app.sel = (app.sel + n - 1) % n;
  } else if (char === "g") app.sel = 0;
  else if (char === "G") app.sel = Math.max(n - 1, 0);
  else if (char === " ") app.toggleDone();
  else if (char === "a" || char === "n") {
    app.mode = "AddTask";
    app.input = "";
  } else if (char === "e") {
    const i = app.selTask();
    if (i !== undefined) {
      app.input = app.store.tasks[i]!.title;
      app.mode = "EditTitle";
    }
  } else if (char === "l" || k.type === "right") {
    app.descend();
  } else if (char === "h" || k.type === "left" || k.type === "esc") {
    app.ascend();
  } else if (char === "i" || k.type === "enter") {
    if (app.descend()) return;
    const i = app.selTask();
    if (i !== undefined) {
      app.notesCursor = app.store.tasks[i]!.notes.length;
      app.mode = "EditNotes";
    }
  } else if (char === "d" && app.selTask() !== undefined) {
    app.mode = "ConfirmDelete";
  }
}

function textPrompt(app: App, k: Key): void {
  if (k.type === "esc") {
    app.mode = "Normal";
  } else if (k.type === "enter") {
    const text = app.input.trim();
    if (text !== "") {
      if (app.mode === "AddTask") {
        app.store.add(text, app.project);
        app.sel = Math.max(app.rows().length - 1, 0);
      } else {
        const i = app.selTask();
        if (i !== undefined) app.store.tasks[i]!.title = text;
      }
      app.store.save();
    }
    app.input = "";
    app.mode = "Normal";
  } else if (k.type === "backspace") {
    app.input = app.input.slice(0, prevChar(app.input, app.input.length));
  } else if (k.type === "kill-line") {
    app.input = "";
  } else if (k.type === "kill-word") {
    app.input = app.input.slice(0, wordStart(app.input, app.input.length));
  } else if (k.type === "char") {
    app.input += k.value;
  }
}

function notesEdit(app: App, k: Key): void {
  const i = app.selTask();
  if (i === undefined) {
    app.mode = "Normal";
    return;
  }
  const task = app.store.tasks[i]!;
  const n = task.notes;
  const cur = Math.min(app.notesCursor, n.length);
  const insert = (text: string) => {
    task.notes = n.slice(0, cur) + text + n.slice(cur);
    app.notesCursor = cur + text.length;
  };

  switch (k.type) {
    case "esc":
      app.store.save();
      app.mode = "Normal";
      return;
    case "char":
      return insert(k.value);
    case "enter":
      return insert("\n");
    case "tab":
      return insert("  ");
    case "backspace": {
      if (cur === 0) return;
      const prev = prevChar(n, cur);
      task.notes = n.slice(0, prev) + n.slice(cur);
      app.notesCursor = prev;
      return;
    }
    case "delete":
      if (cur < n.length) task.notes = n.slice(0, cur) + n.slice(nextChar(n, cur));
      return;
    case "kill-line": {
      const start = lineStart(n, cur);
      task.notes = n.slice(0, start) + n.slice(cur);
      app.notesCursor = start;
      return;
    }
    case "kill-word": {
      const start = wordStart(n, cur);
      task.notes = n.slice(0, start) + n.slice(cur);
      app.notesCursor = start;
      return;
    }
    case "left":
      app.notesCursor = prevChar(n, cur);
      return;
    case "right":
      app.notesCursor = nextChar(n, cur);
      return;
    case "up":
      app.notesCursor = moveVertical(n, cur, false);
      return;
    case "down":
      app.notesCursor = moveVertical(n, cur, true);
      return;
    case "home":
      app.notesCursor = lineStart(n, cur);
      return;
    case "end":
      app.notesCursor = lineEnd(n, cur);
      return;
  }
}

// ------------------------------------------------------------------ main ----

/**
 * Asks the terminal for its background colour (OSC 11) and reports whether it
 * is light. Falls back to dark if the terminal stays quiet.
 * ponytail: read once at start-up; terminals do not announce theme changes.
 */
function detectLightBackground(): Promise<boolean> {
  return new Promise((done) => {
    let buf = "";
    const finish = (light: boolean) => {
      clearTimeout(timer);
      process.stdin.off("data", onData);
      done(light);
    };
    const onData = (chunk: string) => {
      buf += chunk;
      const m = /\x1b\]11;rgb:([0-9a-f]+)\/([0-9a-f]+)\/([0-9a-f]+)/i.exec(buf);
      if (!m) return;
      const [r, g, b] = m.slice(1, 4).map((h) => parseInt(h.slice(0, 2), 16));
      finish(0.2126 * r! + 0.7152 * g! + 0.0722 * b! > 128);
    };
    const timer = setTimeout(() => finish(false), 120);
    process.stdin.on("data", onData);
    process.stdout.write("\x1b]11;?\x07");
  });
}

async function main(): Promise<void> {
  const store = Store.load();
  store.closeStaleWindows();

  const arg = process.argv[2] ?? process.cwd();
  const project = ((p: string) => {
    try {
      return realpathSync(p);
    } catch {
      return p;
    }
  })(resolve(arg));

  if (!process.stdin.isTTY || !process.stdout.isTTY) {
    console.error("twodo needs a terminal.");
    process.exit(1);
  }

  const app = new App(store, project);
  const out = process.stdout;

  const draw = () => {
    const lines = render(app, out.columns || 80, out.rows || 24);
    out.write(`\x1b[H${lines.join("\r\n")}`);
  };

  const leave = () => {
    process.stdin.setRawMode?.(false);
    out.write("\x1b[?25h\x1b[?1049l"); // show cursor, leave alternate screen
  };

  process.stdin.setRawMode(true);
  process.stdin.resume();
  process.stdin.setEncoding("utf8");
  setTheme(await detectLightBackground());

  out.write("\x1b[?1049h\x1b[?25l\x1b[2J"); // alternate screen, hide cursor, clear
  draw();

  const quit = () => {
    leave();
    app.store.save();
    process.exit(0);
  };

  process.stdin.on("data", (chunk: string) => {
    for (const k of parseKeys(chunk)) {
      if (k.type === "ctrl-c") return quit();
      handle(app, k);
      if (app.quit) return quit();
    }
    draw();
  });
  out.on("resize", draw);
  process.on("SIGTERM", quit);
}

if (import.meta.main) main();
