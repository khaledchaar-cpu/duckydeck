# PROGRESS

Kurz halten: Stand, nächster Schritt, offene Probleme. Am Ende jeder Sitzung aktualisieren.

**Aktueller Milestone:** M3 – Icons (M2 abgeschlossen)

Hardware: Stream Deck + ist angeschlossen (`0fd9:0084`, Bus 008).

## Erledigt
- M2 Teil 2: `Renderer::segment` (Strip-Segment 200×100: Icon, rechtsbündiger Text, Balken mit `muted`-Spur), Snapshot- und Pixeltests
- Demo-Strip im Daemon nutzt den Renderer, am Gerät verifiziert
- Theme-Wechsel: inotify auf `~/.local/state/omarchy/current/` (`omarchy theme set` ersetzt `theme/` per `mv`) → Neu-Rendern; Spec angepasst
- Font-Reload kommt in M6 (Hook `font-set.d/duckydeck` → `duckydeck reload`), bis dahin nur beim Start
- `--debug` filtert `cosmic_text`-Fallback-Meldungen

## Nächster Schritt
- M3: `assets/icons.toml` + `scripts/fetch-icons.sh` (Tabler) laut `docs/spec/assets.md`, Validierung

## Geplante Skills (in .claude/skills/ anlegen, wenn der Milestone fertig ist)
- nach M3: `add-icon` (Tabler-Name suchen, icons.toml, fetch-icons.sh, Validierung)
- nach M5a: `add-action` nur für Rust-Actions mit Logik (Katalog-Einträge brauchen keinen Skill) – ggf. ganz weglassen
- nach M7: `shell-plugin` (QML-Regeln, Manifest-Muster, Reload) – Details dann aus CLAUDE.md entfernen

## Offene Probleme / Notizen
- Erstanbieter-Panels (Audio usw.) sind Plugins vom Typ `bar-widget` mit `entryPoints.barWidget: Panel.qml`, nicht `panel` – beim Spike/M7 berücksichtigen (ggf. `docs/spec/ui.md` anpassen)
- Drehen bei gedrücktem Regler noch nicht am Gerät geprüft (nur Unit-Test)
- Shells ohne `~/.cargo/bin` im PATH: vor cargo `. ~/.cargo/env`
