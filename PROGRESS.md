# PROGRESS

Kurz halten: Stand, nächster Schritt, offene Probleme. Am Ende jeder Sitzung aktualisieren.

**Aktueller Milestone:** alle Milestones (M1–M9) fertig, v0.1.0 auf GitHub veröffentlicht

Hardware: Stream Deck + angeschlossen (`0fd9:0084`). Paket 0.1.0 installiert (`/usr/bin/duckydeck`); Entwicklung mit `systemctl --user stop duckydeck` + `cargo run`, danach Dienst wieder starten.

## Erledigt (letzte Sitzung)
- Idee 1 Lückenanalyse: 9 neue Controls (Panels, Bluetooth, Energieprofil, Audio-Ausgabe/Quelle, Touchpad, Tastaturlicht, Erinnerung, Agent); Katalogfelder `text` (Live-Status statt Label, Events via pactl/MPRIS/D-Bus), `choice_icons`, `active`
- Größere Schrift (Labels 16 px, Regler 28 px, Strip)
- Idee 2: Icon-Raster im Editor (`duckydeck icons --color`), 272 Icons inkl. `[general]` und `[adult]`
- Alles vom Nutzer am Gerät getestet (außer Tastaturlicht: kein Gerät); Build-Artefakte aus lokaler Historie entfernt, `.gitignore` ergänzt

## Nächster Schritt
- Release 0.1.1 (Version in PKGBUILD/Cargo erhöhen, damit `setup` neu läuft; push + GitHub-Release nur nach Nutzer-OK), danach Idee 3 (eigene Icons) – Plan abstimmen

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
- Daemon beenden mit `kill $(pgrep -x duckydeckd)` – `pkill -f` trifft auch die eigene Shell; Fake mit stdin per `tail -f <datei> |` – nie `pkill -x tail` (trifft parallele Builds)
- Entscheidung 2026-10-03: kein AUR, nur GitHub-Release mit fertigem Paket; Paket unsigniert, Install in zwei Schritten (Nutzer ok)
