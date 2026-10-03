# Icons & Assets

Jede eingebaute Action sieht ohne Zutun gut aus und passt zu jedem Omarchy-Theme.

## Beschaffung (M3) – nicht von Hand zeichnen
- **Quelle:** Tabler Icons (MIT, Outline, 24er Raster, 2 px Strich).
- `scripts/fetch-icons.sh` lädt eine fest gepinnte Tabler-Version und kopiert die in `assets/icons.toml` gelisteten Icons nach `assets/icons/<kategorie>/<name>.svg`.
- `assets/icons.toml`: `[tabler]` (Version + sha256 des npm-Tarballs), eine Tabelle je Kategorie mit `duckydeck-name = "tabler-name"`, `[pairs]` für Aktiv-/Inaktiv-Paare, `[custom]` für selbst erstellte Icons.
- **Allgemeine Motive** für eigene Belegungen: Kategorien `[general]` (Alltag, Arbeit, Dev, Freizeit, Symbole …) und `[adult]` („Party & 18+“: Drinks, Rauchen, Casino, Flirt-Smileys) – Tabler-Name = DuckyDeck-Name, keine Action nutzt sie standardmäßig (Entscheidung 2026-10-03).
- **Nur fehlende Motive** werden selbst erstellt, im Tabler-Stil, unter `assets/icons/custom/` (bisher: `omarchy-menu` = „o“ aus `/usr/share/omarchy/logo.svg`, `play-pause`). Workspace-Ziffern sind Font-Glyphen statt SVG.

## Eigene Icons (v2, Idee 3)
- Ablage `~/.config/duckydeck/icons/<name>.svg|png` (max. 1 MiB, Name `[a-z0-9_-]`), Import per `duckydeck icons add <datei> [name]` oder im Editor („Import SVG/PNG…“ über `omarchy file select`); löschen mit `duckydeck icons remove <name>`.
- `icon = "<name>"`: eingebaute Icons haben Vorrang, Namen eingebauter Icons sind beim Import gesperrt. SVGs mit `currentColor` werden wie eingebaute eingefärbt, alles andere bleibt farbig.
- Editor-Kategorie `user` („My icons“). Daemon cached geladene Icons bis zum Config-Reload; `icons add/remove` lösen den Reload selbst aus.

- Einfarbige SVGs mit `currentColor`, beim Rendern mit Theme-Tokens eingefärbt.
- Toggles: aktiv = `accent`, inaktiv = `muted`, kritisch = `red`; eigene Icon-Variante je Zustand (z. B. `volume` / `volume-off`).
- **Taste (120×120):** Icon 56 px zentriert (ohne Label 64 px), Label unten in der Systemschrift (12–14 px, 1 Zeile, Ellipse), Hintergrund `background` bzw. `lighter_background`, optional Badge (App läuft, Aufnahmezeit).
- Light-Themes (`mode = "light"`) werden unterstützt; der Renderer prüft Kontrast ≥ 4.5:1 per Test (Zahl, kein Bildvergleich durch Claude).

## Mindestumfang

| Kategorie | Icons |
|---|---|
| System | lock, suspend, reboot, power, logout, omarchy-menu, theme, wallpaper, nightlight(-off), idle-inhibit(-off), notifications / dnd, settings, terminal-command |
| Capture | screenshot-region, screenshot-window, screenshot-full, record, record-stop, ocr, qr, color-picker |
| Audio & Medien | volume / volume-low / volume-off, mic / mic-off, headphones, speaker, play, pause, play-pause, next, previous, stop, shuffle, repeat |
| Display | brightness / brightness-low, monitor, monitor-mirror, keyboard-backlight |
| Netzwerk | wifi / wifi-off, bluetooth / bluetooth-off, vpn |
| Window Management | workspace-1…10 (Glyphen), move-to-workspace, close-window, float, fullscreen, pseudo, split, focus-left/-right/-up/-down, scratchpad, gaps-toggle, layout-toggle, transparency, pop-out |
| Launcher | launcher, browser, terminal, files, editor, agent, webapp, url, generic-app |
| Struktur | folder, back, page-next, page-prev, profile, multi-action, toggle, warning |

## Weitere Assets
- **Touchstrip-Layouts** (800×100): Reglerwerte (Balken + Prozent + Icon), Medien, Workspace-Leiste.
- **Splash** beim Verbinden (Logo in `accent`, 600 ms) und **Screensaver** (gedimmt, Uhrzeit), gekoppelt an Idle/Lock der Shell.
- **Fehlerzustand:** kurzes rotes Aufblinken + `warning`-Icon.
- **App-Icons:** über das Icon-Theme des Omarchy-Themes (`icons.theme`), Fallback `generic-app`. Suche: `Icon=` der Desktop-Datei → Theme, `Inherits`-Kette, `hicolor`, `pixmaps`; größte Variante, SVG vor PNG; PNG wird farbig gezeichnet.
- v2: Geräte-Grafik `assets/device/streamdeck-plus.svg` für den Editor.

## Ablage & Prüfung
```
assets/
  icons.toml                     # Mapping + Kategorien
  icons/<kategorie>/<name>.svg   # einfarbig, currentColor, viewBox 0 0 24 24
  icons/custom/<name>.svg        # selbst erstellt, vom Script unberührt
  icons/LICENSE-tabler.txt
  strip/*.toml                   # Touchstrip-Layouts
  brand/logo.svg
```
- Build-Validierung (`duckydeck-core/build.rs`): viewBox, nur `currentColor`/`none`, keine Bitmaps/Styles; Icons per `include_bytes!` eingebettet (`icons::get`). Kontrasttest misst gerenderte Pixel in allen Stock-Themes.
- Galerie aller Icons in allen Stock-Themes: `cargo run -p duckydeck-core --example gallery` (→ `target/gallery.png`) → ein PNG für Menschen/CI. Claude öffnet die Galerie nicht.
