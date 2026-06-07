# animus-tui

Terminal control plane for [Animus](https://github.com/launchapp-dev/animus-cli) — a `k9s`-style TUI for inspecting and driving the Animus daemon.

`animus-tui` is a sibling client to the CLI and web UI. It connects to the running daemon via the local control socket and surfaces seven views (workflows, subjects, queue, daemon health, log tail, cost, plugins) with vim-style keybinds.

## Install

```bash
animus plugin install launchapp-dev/animus-tui@v0.1.0
```

The binary lands at `~/.animus/plugins/animus-tui/animus-tui` and is shimmed onto your `PATH` by the Animus install pipeline.

## Run

```bash
# From any project root with a running daemon
animus-tui
```

Optional flag:

- `--daemon-socket <path>` — override the daemon control socket path. By default `animus-tui` resolves it the same way the CLI does (`~/.animus/<repo-scope>/control.sock`).

If no daemon is running, `animus-tui` prints a short splash and exits 0:

```text
Animus daemon is not running.

Start it with:    animus daemon start
```

## Keybinds

v0.1 keymap:

| Key      | Action                                  |
|----------|-----------------------------------------|
| `1`-`7`  | Switch view                             |
| `h/j/k/l`| Move (left/down/up/right)               |
| `f`      | Cycle severity filter (logs view)       |
| `tk`     | Subjects → task kind                    |
| `tr`     | Subjects → requirement kind             |
| `r`      | Force refresh                           |
| `?`      | Help overlay                            |
| `q`      | Quit                                    |
| `Ctrl-C` | Quit (signal-clean)                     |

`Enter` (detail pane), `/` (search), and `Esc` (back) are reserved for v0.2.

Views:

1. **Workflows** — recent runs with status, definition, subject id, and age. *(v0.2: detail pane + cancel/pause/resume.)*
2. **Subjects** — tasks + requirements filtered by kind (`tk`/`tr` switches kind). *(v0.2: status-change action + search.)*
3. **Queue** — ready / held / in-flight entries. *(v0.2: hold / release / drop / reorder.)*
4. **Daemon** — PID, uptime, daemon version, plugin health snapshot.
5. **Logs** — live tail of daemon log entries with severity filter.
6. **Cost** — token + USD totals from `~/.animus/<scope>/cost-state.v1.json` (file created by the budget-caps feature landed in daemon v0.5.5).
7. **Plugins** — installed plugin list with kind + version + source (read-only in v0.1).

The sticky header shows daemon status (green/yellow/red), active workflow count, queue depth, and the top-spend workflow today.

## License

MIT
