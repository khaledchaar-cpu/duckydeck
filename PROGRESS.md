# PROGRESS

Kurz halten: Stand, nächster Schritt, offene Probleme. Am Ende jeder Sitzung aktualisieren.

**Aktueller Milestone:** M9 – Editor (v2): M9a–M9d fertig, M9e größtenteils (Rest: Menüeintrag, Tastatur-Feinschliff). M8 fertig bis auf AUR (zurückgestellt, Repo bleibt vorerst privat)

Hardware: Stream Deck + angeschlossen (`0fd9:0084`). Paket 0.1.0 installiert (`/usr/bin/duckydeck`); Entwicklung mit `systemctl --user stop duckydeck` + `cargo run`, danach Dienst wieder starten.

## Erledigt (letzte Sitzung)
- M9e: `duckydeck edit <id> page add|remove|move`, `name`, `match`, `title`, `create <name> [<von>]`, `delete` (Standardprofil wird zurückgesetzt); `duckydeck profiles`
- Editor: „+ Page“, „+ New profile…“ im Profil-Dropdown (Id aus Name), Inspector-Reiter „Key / Dial | Profile“ mit Name, Auto-Wechsel als App-Auswahl, Advanced (Klassen-/Titel-Regex), Seite verschieben/entfernen, Löschen mit Bestätigung
- Apps liefern `wm_class` (`StartupWMClass`)
- Regler `structure.page_scroll` (Drehen blättert, Druck = Seite 1, Strip „n/m“); Spec actions/config/ui aktualisiert

## Nächster Schritt
- Neues Paket am Gerät prüfen (Profil anlegen/kopieren/löschen, Auto-Wechsel nach App und Titel, Pages-Regler); dann M9e-Rest: Menüeintrag „Stream Deck → Edit“, Tastatur-Feinschliff

## Geplante Skills (in .claude/skills/ anlegen, wenn der Milestone fertig ist)
- `add-action` nur für Rust-Actions mit Logik – Entscheidung offen (ggf. weglassen)

## Offene Probleme / Notizen
- Neue Daemon-Fixes erst nach `makepkg -d` im installierten Paket (Rust per rustup → lokal `makepkg -d`)
- Long-Press nie wirklich ausgelöst; Multi-Monitor-Actions nur mit DP-1 geprüft; Bar-Tooltip noch nicht angesehen
- Profil-Dropdown zeigte einmal alten Wert nach Laufzeit-Profilwechsel; nicht reproduzierbar
- Menüeintrag nur bei neuer Paketversion aktualisiert
- Nachtlicht-Status ohne Event; Hyprland-Zugriff nicht hinter Trait
- `scripts/check.sh` braucht `PATH=$HOME/.cargo/bin:$PATH`; Socket-Tests brauchen kurzes `XDG_RUNTIME_DIR` (z. B. `/tmp/claude-1000/dd`)
- Lernmodus: jede Regler-Rastung erzeugt ein `slot_pressed` (Editor entprellt); Strip-Tippen am Gerät noch nicht gesehen
- Dev-Test der Shell mit neuer CLI: Shell nutzt `/usr/bin/duckydeck` → lokales Paket bauen (Working Tree als `duckydeck-0.1.0.tar.gz` neben PKGBUILD, `makepkg -d -f`), Nutzer installiert per `sudo pacman -U`. Version bleibt 0.1.0 → `setup` läuft nicht automatisch neu
- „Never“ im Auto-Wechsel-Dropdown (Wert "") am Gerät noch nicht geprüft
- Dev-Loop Editor: `setup` aus Debug-Build ändert vorhandene Paket-Links nicht → Paket bauen (Tarball ohne target/.git), Nutzer installiert; bei CLI-/Katalog-Änderung auch `systemctl --user restart duckydeck`
- Daemon beenden mit `kill $(pgrep -x duckydeckd)` – `pkill -f` trifft auch die eigene Shell
- Später: Repo öffentlich, Tag `v0.1.0`, `sha256sums` + `.SRCINFO`, AUR (Account + SSH-Key)
