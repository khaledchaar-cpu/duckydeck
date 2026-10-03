# SPEC – DuckyDeck

Version 0.3 · Stand: 2026-10-03 · M0–M9 fertig, v0.1.0 veröffentlicht

Diese Datei ist der **Einstieg**. Details stehen in Themen-Dateien – nur die lesen, die für die aktuelle Aufgabe nötig ist.

| Thema | Datei |
|---|---|
| Omarchy-Plattform, Architektur, IPC, Plug & Play, nicht-funktionale Anforderungen | [docs/spec/architecture.md](docs/spec/architecture.md) |
| Eingebaute Actions, Profile, Kontextwechsel | [docs/spec/actions.md](docs/spec/actions.md) |
| Icons, Tastenlayout, Touchstrip, weitere Assets | [docs/spec/assets.md](docs/spec/assets.md) |
| Shell-Plugins (Bar-Widget, Panel), Editor (v2) | [docs/spec/ui.md](docs/spec/ui.md) |
| Config-Format, CLI | [docs/spec/config.md](docs/spec/config.md) |
| IPC-Protokoll Daemon ↔ CLI | [docs/ipc.md](docs/ipc.md) |
| Verwendete Omarchy-Routen und Shell-APIs (generiert) | [docs/omarchy-reference.md](docs/omarchy-reference.md) |

## Vision

DuckyDeck macht Elgato Stream Decks – Referenz: Stream Deck + (8 LCD-Tasten, 4 Drehregler, Touchstrip) – zu einem festen Bestandteil von Omarchy: Bedienung in der Omarchy-Shell, Look aus dem aktiven Theme, Feedback über OSD und Shell-Benachrichtigungen, Aktionen über die `omarchy`-CLI. Einstecken – funktioniert.

## Abgrenzung

- **Zielgruppe:** Omarchy-Nutzer mit Stream Deck (Stream Deck + voll getestet, andere Modelle experimentell).
- **Nicht-Ziele:** andere Distros/Desktops, ältere Omarchy-Versionen (Waybar/Walker/mako), Elgato-Plugins, Cloud-Sync, eigenständige GTK/Qt-App.

## Umfang v1 / v2

| v1 | v2 |
|---|---|
| Rust-Daemon, Hotplug, Rendering, Theme-Sync | ✅ Grafischer Editor mit Drag & Drop (Shell-Overlay, M9) |
| Alle Action-Kategorien (System, Capture, Medien, Launcher, Window Management, Struktur) | offen: Lautstärke pro App (PipeWire-Streams) |
| Layouts als TOML mit Live-Reload + Beispielprofile | offen: Zweite UI-Sprache (Deutsch) |
| Bar-Widget + Panel (Status, Profil/Seite, Helligkeit) | offen: Eigene Script-Actions mit JSON-Protokoll |
| Auto-Profilwechsel, Multi-/Toggle-Actions, CLI | |
| UI-Texte nur Englisch, aber zentral abgelegt | |

## Milestones

Ein Schritt = etwa eine Claude-Sitzung. Fortschritt steht in [PROGRESS.md](PROGRESS.md).

| # | Ziel | Inhalt |
|---|---|---|
| M0 | Grundlagen | Cargo-Workspace, `CommandRunner`-Trait, `scripts/gen-omarchy-reference.sh` → `docs/omarchy-reference.md` (kein CI) |
| M0.5 | Spike Shell-IPC | Minimales QML-Testplugin startet einen Dummy-`Process`, liest JSON-Zeilen, startet bei Abbruch neu – Ergebnis in `docs/spec/architecture.md` festhalten |
| M1 | Hardware | **Am echten Gerät:** Erkennung, Hotplug, Tasten/Regler/Touch-Events (Tap, Long-Press, Swipe), udev-Regel; danach Fake-Device mit Gesamtbild, abgeglichen mit echten Event-Mitschnitten |
| M2 | Rendering & Theme | Tasten- und Strip-Renderer, `colors.toml`-Parser, Theme-/Font-Hooks, Kontrasttest |
| M3 | Icons | `scripts/fetch-icons.sh` + `assets/icons.toml`, eigene Ergänzungs-Icons, Validierung |
| M4 | Config | TOML-Format, Live-Reload, Default-Profil, Seiten/Ordner |
| M5 | Action-Katalog | `actions/catalog.toml`, `CommandAction`, Routenprüfung, Katalog-Test |
| M5a | Actions: Medien | Lautstärke, Mikrofon, Helligkeit, MPRIS, Touchstrip-Medienanzeige |
| M5b | Actions: Window Management | Hyprland-Socket-Modul, Workspaces, Fenster-Dispatches, Omarchy-Fenster-Routen |
| M5c | Actions: System & Capture | `omarchy system/toggle/capture/theme …`, Laufzeitanzeige Aufnahme |
| M5d | Actions: Launcher | `.desktop`, `omarchy launch …`, Fokussieren-oder-Starten, Badges |
| M6 | IPC & CLI | Unix-Socket-Protokoll, `duckydeck`-CLI inkl. `subscribe`/`--json`, `duckydeck setup` |
| M7 | Shell-Plugins | Bar-Widget, Panel, Menüeintrag |
| M8 | Kontext & Release | Auto-Profilwechsel, Multi-/Toggle-Actions, CI (GitHub Actions: fmt/clippy/test), PKGBUILD, Doku, Release-Paket auf GitHub (kein AUR) |
| M9 | Editor (v2) | Schritte M9a–M9e siehe [docs/spec/ui.md](docs/spec/ui.md) |

## Ideen (in Scope, Reihenfolge abgestimmt 2026-10-03)

Jede Idee wird vor der Umsetzung einzeln abgestimmt; Entscheidungen landen in der passenden `docs/spec/*.md`, Schritte als neue Milestones.

| # | Idee | Notiz |
|---|---|---|
| 1 | Lückenanalyse Controls – Katalog-Teil erledigt 2026-10-03, Rust-Teil offen (Regler Tastaturbeleuchtung, Energieprofil-Zyklus) | `actions.md` |
| 2 | Icon-Auswahl mit visueller Vorschau des Katalogs | Editor (`ui.md`, `assets.md`) |
| 3 | Eigene Icons (Upload) und freie Icon-Bibliotheken | KI-Generierung zurückgestellt; Netzwerk nur per expliziter Nutzer-Action (Regel 6) |
| 4 | Weitere Stream-Deck-Modelle | über Gerätefähigkeiten, ungetestet → experimentell, Fake-Device pro Modell |
| – | UX-Polishing | laufend, gesammelt aus konkreten Stolperstellen |
