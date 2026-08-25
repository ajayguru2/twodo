# twodo

A quiet todo list for developers, right in the terminal.

Every task has one free-form note. Open a task, write down the context you need,
then get back to work.

```text
  twodo / compiler                                      2 open
  ────────────────────────────────────────────────────────────
  01  ○  Reproduce the parser crash
  02  ○  Add the missing span
  03  ✓  Write the regression test

  enter open   n new   space done   e rename   d delete   q quit
```

## Install

```sh
brew tap ajayguru2/tap
brew trust ajayguru2/tap
brew install twodo
```

Homebrew will not load formulae from an untrusted third-party tap, so the
`brew trust` step is required. To skip it, install by full name instead:

```sh
brew install ajayguru2/tap/twodo
```

Or from source, with [Bun](https://bun.sh):

```sh
bun install
bun run build                             # writes a standalone ./twodo binary
cp twodo ~/.local/bin/                    # or anywhere on PATH
```

## Use

```sh
twodo             # tasks for the current directory
twodo ~/some/repo  # tasks for another project
```

## Keys

| key | action |
|---|---|
| `j` `k` `↓` `↑` | move through tasks |
| `enter` or `i` | open the selected task's note |
| `n` or `a` | add a task |
| `e` | rename a task |
| `space` | mark a task done or open |
| `d` `d` | delete a task |
| `esc` | save the note and return to tasks |
| arrows, `home`, `end` | move through a note |
| `q` | quit |

## Develop

```sh
bun run start     # run it straight from source
bun test
bun run typecheck
```

## Storage

Tasks are plain JSON at `~/Library/Application Support/twodo/store.json` on
macOS or `~/.local/share/twodo/store.json` on Linux. Writes are atomic.
