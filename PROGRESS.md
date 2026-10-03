# PROGRESS

Kurz halten: Stand, nächster Schritt, offene Probleme. Am Ende jeder Sitzung aktualisieren.

**Aktueller Milestone:** alle Milestones (M1–M9) fertig, v0.1.0 auf GitHub veröffentlicht

Hardware: Stream Deck + angeschlossen (`0fd9:0084`). Paket 0.1.0 installiert (`/usr/bin/duckydeck`); Entwicklung mit `systemctl --user stop duckydeck` + `cargo run`, danach Dienst wieder starten.

## Erledigt (letzte Sitzung)
- Lückenanalyse Controls, Katalog-Teil: `system.panel`, `system.bluetooth` (mit Zustand), `system.power_profile`, `system.touchpad`, `system.keyboard_backlight`, `system.reminder`, `system.agent`, `media.output_switch`, `media.source_switch` (+ Icons)
- `gen-omarchy-reference.sh` um `agent`, `bluetooth`, `powerprofiles`, `reminder` erweitert
- Mit Fake-Daemon geprüft (`duckydeck actions` listet alle); am echten Gerät noch nicht gedrückt
- Status-Text statt Label (Katalogfeld `text`): Output zeigt Ausgabegerät, Quelle zeigt aktiven Player; Icons Lautsprecher/Note
- Schritt 2: Bluetooth zeigt verbundenes Gerät, aktives Energieprofil leuchtet (D-Bus-Events); größere Schrift für Labels/Regler/Strip
- Schritt 3: Erinnerung zeigt Fälligkeit, Tastaturlicht Stufe (ungetestet, Desktop ohne Tastaturlicht)
- `choice_icons`: Icon je Platzhalterwert (Shell-Panels, Fokus-Richtung statt Sonderfall im Daemon)

## Nächster Schritt
- Am Gerät prüfen: Bluetooth-Gerätename (beim Test war kein Gerät verbunden), größere Schrift
- Rust-Teil aus Idee 1: Regler Tastaturbeleuchtung (Energieprofil-Zyklus entfällt, aktive Taste leuchtet)
- Testprofil `~/.config/duckydeck/profiles/test-controls.toml` danach löschen
- Danach Release 0.1.1 erwägen, dann Idee 2 (Icon-Auswahl)

## Offene Probleme / Notizen
- Erinnerung/Tastaturlicht-Text nur nach Druck, Start und (Erinnerung) Fälligkeit aktuell – außerhalb gesetzte Erinnerungen erst danach sichtbar
- Neue Daemon-Fixes erst nach `makepkg -d` im installierten Paket (Rust per rustup → lokal `makepkg -d`)
- Multi-Monitor-Actions nur mit DP-1 geprüft; Bar-Tooltip noch nicht angesehen
- Profil-Dropdown zeigte einmal alten Wert nach Laufzeit-Profilwechsel; nicht reproduzierbar
- Menüeintrag nur bei neuer Paketversion aktualisiert
- Hyprland-Zugriff nicht hinter Trait (bewusst offen, geringer Nutzen). Nachtlicht ohne Event ist bewusst so: weder hyprsunset noch Shell melden Änderungen, Polling verboten – steht in actions.md)
- `scripts/check.sh` braucht `PATH=$HOME/.cargo/bin:$PATH`; Socket-Tests brauchen kurzes `XDG_RUNTIME_DIR` (z. B. `/tmp/claude-1000/dd`)
- Lernmodus: jede Regler-Rastung erzeugt ein `slot_pressed` (Editor entprellt)
- Dev-Test der Shell mit neuer CLI: Shell nutzt `/usr/bin/duckydeck` → lokales Paket bauen (Working Tree als `duckydeck-0.1.0.tar.gz` neben PKGBUILD, `makepkg -d -f`), Nutzer installiert per `sudo pacman -U`. Version bleibt 0.1.0 → `setup` läuft nicht automatisch neu
- Dev-Loop Editor: `setup` aus Debug-Build ändert vorhandene Paket-Links nicht → Paket bauen (Tarball ohne target/.git), Nutzer installiert; bei CLI-/Katalog-Änderung auch `systemctl --user restart duckydeck`
- Daemon beenden mit `kill $(pgrep -x duckydeckd)` – `pkill -f` trifft auch die eigene Shell
- Entscheidung 2026-10-03: kein AUR, nur GitHub-Release mit fertigem Paket; Paket unsigniert, Install in zwei Schritten (Nutzer ok)
