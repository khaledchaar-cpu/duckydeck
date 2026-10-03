# PROGRESS

Kurz halten: Stand, nächster Schritt, offene Probleme. Am Ende jeder Sitzung aktualisieren.

**Aktueller Milestone:** alle Milestones (M1–M9) fertig, v0.1.2 auf GitHub; v2 „Lautstärke pro App“ fertig (unveröffentlicht)

Hardware: Stream Deck + angeschlossen (`0fd9:0084`). Lokales Paket 0.1.2 mit App-Lautstärke installiert (`/usr/bin/duckydeck`); Entwicklung mit `systemctl --user stop duckydeck` + `cargo run`, danach Dienst wieder starten.

## Erledigt (letzte Sitzung)
- Release v0.1.2 (Idee 3) auf GitHub; Idee 4 auf späteres Release zurückgestellt
- Erinnerungs-Text live über systemd-User-Manager-Signale (`UnitNew`/`UnitRemoved`)
- Regler `media.app_volume`: Drehen = Lautstärke, Drücken = Mute, gedrückt drehen = App wählen; Fokus-Modus verworfen (fragil)
- `duckydeck audio apps --json`, Editor-Auswahlfeld `app` (Kind `audio_app`), Beispielprofil Seite 2; vom Nutzer am Gerät getestet

## Nächster Schritt
- Release 0.1.3 mit Erinnerung live + App-Lautstärke (Version erhöhen, Paket, push/Release nach Nutzer-OK)

## Offene Probleme / Notizen
- Tastaturlicht-Text nur nach Druck und Start aktuell (kein Event)
- App-Lautstärke: gewählte App gilt nur bis Daemon-Neustart (bewusst, kein Zustand gespeichert)
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
