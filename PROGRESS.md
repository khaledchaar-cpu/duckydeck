# PROGRESS

Kurz halten: Stand, nächster Schritt, offene Probleme. Am Ende jeder Sitzung aktualisieren.

**Aktueller Milestone:** M9 – Editor (v2): M9a–M9d fertig, M9e größtenteils (Rest: Menüeintrag, Tastatur-Feinschliff). M8 fertig bis auf AUR (zurückgestellt, Repo bleibt vorerst privat)

Hardware: Stream Deck + angeschlossen (`0fd9:0084`). Paket 0.1.0 installiert (`/usr/bin/duckydeck`); Entwicklung mit `systemctl --user stop duckydeck` + `cargo run`, danach Dienst wieder starten.

## Erledigt (letzte Sitzung)
- Offene Punkte: letzter Multi-Schritt nicht löschbar, `| head` ohne Panic, `setup --remove` nimmt Widget aus der Bar (`omarchy plugin disable`)
- Event `config_changed`; Editor lädt bei externen Änderungen neu
- Undo für Seite entfernen / Profil löschen (`duckydeck edit <id> restore <toml>`)
- App-Icons aus dem Icon-Theme auf `launcher.app`-Tasten (PNG farbig)
- Fokus ohne Fenster behält das Auto-Profil; am Gerät geprüft: Long-Press, Strip-Gesten, Profil löschen + Undo

## Nächster Schritt
- M9e-Rest: Menüeintrag „Stream Deck → Edit“, Tastatur-Feinschliff im Editor

## Geplante Skills (in .claude/skills/ anlegen, wenn der Milestone fertig ist)
- `add-action` nur für Rust-Actions mit Logik – Entscheidung offen (ggf. weglassen)

## Offene Probleme / Notizen
- Neue Daemon-Fixes erst nach `makepkg -d` im installierten Paket (Rust per rustup → lokal `makepkg -d`)
- Multi-Monitor-Actions nur mit DP-1 geprüft; Bar-Tooltip noch nicht angesehen
- Noch ungeprüft: Strip-Tippen im Editor, Strip-Wischen mit mehrseitigem Profil; Label unter App-Tasten zeigt „App“ statt App-Name
- Profil-Dropdown zeigte einmal alten Wert nach Laufzeit-Profilwechsel; nicht reproduzierbar
- Menüeintrag nur bei neuer Paketversion aktualisiert
- Hyprland-Zugriff nicht hinter Trait (bewusst offen, geringer Nutzen). Nachtlicht ohne Event ist bewusst so: weder hyprsunset noch Shell melden Änderungen, Polling verboten – steht in actions.md)
- `scripts/check.sh` braucht `PATH=$HOME/.cargo/bin:$PATH`; Socket-Tests brauchen kurzes `XDG_RUNTIME_DIR` (z. B. `/tmp/claude-1000/dd`)
- Lernmodus: jede Regler-Rastung erzeugt ein `slot_pressed` (Editor entprellt)
- Dev-Test der Shell mit neuer CLI: Shell nutzt `/usr/bin/duckydeck` → lokales Paket bauen (Working Tree als `duckydeck-0.1.0.tar.gz` neben PKGBUILD, `makepkg -d -f`), Nutzer installiert per `sudo pacman -U`. Version bleibt 0.1.0 → `setup` läuft nicht automatisch neu
- „Never“ im Auto-Wechsel-Dropdown (Wert "") am Gerät noch nicht geprüft
- Dev-Loop Editor: `setup` aus Debug-Build ändert vorhandene Paket-Links nicht → Paket bauen (Tarball ohne target/.git), Nutzer installiert; bei CLI-/Katalog-Änderung auch `systemctl --user restart duckydeck`
- Daemon beenden mit `kill $(pgrep -x duckydeckd)` – `pkill -f` trifft auch die eigene Shell
- Später: Repo öffentlich, Tag `v0.1.0`, `sha256sums` + `.SRCINFO`, AUR (Account + SSH-Key)
