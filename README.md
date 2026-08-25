# twodo

A quiet todo list for developers, right in the terminal.

Every task has one free-form note. Open a task, write down the context you need,
then get back to work.

Tasks belong to the directory you created them in. Run twodo in a project and
you see that project. Run it a level up and you see the projects below it, as a
tree you walk into.

```text
  twodo  /  work
  2 open  ·  1 done
  ────────────────────────────────────────────────────────────
  01  ▸  compiler/   3 open · 8 done
  02  ○  Renew the certificate
  03  ✓  Write the release notes

  ↑↓ move   enter open   n add   e rename   space done   q quit
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
twodo              # this directory, and any projects below it
twodo ~/some/repo  # another project
twodo ~            # every project you track, as a tree
```

## Keys

| key | action |
|---|---|
| `j` `k` `↓` `↑` | move through the list |
| `enter` `l` `→` | open a note, or enter a sub-project |
| `esc` `h` `←` | go back up to the parent directory |
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
