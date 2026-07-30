# twodo

A blazing fast TUI task manager and note taker for devs, with **automatic AI agent
attribution** — it tells you how many agents you burned per task, without you
tagging anything.

```
┌ tasks · twodo ───────────────┐┌ notes ────────────────────────────┐
│▌◐ Wire up the scanner  ⛁7 42m││ Codex rollout logs put cwd in     │
│ ○ Add sqlite backend         ││ session_meta, not per-line.       │
│ ● Ship v0.1             ⛁3   ││                                   │
└──────────────────────────────┘└───────────────────────────────────┘
                                ┌ agents ───────────────────────────┐
                                │ 7 agents  2 sessions  5 subagents │
                                │ 214 tool calls · 1.2M in / 18k out│
                                │ claude:Explore    ██████████ 3    │
                                │ codex:session     ██████ 2        │
                                └───────────────────────────────────┘
```

## How agent attribution works

There is no manual tagging. twodo reads your session logs directly:

- **Claude Code** — `~/.claude/projects/**/*.jsonl`. Each session is one main
  agent; every `Task`/`Agent` tool call is a subagent, labelled by its
  `subagent_type`. Token usage and tool calls come from the same lines.
- **Codex** — `~/.codex/sessions/**/*.jsonl`. `session_meta` gives the cwd,
  `token_count` the running usage, and `spawn_agent` calls are subagents.
  Lifecycle calls (`wait_agent`, `list_agents`, `close_agent`) are deliberately
  *not* counted as spawns.

A task owns a **project directory** and a set of **active windows**. A window
opens when you mark the task `◐ Doing` and closes when you move it off. Any
agent activity in that project (or a subdirectory) that *overlaps* a window is
credited to the task — so a long session you were already inside when you
marked the task Doing still counts, rather than being silently dropped.

If two tasks' windows overlap, the **most recently started** one owns the
instant — so a run is counted exactly once, never double counted.

The practical consequence: mark a task Doing before you start working on it,
and the accounting takes care of itself.

## Launching agents (herdr)

If [`herdr`](https://herdr.dev) is on your PATH, each task can own an agent tab:

- `o` — launch this task's agent, or focus it if one is already running
- `O` — pick the agent kind (claude, codex, gemini, opencode, cursor, amp)
- `x` — close the task's tab

Launching creates a herdr tab labelled with the task, `cwd` set to the task's
project, starts the agent, and submits the task title and notes as its opening
prompt. It also flips the task to `◐ Doing`, which opens the attribution window
— so the agent you just launched is counted against the task automatically.

The task list shows the tab's live state: `◉` working, `✔` done, `◎` idle,
`◌` unknown. If you close a tab in herdr, twodo notices and drops the link.

Without herdr installed, every other feature works as normal.

## Install

```sh
cargo build --release
cp target/release/twodo ~/.local/bin/     # or anywhere on PATH
```

## Use

```sh
twodo            # tasks for the current directory
twodo ~/some/repo # tasks for another project
twodo --scan     # debug: print what the scanner sees, no TUI
```

## Keys

| key | action | key | action |
|---|---|---|---|
| `j` `k` `↓` `↑` | move | `a` | add task |
| `space` | cycle status | `e` | rename task |
| `1` `2` `3` | todo / doing / done | `i` `enter` | edit notes |
| `tab` `shift-tab` | cycle panes | `esc` | back / save |
| `A` | jump to agents pane | `enter` | agent run detail |
| `y` | copy run's log path | | |
| `/` | search titles + notes | `d` `d` | delete task |
| `r` | rescan agents | `p` | toggle all projects |
| `g` `G` | top / bottom | `?` | help |
| `q` | quit | | |

## Storage

Plain JSON at `~/Library/Application Support/twodo/store.json` (macOS) or
`~/.local/share/twodo/store.json` (Linux). Writes are atomic. Session logs are
never modified — twodo only reads them. A scan cache keyed on file mtime+size
sits alongside the store.

## Performance

Scanning is incremental and runs off the UI thread, so the interface never
blocks. Files whose mtime predates your earliest task window are skipped
entirely.

On a real history of ~4.9k sessions: **6.2s cold, 26ms warm.**
