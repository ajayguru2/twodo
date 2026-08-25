export type Key =
  | { type: "char"; value: string }
  | { type: "enter" | "esc" | "backspace" | "tab" | "delete" | "home" | "end" }
  | { type: "kill-line" | "kill-word" }
  | { type: "up" | "down" | "left" | "right" }
  | { type: "ctrl-c" };

const ESCAPES: Record<string, Key["type"]> = {
  "[A": "up",
  "[B": "down",
  "[C": "right",
  "[D": "left",
  "[H": "home",
  "[F": "end",
  "[1~": "home",
  "[7~": "home",
  "[4~": "end",
  "[8~": "end",
  "[3~": "delete",
  "\x7f": "kill-word", // option+backspace
  "\b": "kill-word",
  OA: "up",
  OB: "down",
  OC: "right",
  OD: "left",
  OH: "home",
  OF: "end",
};

/** Splits one raw stdin chunk into key events. */
export function parseKeys(data: string): Key[] {
  const keys: Key[] = [];
  let i = 0;
  while (i < data.length) {
    const c = data[i]!;
    if (c === "\x03") {
      keys.push({ type: "ctrl-c" });
      i += 1;
    } else if (c === "\x1b") {
      const rest = data.slice(i + 1);
      const match = Object.keys(ESCAPES).find((seq) => rest.startsWith(seq));
      if (match) {
        keys.push({ type: ESCAPES[match]! } as Key);
        i += 1 + match.length;
      } else {
        keys.push({ type: "esc" });
        i += 1;
      }
    } else if (c === "\r" || c === "\n") {
      keys.push({ type: "enter" });
      i += 1;
    } else if (c === "\x7f" || c === "\b") {
      keys.push({ type: "backspace" });
      i += 1;
    } else if (c === "\t") {
      keys.push({ type: "tab" });
      i += 1;
    } else if (c === "\x15") {
      keys.push({ type: "kill-line" }); // ctrl+U / cmd+backspace
      i += 1;
    } else if (c === "\x17") {
      keys.push({ type: "kill-word" }); // ctrl+W
      i += 1;
    } else if (c < " ") {
      i += 1; // other control bytes: ignored, same as the Rust build
    } else {
      const cp = String.fromCodePoint(data.codePointAt(i)!);
      keys.push({ type: "char", value: cp });
      i += cp.length;
    }
  }
  return keys;
}
