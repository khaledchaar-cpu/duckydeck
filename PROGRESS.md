# PROGRESS

Kurz halten: Stand, nächster Schritt, offene Probleme. Am Ende jeder Sitzung aktualisieren.

**Aktueller Milestone:** M2 – Rendering & Theme (siehe SPEC.md)

Hardware: Stream Deck + ist angeschlossen (`0fd9:0084`, Bus 008).

## Erledigt
- M2 Teil 1: `theme.rs` (colors.toml, Fallbacks für unvollständige Nutzer-Themes, WCAG-Kontrast, `readable_on` hebt Farben auf ≥ 4.5:1)
- `render.rs`: Tasten-Renderer (resvg-Icon mit `currentColor` → Token, cosmic-text-Label mit Ellipse), Snapshot nur für Tasten ohne Text
- `font.rs`: Schrift über `omarchy font current` + `fc-match`, Renderer lädt nur diese eine Datei
- Daemon: Demo-Tasten mit echtem Renderer, am Gerät verifiziert; RSS Release 10,4 MB

## Nächster Schritt
- M2 Teil 2: Strip-Renderer (800×100, Reglerwerte: Balken + Prozent + Icon) und Theme-/Font-Hooks (Neu-Rendern bei `theme-set`/`font-set`)

## Geplante Skills (in .claude/skills/ anlegen, wenn der Milestone fertig ist)
- nach M3: `add-icon` (Tabler-Name suchen, icons.toml, fetch-icons.sh, Validierung)
- nach M5a: `add-action` nur für Rust-Actions mit Logik (Katalog-Einträge brauchen keinen Skill) – ggf. ganz weglassen
- nach M7: `shell-plugin` (QML-Regeln, Manifest-Muster, Reload) – Details dann aus CLAUDE.md entfernen

## Offene Probleme / Notizen
- Erstanbieter-Panels (Audio usw.) sind Plugins vom Typ `bar-widget` mit `entryPoints.barWidget: Panel.qml`, nicht `panel` – beim Spike/M7 berücksichtigen (ggf. `docs/spec/ui.md` anpassen)
- Drehen bei gedrücktem Regler noch nicht am Gerät geprüft (nur Unit-Test)
- Shells ohne `~/.cargo/bin` im PATH: vor cargo `. ~/.cargo/env`
