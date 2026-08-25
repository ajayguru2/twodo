# twodo

A blazing fast TUI task manager and note taker for devs.

```
┌ tasks · twodo ───────────────┐┌ notes ────────────────────────────┐
│▌◐ Wire up the scanner     42m││ Codex rollout logs put cwd in     │
│ ○ Add sqlite backend         ││ session_meta, not per-line.       │
│ ● Ship v0.1                  ││                                   │
└──────────────────────────────┘└───────────────────────────────────┘
```

Marking a task `◐ Doing` starts its clock; moving it off stops it. The task
list shows the total time spent on each task.

## Install

```sh
cargo build --release
cp target/release/twodo ~/.local/bin/     # or anywhere on PATH
```

## Use

```sh
twodo             # tasks for the current directory
twodo ~/some/repo  # tasks for another project
```

## Keys

| key | action | key | action |
|---|---|---|---|
| `j` `k` `↓` `↑` | move | `a` | add task |
| `space` | cycle status | `e` | rename task |
| `1` `2` `3` | todo / doing / done | `i` `enter` | edit notes |
| `tab` `shift-tab` | cycle panes | `esc` | back / save |
| `/` | search titles + notes | `d` `d` | delete task |
| `p` | toggle all projects | | |
| `g` `G` | top / bottom | `?` | help |
| `q` | quit | | |

## Storage

Plain JSON at `~/Library/Application Support/twodo/store.json` (macOS) or
`~/.local/share/twodo/store.json` (Linux). Writes are atomic.
