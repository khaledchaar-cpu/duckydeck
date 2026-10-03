# PROGRESS

Kurz halten: Stand, nächster Schritt, offene Probleme. Am Ende jeder Sitzung aktualisieren.

**Aktueller Milestone:** M1 – Hardware (siehe SPEC.md)

Hardware: Stream Deck + ist angeschlossen (`0fd9:0084`, Bus 008).

## Erledigt
- M0.5 Spike Shell-IPC: `shell-plugins/spike/` (Service-Plugin, `Process` + `SplitParser`, Neustart mit Backoff) am echten Shell getestet – stabil, keine Waisen
- Ergebnisse in `docs/spec/architecture.md` (Abschnitt IPC)

## Nächster Schritt
- M1 Hardware: `elgato-streamdeck` einbinden, Stream Deck + (PID `0x0084`) im Daemon erkennen und Tasten-/Regler-/Touch-Events per `tracing` loggen (am echten Gerät)

## Geplante Skills (in .claude/skills/ anlegen, wenn der Milestone fertig ist)
- nach M3: `add-icon` (Tabler-Name suchen, icons.toml, fetch-icons.sh, Validierung)
- nach M5a: `add-action` nur für Rust-Actions mit Logik (Katalog-Einträge brauchen keinen Skill) – ggf. ganz weglassen
- nach M7: `shell-plugin` (QML-Regeln, Manifest-Muster, Reload) – Details dann aus CLAUDE.md entfernen

## Offene Probleme / Notizen
- M1: prüfen, ob die Zugriffsrechte auf das hidraw-Gerät ohne eigene udev-Regel reichen (ein hidraw hat bereits eine ACL) – Plug & Play braucht die Regel trotzdem im Paket
- Erstanbieter-Panels (Audio usw.) sind Plugins vom Typ `bar-widget` mit `entryPoints.barWidget: Panel.qml`, nicht `panel` – beim Spike/M7 berücksichtigen (ggf. `docs/spec/ui.md` anpassen)
- Shells ohne `~/.cargo/bin` im PATH: vor cargo `. ~/.cargo/env`
