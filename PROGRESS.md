# PROGRESS

Kurz halten: Stand, nächster Schritt, offene Probleme. Am Ende jeder Sitzung aktualisieren.

**Aktueller Milestone:** M0 – Grundlagen (siehe SPEC.md)

Hardware: Stream Deck + ist angeschlossen (`0fd9:0084`, Bus 008).

## Erledigt
- Spezifikation (SPEC.md + docs/spec/), CLAUDE.md, Skill `session-wrapup`, `scripts/check.sh`
- Architekturentscheidungen: Action-Katalog, Shell-IPC über CLI, Hyprland-Sockets direkt, `CommandRunner`-Trait, CI erst in M8

## Nächster Schritt
- Cargo-Workspace anlegen, `CommandRunner`-Trait, `scripts/gen-omarchy-reference.sh` schreiben und `docs/omarchy-reference.md` erzeugen; danach Spike M0.5

## Geplante Skills (in .claude/skills/ anlegen, wenn der Milestone fertig ist)
- nach M3: `add-icon` (Tabler-Name suchen, icons.toml, fetch-icons.sh, Validierung)
- nach M5a: `add-action` nur für Rust-Actions mit Logik (Katalog-Einträge brauchen keinen Skill) – ggf. ganz weglassen
- nach M7: `shell-plugin` (QML-Regeln, Manifest-Muster, Reload) – Details dann aus CLAUDE.md entfernen

## Offene Probleme / Notizen
- M1: prüfen, ob die Zugriffsrechte auf das hidraw-Gerät ohne eigene udev-Regel reichen (ein hidraw hat bereits eine ACL) – Plug & Play braucht die Regel trotzdem im Paket
