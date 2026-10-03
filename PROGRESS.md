# PROGRESS

Kurz halten: Stand, nächster Schritt, offene Probleme. Am Ende jeder Sitzung aktualisieren.

**Aktueller Milestone:** M9 – Editor (v2): M9a fertig, nächster Schritt M9b. M8 fertig bis auf AUR (zurückgestellt, Repo bleibt vorerst privat)

Hardware: Stream Deck + angeschlossen (`0fd9:0084`). Paket 0.1.0 installiert (`/usr/bin/duckydeck`); Entwicklung mit `systemctl --user stop duckydeck` + `cargo run`, danach Dienst wieder starten.

## Erledigt (letzte Sitzung)
- M9a: `launcher.app` (beliebige `.desktop`-App, wie Omarchy-Launcher) – am Gerät geprüft
- M9a: `duckydeck actions` / IPC `list_actions` (`duckydeck_core::library`: alle Actions mit Slot-Typ, Parametern, Verfügbarkeit)
- M9a: `duckydeck preview <profil> [<seite>|<ordner>]` → 8+4 PNGs mit `rev` im Namen; `export --json`
- M9a: `duckydeck edit <profil> set|clear|swap …` (`toml_edit`, Kommentare bleiben, lehnt neue `check`-Probleme ab)
- M9a: `duckydeck learn` (Lernmodus solange Verbindung offen, Events `slot_pressed`) – am Gerät geprüft

## Nächster Schritt
- M9b: Plugin `duckydeck.editor` (Overlay) anlegen – Skill `shell-plugin`; Gerätevorschau aus `preview`, Auswahl per Klick/Pfeiltasten und `learn`, Profil-/Seitenleiste

## Geplante Skills (in .claude/skills/ anlegen, wenn der Milestone fertig ist)
- `add-action` nur für Rust-Actions mit Logik – Entscheidung offen (ggf. weglassen)

## Offene Probleme / Notizen
- Neue Daemon-Fixes erst nach `makepkg -d` im installierten Paket (Rust per rustup → lokal `makepkg -d`)
- Long-Press nie wirklich ausgelöst; Multi-Monitor-Actions nur mit DP-1 geprüft; Bar-Tooltip noch nicht angesehen
- Leere Fenster (Menü offen, leerer Workspace) fallen aufs manuelle Profil zurück
- Profil-Dropdown zeigte einmal alten Wert nach Laufzeit-Profilwechsel; nicht reproduzierbar
- Menüeintrag nur bei neuer Paketversion aktualisiert; `setup --remove` nimmt das Widget nicht aus dem Bar-Layout
- Nachtlicht-Status ohne Event; Hyprland-Zugriff nicht hinter Trait
- `scripts/check.sh` braucht `PATH=$HOME/.cargo/bin:$PATH`; Socket-Tests brauchen kurzes `XDG_RUNTIME_DIR` (z. B. `/tmp/claude-1000/dd`)
- Lernmodus: jede Regler-Rastung erzeugt ein `slot_pressed` (Editor entprellt); Strip-Tippen am Gerät noch nicht gesehen
- `duckydeck … | head` → Panic „Broken pipe“ (println!), harmlos
- Daemon beenden mit `kill $(pgrep -x duckydeckd)` – `pkill -f` trifft auch die eigene Shell
- Später: Repo öffentlich, Tag `v0.1.0`, `sha256sums` + `.SRCINFO`, AUR (Account + SSH-Key)
