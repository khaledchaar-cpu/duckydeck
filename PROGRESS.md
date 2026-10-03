# PROGRESS

Kurz halten: Stand, nächster Schritt, offene Probleme. Am Ende jeder Sitzung aktualisieren.

**Aktueller Milestone:** M9 – Editor (v2): M9c fertig bis auf Apps, nächster Schritt M9d. M8 fertig bis auf AUR (zurückgestellt, Repo bleibt vorerst privat)

Hardware: Stream Deck + angeschlossen (`0fd9:0084`). Paket 0.1.0 installiert (`/usr/bin/duckydeck`); Entwicklung mit `systemctl --user stop duckydeck` + `cargo run`, danach Dienst wieder starten.

## Erledigt (letzte Sitzung)
- M9b: Overlay-Plugin `duckydeck.editor` (`Editor.qml`): Profil-/Seitenleiste, Gerätevorschau aus `preview`, Auswahl per Klick, Pfeiltasten und Gerät (`learn`), schließt nur per Esc, Ordner per Enter/Doppelklick mit Breadcrumb, Inspector nur lesend – am Gerät geprüft
- Panel: Knopf „Edit layout“ (`omarchy-shell shell summon duckydeck.editor`)
- `setup` aktiviert verlinkte Overlays per `omarchy plugin enable` (sonst verweigert die Shell `summon`); PKGBUILD kopiert das Editor-Plugin

- M9c: Bibliothek (`actions --json`, gruppiert, Suche – Tippen sucht sofort, daher kein hjkl mehr), unpassende/fehlende Actions ausgegraut; Belegen per Enter/Doppelklick/Drag & Drop, Slot→Slot tauschen per Drag oder Strg+X/V, Entf leert, Undo/Redo (Strg+Z / Strg+Umschalt+Z); nach jedem Edit `duckydeck reload`, dann Vorschau – am Gerät geprüft (Suche, Enter, Drag & Drop)

- M9d Teil 1: Katalog `choices` (geprüft), Parameter-Typen in `actions --json`, `duckydeck icons [--json]`; Inspector mit Label, Icon (Suche), Parametern (Auswahl/Zahl/Text, Ordner/Profil), „Clear slot“ – sofort gespeichert, Undo – am Gerät geprüft

## Nächster Schritt
- M9d Teil 2: Multi/Toggle (Schritte/Zustände im Inspector, Verzögerung), Slot kopieren/einfügen
- Offen aus M9c: Gruppe „Apps“ (.desktop-Einträge) – `actions --json` liefert sie noch nicht

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
- Dev-Test der Shell mit neuer CLI: Shell nutzt `/usr/bin/duckydeck` → lokales Paket bauen (Working Tree als `duckydeck-0.1.0.tar.gz` neben PKGBUILD, `makepkg -d -f`), Nutzer installiert per `sudo pacman -U`. Version bleibt 0.1.0 → `setup` läuft nicht automatisch neu
- Menüeintrag „Stream Deck → Edit“ fehlt noch (M9e); externe Dateiänderung lädt den Editor noch nicht neu
- `duckydeck … | head` → Panic „Broken pipe“ (println!), harmlos
- Daemon beenden mit `kill $(pgrep -x duckydeckd)` – `pkill -f` trifft auch die eigene Shell
- Später: Repo öffentlich, Tag `v0.1.0`, `sha256sums` + `.SRCINFO`, AUR (Account + SSH-Key)
