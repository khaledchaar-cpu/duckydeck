# Konfiguration & CLI

## Dateien
- `~/.config/duckydeck/config.toml` – global (Helligkeit, Screensaver-Zeit, Default-Profil)
- `~/.config/duckydeck/profiles/<name>.toml` – ein Profil mit Seiten
- Live-Reload per inotify. Ungültige Config → letzte gültige bleibt aktiv, Fehler als Shell-Benachrichtigung mit Datei und Zeile.
- Optional: ein Omarchy-Theme kann `duckydeck.toml` mitbringen (Tasten-Hintergrund, Icon-Farben), das die aus `colors.toml` abgeleiteten Werte überschreibt.

Da Layouts in v1 von Hand bearbeitet werden, gilt: Format kurz und lesbar halten, `examples/profiles/` mit kommentierten Beispielprofilen pflegen, `duckydeck check` validiert ohne Neustart.

## Beispiel (Richtwert, finales Schema in M4)

```toml
name = "Omarchy"
match = { class = "^(Alacritty|kitty)$" }   # optional: Auto-Profilwechsel

[[pages]]
keys = [
  { action = "window.workspace", args = { n = 1 } },
  { action = "launcher.app", args = { desktop = "firefox.desktop" }, label = "Web" },
  { action = "capture.screenshot", args = { mode = "region" } },
  { action = "system.menu" },
]
dials = [
  { action = "media.volume", step = 5 },
  { action = "media.mic" },
  { action = "display.brightness", step = 5 },
  { action = "window.workspace_scroll" },
]
```

## CLI

```
duckydeck status | reload | check
duckydeck profile <name> | page <n> | brightness <0-100>
duckydeck export <profil> > profil.toml
duckydeck setup [--remove]
```
