import type { App } from "./app";
import { basename } from "node:path";

type RGB = readonly [number, number, number];

const INK: RGB = [10, 12, 19];
const SURFACE: RGB = [18, 21, 32];
const TEXT: RGB = [232, 234, 242];
const MUTED: RGB = [126, 139, 164];
const VIOLET: RGB = [174, 143, 255];
const MINT: RGB = [115, 218, 176];
const CORAL: RGB = [255, 123, 125];
const SELECTED: RGB = [35, 31, 53];

interface Style {
  fg?: RGB;
  bg?: RGB;
  bold?: boolean;
  strike?: boolean;
}
interface Seg {
  text: string;
  style?: Style;
}
type Line = Seg[];

// ponytail: columns are counted in code points, so wide CJK and emoji titles
// misalign by a cell. Swap in a width table if that ever shows up.
const cols = (s: string) => [...s].length;
const chars = (s: string) => [...s];

function ansi(style: Style = {}): string {
  const parts: string[] = ["0"];
  if (style.bold) parts.push("1");
  if (style.strike) parts.push("9");
  const out = [`\x1b[${parts.join(";")}m`];
  if (style.fg) out.push(`\x1b[38;2;${style.fg.join(";")}m`);
  if (style.bg) out.push(`\x1b[48;2;${style.bg.join(";")}m`);
  return out.join("");
}

const seg = (text: string, style?: Style): Seg => ({ text, style });
const key = (text: string) => seg(text, { fg: VIOLET, bold: true });
const keyDanger = (text: string) => seg(text, { fg: CORAL, bold: true });
const label = (text: string) => seg(text, { fg: MUTED });

/** Visible columns [start, end) of a line, keeping styles. */
function sliceLine(line: Line, start: number, end: number): Line {
  const out: Line = [];
  let col = 0;
  for (const s of line) {
    const cs = chars(s.text);
    const from = Math.max(start - col, 0);
    const to = Math.min(end - col, cs.length);
    if (to > from) out.push(seg(cs.slice(from, to).join(""), s.style));
    col += cs.length;
    if (col >= end) break;
  }
  return out;
}

const lineWidth = (line: Line) => line.reduce((n, s) => n + cols(s.text), 0);

function pad(line: Line, width: number, style: Style): Line {
  const gap = width - lineWidth(line);
  return gap > 0 ? [...line, seg(" ".repeat(gap), style)] : sliceLine(line, 0, width);
}

/** Draws `patch` over `line` starting at visible column `x`. */
function overlay(line: Line, x: number, patch: Line): Line {
  const w = lineWidth(patch);
  return [...sliceLine(line, 0, x), ...patch, ...sliceLine(line, x + w, Infinity)];
}

const toAnsi = (line: Line) =>
  line.map((s) => ansi(s.style) + s.text).join("") + "\x1b[0m";

function box(width: number, style: Style): Line {
  return [seg(" ".repeat(Math.max(width, 0)), style)];
}

function border(width: number, ch: string, style: Style): Line {
  return [seg(ch.repeat(Math.max(width, 0)), { bg: INK, ...style })];
}

// ---------------------------------------------------------------- screen ----

export function render(app: App, width: number, height: number): string[] {
  const margin = height > 12 ? 1 : 0;
  const w = Math.min(width, 86);
  const x = Math.floor(Math.max(width - w, 0) / 2);
  const h = Math.max(height - margin * 2, 1);

  const content =
    app.mode === "EditNotes" ? drawNote(app, w, h) : drawIndex(app, w, h);

  const blank = () => box(width, { bg: INK });
  const screen: Line[] = [];
  for (let i = 0; i < height; i++) screen.push(blank());
  content.forEach((line, i) => {
    const row = margin + i;
    if (row < height) screen[row] = overlay(screen[row]!, x, pad(line, w, { bg: INK }));
  });

  const area = { x, y: margin, w, h };
  if (app.mode === "AddTask") textPrompt(screen, area, "new task", app.input);
  if (app.mode === "EditTitle") textPrompt(screen, area, "rename task", app.input);
  if (app.mode === "ConfirmDelete") deletePrompt(screen, area, app);

  return screen.map((line) => toAnsi(pad(line, width, { bg: INK })));
}

// ----------------------------------------------------------------- index ----

