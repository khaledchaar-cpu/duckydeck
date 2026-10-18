# PROGRESS

Kurz halten: Stand, nächster Schritt, offene Probleme. Am Ende jeder Sitzung aktualisieren.

**Aktueller Milestone:** alle Milestones (M1–M9) fertig, v0.1.6 vorbereitet (Stream Deck + XL, Rec+Audio); v2 vollständig

Hardware: Stream Deck + XL angeschlossen (`0fd9:00c6`), Stream Deck + (`0fd9:0084`) vorhanden, zweiter Monitor (DP-1, DP-2). Paket 0.1.5-1 installiert (0.1.6 ausstehend) (`/usr/bin/duckydeck`); Entwicklung mit `systemctl --user stop duckydeck` + `cargo run`, danach Dienst wieder starten.

## Erledigt (letzte Sitzung)
- Stream Deck + XL (`0fd9:00c6`) unterstützt: `duckydeck_core::model` (Tasten/Spalten/Regler/Strip), Profile pro Modell (`device = "plus_xl"`, Standard `plus`), eingebautes Profil `omarchy-xl`, Daemon verbindet Plus oder XL und nutzt nur passende Profile, Gesten ohne feste 8/4-Tabelle, Fake `DUCKYDECK_FAKE_DEVICE=plus_xl`, Editor-Raster aus `export --json` (`device`), `create --device`, udev-Regel für `00c6`
- Lokales Paket gebaut/installiert, echtes XL verbunden (RSS 13,7 MB)

## Nächster Schritt
- Nutzer prüft XL am Gerät (Tasten, Regler, Strip, Editor); danach Plus erneut am echten Gerät gegentesten und Release 0.1.6

## Offene Probleme / Notizen
- RSS-Puffer nur ~0,7 MB; HID-Flushes blockieren jetzt kurz auch IPC/MPRIS/pactl (unkritisch)
- Script-Zustand gilt pro Action, nicht pro Slot (bewusst, in actions.md)
- Passwortloses `sudo` (`/etc/sudoers.d/90-kc-nopasswd`) + `Bash(sudo *)` in `.claude/settings.local.json` → Claude installiert Pakete selbst (`sudo pacman -U --noconfirm …`)
- Tastaturlicht-Text nur nach Druck und Start aktuell (kein Event)
- App-Lautstärke: gewählte App gilt nur bis Daemon-Neustart (bewusst, kein Zustand gespeichert)
- Neue Daemon-Fixes erst nach `makepkg -d` im installierten Paket (Rust per rustup → lokal `makepkg -d`)
- Laptop-Actions (`monitor_internal`/`monitor_mirror`) ohne internes Display nicht testbar
- Menüeintrag nur bei neuer Paketversion aktualisiert
- Hyprland-Zugriff nicht hinter Trait (bewusst offen, geringer Nutzen). Nachtlicht ohne Event ist bewusst so: weder hyprsunset noch Shell melden Änderungen, Polling verboten – steht in actions.md)
- `scripts/check.sh` braucht `PATH=$HOME/.cargo/bin:$PATH`; Socket-Tests brauchen kurzes `XDG_RUNTIME_DIR` (z. B. `/tmp/claude-1000/dd`)
- Lernmodus: jede Regler-Rastung erzeugt ein `slot_pressed` (Editor entprellt)
- `git archive` immer aus dem Repo-Root (aus `packaging/` heraus packt es nur den Unterordner)
- Dev-Test der Shell mit neuer CLI: Shell nutzt `/usr/bin/duckydeck` → lokales Paket bauen (`git archive` als `duckydeck-<ver>.tar.gz` neben PKGBUILD, `makepkg -d -f`), Install per `sudo pacman -U --noconfirm` (Claude darf selbst). Version bleibt 0.1.0 → `setup` läuft nicht automatisch neu
- Dev-Loop Editor: `setup` aus Debug-Build ändert vorhandene Paket-Links nicht → Paket bauen (Tarball via `git archive`), selbst installieren; bei CLI-/Katalog-Änderung auch `systemctl --user restart duckydeck`
- Daemon beenden mit `kill $(pgrep -x duckydeckd)` – `pkill -f` trifft auch die eigene Shell; Fake mit stdin per `tail -f <datei> |` – nie `pkill -x tail` (trifft parallele Builds)
- Auto-Modus blockiert Änderungen an `.claude/settings*.json` auch mit Zustimmung → Nutzer per `! <befehl>` ändern lassen
- Entscheidung 2026-10-03: kein AUR, nur GitHub-Release mit fertigem Paket; Paket unsigniert, Install in zwei Schritten (Nutzer ok)
