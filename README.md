# DuckyDeck

The Elgato **Stream Deck +** as a native part of [Omarchy](https://omarchy.org):
8 keys, 4 dials and the touch strip, styled by your Omarchy theme, controlled from
the Omarchy shell, actions running through the `omarchy` CLI.

Plug it in – it works.

- Keys follow the active theme and font, live
- Dials for volume, mic, brightness and workspaces; the strip shows their levels or the playing media
- Bar icon and panel in the Omarchy shell (profile, page, brightness)
- Profiles with pages and folders as plain TOML, reloaded on save
- Automatic profile switching by the focused window
- Multi actions (sequences with delays) and toggle keys
- Small and quiet: one Rust daemon, ~14 MB RAM, no polling, no network, no root

Only for current Omarchy and only for the Stream Deck + (USB `0fd9:0084`).

## Install

```bash
sudo pacman -U https://github.com/khaledchaar-cpu/duckydeck/releases/latest/download/duckydeck-0.1.0-1-x86_64.pkg.tar.zst
```

Packages for every version are on the [releases page](https://github.com/khaledchaar-cpu/duckydeck/releases).

Then plug the deck in (or replug it). That's all: the package ships a udev rule
(access without root, starts the daemon on plug-in) and a systemd user service
for every user. On its first start the daemon runs `duckydeck setup`, which adds
the bar widget, an Omarchy menu entry and a font hook. You get a
“Stream Deck + connected” notification.

## Use

- **Bar icon / panel:** click the deck icon in the bar to switch profile and page,
  set the brightness or reload. Also in the Omarchy menu as *Stream Deck*.
- **Touch strip:** swipe left/right to change the page.
- **CLI:**

```
duckydeck status                 # device, profile, page, brightness
duckydeck profile <name>         # switch profile
duckydeck page <n>               # open page n
duckydeck brightness <0-100>
duckydeck check                  # validate your config without restarting
duckydeck export omarchy > ~/.config/duckydeck/profiles/mine.toml
duckydeck reload
```

Add `--json` for machine-readable output.

## Configure

Everything lives in `~/.config/duckydeck/` and reloads when you save. An invalid
file shows a notification with file and line; the last valid config stays active.

`config.toml` (all optional):

```toml
brightness = 60           # 0-100
screensaver_minutes = 10  # dim after inactivity, 0 = never
profile = "omarchy"       # profile when no window rule matches
```

A profile is `profiles/<name>.toml`. Start from the built-in one
(`duckydeck export omarchy`), see [examples/profiles/omarchy.toml](examples/profiles/omarchy.toml):

```toml
name = "Coding"
match = { class = "^(Alacritty|kitty|com.mitchellh.ghostty)$" }   # optional, regex

[[pages]]
keys = [
  { action = "window.workspace", args = { n = 1 } },
  { action = "launcher.terminal" },
  { action = "structure.folder", args = { folder = "power" }, label = "Power" },
  {},                                                    # empty key
]
dials = [{ action = "media.volume", args = { step = 5 } }]

[folders.power]
keys = [{ action = "system.lock" }, { action = "system.suspend" }]
```

### Automatic profile switching

A profile with `match = { class = "…", title = "…" }` becomes active while a
matching window has focus (all given regexes must match; the first profile by
file name wins). Otherwise the deck shows `profile` from `config.toml`, or the
profile you picked last in the panel or CLI.

### Actions

| Group | Actions |
|---|---|
| `system` | `lock`, `suspend`, `reboot`, `shutdown`, `logout`, `menu`, `theme`, `wallpaper`, `nightlight`, `idle`, `dnd`, `dismiss_notifications` |
| `capture` | `screenshot`, `screenrecording`, `color_picker`, `ocr`, `qr` |
| `launcher` | `terminal`, `browser`, `files`, `app` (`app = "<desktop-entry id>"`, e.g. `"omacalc"`) |
| `window` | `workspace`, `move_to_workspace`, `close`, `float`, `fullscreen`, `tiled_fullscreen`, `pseudo`, `split`, `pop`, `focus`, `move`, `scratchpad`, `transparency`, `gaps`, `layout`, `to_monitor`, `monitor_internal`, `monitor_mirror`, `dispatch` |
| `media` | `play_pause`, `next`, `previous` (keys); `volume`, `mic` (dials) |
| `display` | `brightness` (dial) |
| `window` (dial) | `workspace_scroll` |
| `structure` | `folder`, `page`, `back`, `profile`, `multi`, `toggle` |

Arguments of each action are documented in [actions/catalog.toml](actions/catalog.toml).
`system.reboot`, `system.shutdown` and `system.logout` need a long press.

Multi and toggle keys wrap other actions:

```toml
# Runs the steps in order.
{ action = "structure.multi", label = "Focus", args = { steps = [
    { action = "system.dnd" },
    { delay_ms = 300 },
    { action = "window.workspace", args = { n = 3 } },
] } }

# Alternates between two actions; the key shows what the next press does.
{ action = "structure.toggle", args = { states = [
    { action = "structure.profile", args = { profile = "coding" }, label = "Coding" },
    { action = "structure.profile", args = { profile = "omarchy" }, label = "Default" },
] } }
```

## Disable or remove

```bash
systemctl --user mask --now duckydeck   # stop for good (disable has no effect, it is enabled globally)
duckydeck setup --remove                # remove bar widget, editor, menu entry and hook
omarchy pkg drop duckydeck
```

Run `setup --remove` while the shell is running: it takes the widget out of the
bar layout via `omarchy plugin disable`.

## Develop

```bash
systemctl --user stop duckydeck
cargo run -p duckydeckd -- --debug
DUCKYDECK_FAKE_DEVICE=1 cargo run -p duckydeckd   # no device: renders to $XDG_RUNTIME_DIR/duckydeck/fake/deck.png
scripts/check.sh                                  # fmt + clippy + tests
```

Package: [packaging/PKGBUILD](packaging/PKGBUILD). Specification: [SPEC.md](SPEC.md) (German).

## License

MIT. Icons: [Tabler Icons](https://tabler.io/icons) (MIT).