function drawIndex(app: App, w: number, h: number): Line[] {
  const visible = app.visible();
  const done = visible.filter((i) => app.store.tasks[i]!.status === "Done").length;
  const open = visible.length - done;
  const project = basename(app.project) || app.project;
  const footerH = w >= 72 ? 2 : 3;
  const bodyH = Math.max(h - 3 - footerH, 0);

  const lines: Line[] = [
    [seg("twodo", { fg: VIOLET, bold: true }), seg(`  /  ${project}`, { fg: MUTED })],
    [
      seg(`${open} open`, { fg: TEXT }),
      seg("  ·  ", { fg: MUTED }),
      seg(`${done} done`, { fg: MINT }),
    ],
    border(w, "─", { fg: SURFACE }),
  ];

  if (visible.length === 0) {
    lines.push(
      [],
      [seg("▌ ", { fg: VIOLET }), seg("No tasks yet.", { fg: TEXT })],
      [seg("  Press ", { fg: MUTED }), key("n"), seg(" to add one.", { fg: MUTED })],
    );
  } else {
    const offset = Math.max(0, Math.min(app.sel - bodyH + 1, visible.length - bodyH));
    for (const row of visible.keys()) {
      if (row < offset || row >= offset + bodyH) continue;
      const task = app.store.tasks[visible[row]!]!;
      const selected = row === app.sel;
      const [mark, markColor] =
        task.status === "Done" ? ["✓", MINT] : ["○", MUTED];
      const titleStyle: Style =
        task.status === "Done" ? { fg: MUTED, strike: true } : { fg: TEXT };
      const fill: Style = selected ? { bg: SELECTED, bold: true } : { bg: INK };
      lines.push(
        pad(
          [
            seg(selected ? "▌ " : "  ", { fg: VIOLET, ...fill }),
            seg(`${String(row + 1).padStart(3)}  `, { fg: MUTED, ...fill }),
            seg(`${mark}  `, { fg: markColor as RGB, ...fill }),
            seg(task.title, { ...titleStyle, ...fill }),
          ],
          w,
          fill,
        ),
      );
    }
  }

  while (lines.length < 3 + bodyH) lines.push([]);
  return [...lines.slice(0, 3 + bodyH), ...indexFooter(w, footerH)];
}

function indexFooter(w: number, footerH: number): Line[] {
  const rows: Line[] =
    w >= 72
      ? [
          [
            key("↑↓"), label(" move   "),
            key("enter"), label(" note   "),
            key("n"), label(" add   "),
            key("e"), label(" rename   "),
            key("space"), label(" done   "),
            key("d"), label(" delete   "),
            key("q"), label(" quit"),
          ],
        ]
      : [
          [
            key("↑↓"), label(" move   "),
            key("enter"), label(" note   "),
            key("n"), label(" add   "),
            key("space"), label(" done"),
          ],
          [
            key("e"), label(" rename   "),
            key("d"), label(" delete   "),
            key("q"), label(" quit"),
          ],
        ];
  return [border(w, "─", { fg: SURFACE }), ...rows].slice(0, footerH);
}

// ------------------------------------------------------------------ note ----

function drawNote(app: App, w: number, h: number): Line[] {
  const i = app.selTask();
  if (i === undefined) return [];
  const task = app.store.tasks[i]!;
  const footerH = w >= 48 ? 2 : 3;
  const bodyH = Math.max(h - 3 - footerH, 1);
  const cursor = Math.max(0, Math.min(app.notesCursor, task.notes.length));

  const lines: Line[] = [
    [seg("twodo", { fg: VIOLET, bold: true }), seg("  note", { fg: MUTED })],
    [seg(task.title, { fg: TEXT, bold: true })],
    border(w, "─", { fg: SURFACE }),
  ];

  const body = noteBody(task.notes, cursor, w - 1, bodyH);
  for (let row = 0; row < bodyH; row++) {
    lines.push([seg("│", { fg: VIOLET }), ...(body[row] ?? [])]);
  }

  const footer: Line =
    w >= 48
      ? [key("esc"), label(" save & back   "), key("tab"), label(" indent")]
      : [key("esc"), label(" save   "), key("tab"), label(" indent")];
  return [...lines, border(w, "─", { fg: SURFACE }), footer].slice(0, h);
}

