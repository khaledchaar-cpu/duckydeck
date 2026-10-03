# PROGRESS

Kurz halten: Stand, nächster Schritt, offene Probleme. Am Ende jeder Sitzung aktualisieren.

**Aktueller Milestone:** M9 – Editor (v2), Schritt M9a. M8 fertig bis auf AUR (zurückgestellt, Repo bleibt vorerst privat)

Hardware: Stream Deck + angeschlossen (`0fd9:0084`). Paket 0.1.0 installiert (`/usr/bin/duckydeck`); Entwicklung mit `systemctl --user stop duckydeck` + `cargo run`, danach Dienst wieder starten.

## Erledigt (letzte Sitzung)
- Daemon meldet ungültige Actions beim Laden/Reload per Shell-Benachrichtigung (`check::notify_problems`)
- Catalog-Befehle, die innerhalb von 3 s fehlschlagen → Benachrichtigung „<Label> failed“ (`CommandRunner::spawn_watched`)
- Swipe-Schwelle 100 → 40 px (Gerät meldet nur ~50–100 px)
- Am Gerät bestätigt: Auto-Profil, Toggle, Long-Press-Schutz (kurzer Druck), Fehlermeldung, Swipe
- Funktionales Design des Editors → `docs/spec/ui.md` (v2), Milestones M9a–M9e

## Nächster Schritt
- M9a: prüfen, ob es eine allgemeine `.desktop`-Launcher-Action gibt (Kategorie „Apps“); dann `duckydeck catalog --json` (Actions mit Kategorie, Slot-Typ, Parametern, Verfügbarkeit)

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
- Daemon beenden mit `kill $(pgrep -x duckydeckd)` – `pkill -f` trifft auch die eigene Shell
- Später: Repo öffentlich, Tag `v0.1.0`, `sha256sums` + `.SRCINFO`, AUR (Account + SSH-Key)
