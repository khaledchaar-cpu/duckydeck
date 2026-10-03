# PROGRESS

Kurz halten: Stand, nächster Schritt, offene Probleme. Am Ende jeder Sitzung aktualisieren.

**Aktueller Milestone:** M6 – IPC & CLI (M5c abgeschlossen)

Hardware: Stream Deck + ist angeschlossen (`0fd9:0084`, Bus 008).

## Erledigt
- M5c: Katalogfeld `state` (`file` per inotify / `command` nach Druck, `json`, `elapsed`), Task `toggles.rs`
- Toggle-Icons für Nachtlicht, Wach bleiben, Nicht stören; unbekannter Status = `off`
- Neu: `capture.screenrecording` mit Laufzeit, `capture.color_picker` (hyprpicker), `system.dismiss_notifications` (Icon `bell-x`)
- Am Gerät abgenommen (OCR/QR nur angetippt)

## Nächster Schritt
- M6: Unix-Socket-Protokoll (JSON-Lines) im Daemon + `duckydeck`-CLI-Grundgerüst laut `docs/spec/`

## Geplante Skills (in .claude/skills/ anlegen, wenn der Milestone fertig ist)
- nach M5a: `add-action` nur für Rust-Actions mit Logik – Entscheidung offen (ggf. weglassen)
- nach M7: `shell-plugin` (QML-Regeln, Manifest-Muster, Reload) – Details dann aus CLAUDE.md entfernen

## Offene Probleme / Notizen
- Multi-Monitor-Actions (`to_monitor`, `monitor_internal`/`mirror`) nicht testbar (nur DP-1)
- `scripts/check.sh` findet `cargo` nur mit `PATH=$HOME/.cargo/bin:$PATH`
- RSS ~27 MB (Debug) statt < 15 MB – laut Nutzer unkritisch; Release-Build noch nicht gemessen
- Strip zeigt immer den automatisch gewählten Player, Tasten mit `player = …` ggf. einen anderen
- Catalog-Actions per `spawn` – Fehler von `omarchy` werden nicht gemeldet
- Nachtlicht-Status ohne Event: Änderung per Tastatur erst beim nächsten Druck sichtbar
- Aufnahme-Zeit läuft während des Nachbearbeitens (ffmpeg) nach dem Stopp kurz weiter
- Long-Press-Bestätigung (Power off) und mehrseitige Profile/Swipe noch nicht am Gerät geprüft
- Erstanbieter-Panels sind `bar-widget`-Plugins (`entryPoints.barWidget`) – bei M7 beachten
- Hyprland-Zugriff noch nicht hinter Trait (Spec architecture.md: Fake für Tests) – Logik ist in Core testbar, I/O nicht
- `duckydeck.service` fehlt noch (Packaging); Kontrasttest braucht `/usr/share/omarchy/themes`