/** Wraps the note, places the cursor bar, and scrolls to keep it in view. */
function noteBody(note: string, cursor: number, width: number, viewport: number): Line[] {
  if (note === "") {
    return [
      [
        seg("  "),
        seg("▏", { fg: VIOLET, bold: true }),
        seg(" Write anything.", { fg: MUTED }),
      ],
    ];
  }

  const before = note.slice(0, cursor);
  const cursorRow = before.split("\n").length - 1;
  const cursorCol = 2 + cols(before.slice(before.lastIndexOf("\n") + 1));

  const rows: Line[] = [];
  let cursorAt = 0;
  note.split("\n").forEach((raw, row) => {
    const text = chars(`  ${raw}`);
    const step = Math.max(width, 1);
    for (let start = 0; start < Math.max(text.length, 1); start += step) {
      const chunk = text.slice(start, start + step);
      if (row === cursorRow && cursorCol >= start && cursorCol <= start + step) {
        cursorAt = rows.length;
        const at = cursorCol - start;
        rows.push([
          seg(chunk.slice(0, at).join(""), { fg: TEXT }),
          seg("▏", { fg: VIOLET, bold: true }),
          seg(chunk.slice(at).join(""), { fg: TEXT }),
        ]);
      } else {
        rows.push([seg(chunk.join(""), { fg: TEXT })]);
      }
    }
  });

  const scroll = Math.max(0, cursorAt - viewport + 1);
  return rows.slice(scroll);
}

// --------------------------------------------------------------- prompts ----

interface Area {
  x: number;
  y: number;
  w: number;
  h: number;
}

function centered(area: Area, maxWidth: number, height: number): Area {
  const w = Math.min(maxWidth, Math.max(area.w - 2, 1));
  const h = Math.min(height, Math.max(area.h - 2, 1));
  return {
    x: area.x + Math.floor(Math.max(area.w - w, 0) / 2),
    y: area.y + Math.floor(Math.max(area.h - h, 0) / 2),
    w,
    h,
  };
}

/** A bordered modal: top title bar, body lines, bottom hint bar. */
function drawBox(
  screen: Line[],
  at: Area,
  frame: RGB,
  title: Line,
  body: Line[],
  hint: Line,
): void {
  const style: Style = { bg: SURFACE };
  const edge: Style = { fg: frame, bg: SURFACE };
  const inner = at.w - 2;

  /** Title bar: rule with `text` laid over it, left-aligned or centred. */
  const bar = (left: string, right: string, text: Line, centre: boolean): Line => {
    const rule: Line = [seg("─".repeat(Math.max(inner, 0)), edge)];
    const start = centre ? Math.floor(Math.max(inner - lineWidth(text), 0) / 2) : 0;
    return [seg(left, edge), ...overlay(rule, start, sliceLine(text, 0, inner)), seg(right, edge)];
  };

  const rows: Line[] = [bar("┌", "┐", title, false)];
  for (let i = 0; i < at.h - 2; i++) {
    rows.push([seg("│", edge), ...pad(body[i] ?? [], inner, style), seg("│", edge)]);
  }
  rows.push(bar("└", "┘", hint, true));

  rows.forEach((row, i) => {
    const y = at.y + i;
    if (y < screen.length) screen[y] = overlay(screen[y]!, at.x, pad(row, at.w, style));
  });
}

const onSurface = (line: Line): Line =>
  line.map((s) => seg(s.text, { ...s.style, bg: SURFACE }));

function textPrompt(screen: Line[], area: Area, title: string, input: string): void {
  const at = centered(area, 56, 5);
  drawBox(
    screen,
    at,
    VIOLET,
    [seg(` ${title} `, { fg: TEXT, bold: true, bg: SURFACE })],
    [[seg(input, { fg: TEXT, bg: SURFACE }), seg("▏", { fg: VIOLET, bold: true, bg: SURFACE })]],
    onSurface([key("enter"), label(" save  "), key("esc"), label(" cancel")]),
  );
}

function deletePrompt(screen: Line[], area: Area, app: App): void {
  const at = centered(area, 56, 6);
  const i = app.selTask();
  const title = i === undefined ? "this task" : app.store.tasks[i]!.title;
  drawBox(
    screen,
    at,
    CORAL,
    [seg(" delete ", { fg: CORAL, bold: true, bg: SURFACE })],
    [
      [seg("Delete this task?", { fg: TEXT, bold: true, bg: SURFACE })],
      [seg(title, { fg: MUTED, bg: SURFACE })],
    ],
    onSurface([keyDanger("d"), label(" confirm  "), key("esc"), label(" cancel")]),
  );
}

/** The rendered screen as plain text, for tests. */
export const plain = (app: App, width: number, height: number) =>
  render(app, width, height)
    .map((l) => l.replace(/\x1b\[[0-9;]*m/g, ""))
    .join("\n");
