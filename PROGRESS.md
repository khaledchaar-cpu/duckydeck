# PROGRESS

Kurz halten: Stand, nächster Schritt, offene Probleme. Am Ende jeder Sitzung aktualisieren.

**Aktueller Milestone:** alle Milestones (M1–M9) fertig, v0.1.3 auf GitHub (inkl. Lautstärke pro App)

Hardware: Stream Deck + angeschlossen (`0fd9:0084`). Lokales Paket 0.1.3-1 ohne Glow installiert (Stand `b84bab1`) (`/usr/bin/duckydeck`); Entwicklung mit `systemctl --user stop duckydeck` + `cargo run`, danach Dienst wieder starten.

## Erledigt (letzte Sitzung)
- Alle Glow-Effekte entfernt (Neon-Lichthof, Underglow, Blur-Code); HUD-Look sonst unverändert, am Gerät abgenommen
- Lokales Paket gebaut und installiert
- Nutzer hat passwortloses `sudo` eingerichtet (`/etc/sudoers.d/90-kc-nopasswd`) + `Bash(sudo *)` in `.claude/settings.local.json` → Claude installiert Pakete selbst (`sudo pacman -U --noconfirm …`)

## Nächster Schritt
- Offen; Kandidaten: Punkte unter „Offene Probleme“ oder v2-Features aus SPEC.md (nach Absprache)
- Entscheidung 2026-10-04: Editor/Panel bleiben im bisherigen Look (kein HUD-Stil)

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
- Dev-Test der Shell mit neuer CLI: Shell nutzt `/usr/bin/duckydeck` → lokales Paket bauen (`git archive` als `duckydeck-<ver>.tar.gz` neben PKGBUILD, `makepkg -d -f`), Install per `sudo pacman -U --noconfirm` (Claude darf selbst). Version bleibt 0.1.0 → `setup` läuft nicht automatisch neu
- Dev-Loop Editor: `setup` aus Debug-Build ändert vorhandene Paket-Links nicht → Paket bauen (Tarball via `git archive`), selbst installieren; bei CLI-/Katalog-Änderung auch `systemctl --user restart duckydeck`
- Daemon beenden mit `kill $(pgrep -x duckydeckd)` – `pkill -f` trifft auch die eigene Shell; Fake mit stdin per `tail -f <datei> |` – nie `pkill -x tail` (trifft parallele Builds)
- Entscheidung 2026-10-03: kein AUR, nur GitHub-Release mit fertigem Paket; Paket unsigniert, Install in zwei Schritten (Nutzer ok)
