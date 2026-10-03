---
name: shell-plugin
description: Change the DuckyDeck Omarchy-shell plugin (QML in shell-plugins/duckydeck.widget – bar icon and panel). Use for any QML/UI work, new panel controls, strings, or when the shell does not pick up plugin changes.
---

# Shell plugin (`duckydeck.widget`)

One `bar-widget` plugin like the first-party panels: `Panel.qml` (base `Ui/Panel`: bar icon + `KeyboardPanel`, IPC target `duckydeck.widget` with `open|close|toggle`), `Service.qml` (`duckydeck subscribe`, restart with backoff), `Strings.qml` (all UI texts, English).

## Rules
- Only shell components/tokens (`qs.Ui`, `qs.Commons`: `Style`, `Color`); colors via `root.foreground`/`bar`. Look up props/signals in `docs/omarchy-reference.md`, examples in `/usr/share/omarchy/shell/plugins/panels/` (grep, don't read whole files).
- Daemon only through the CLI: state from `Service.status` (full status per event, see `docs/ipc.md`), commands via `root.run([...])` (async, 5 s timeout, stderr → hero meta). Never block the shell.
- New texts go into `Strings.qml`. New focusable controls: add to `cursorItems`, set `hasCursor: root.hasCursor("<id>")`, handle in `adjust()`/`activate()`.

## Test loop
1. `omarchy plugin validate shell-plugins/duckydeck.widget`
2. **Hot reload does not work** for the symlinked plugin (the shell's `inotifywait` does not follow the link, re-linking doesn't reload the bar widget) → `omarchy restart shell`.
3. Errors/`console.log`: `qs log -p /usr/share/omarchy/shell | grep -i 'ducky\|warn' | tail`
4. Open: `omarchy-shell duckydeck.widget open`; keys via `wtype j`; screenshot only the panel area with `grim -g "<x>,<y> 420x260"` (top right of the monitor).
5. Remove debug logs before committing.
