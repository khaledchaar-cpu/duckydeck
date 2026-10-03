# PROGRESS

Kurz halten: Stand, nächster Schritt, offene Probleme. Am Ende jeder Sitzung aktualisieren.

**Aktueller Milestone:** M6 – IPC & CLI (Teil 3a: `reload` fertig)

Hardware: Stream Deck + ist angeschlossen (`0fd9:0084`, Bus 008).

## Erledigt
- `duckydeck setup [--remove]` (`core::setup`): Plugin-Symlinks `duckydeck.*`, `omarchy bar put duckydeck.widget`, markierter Menüblock, Hook `font-set.d/duckydeck`
- Daemon führt Setup einmal pro Version aus (Marker `~/.local/state/duckydeck/setup`)
- `duckydeck reload` (IPC `reload`): Systemschrift, Theme, Config neu laden + neu zeichnen; am Gerät geprüft, auch über den Font-Hook (`omarchy font set`)
- Am echten System geprüft: Menüeintrag „Stream Deck“ erscheint und öffnet den Editor

## Nächster Schritt
- M6 Teil 3b: `duckydeck check`, `export`, `get_config`/`list_actions`

## Geplante Skills (in .claude/skills/ anlegen, wenn der Milestone fertig ist)
- nach M5a: `add-action` nur für Rust-Actions mit Logik – Entscheidung offen (ggf. weglassen)
- nach M7: `shell-plugin` (QML-Regeln, Manifest-Muster, Reload) – Details dann aus CLAUDE.md entfernen

## Offene Probleme / Notizen
- Menüeintrag öffnet vorerst die Config – bei M7 auf das Panel umstellen (`packaging/omarchy/menu.jsonc`)
- `setup --remove` nimmt das Widget nicht aus dem Bar-Layout (keine Route zum Entfernen)
- Erstanbieter-Panels sind `bar-widget`-Plugins (`entryPoints.barWidget`) – bei M7 beachten
- `scripts/check.sh` findet `cargo` nur mit `PATH=$HOME/.cargo/bin:$PATH`
- RSS ~27 MB (Debug) statt < 15 MB – laut Nutzer unkritisch; Release noch nicht gemessen
- Nicht am Gerät geprüft: Long-Press-Bestätigung, mehrseitige Profile/Swipe; Multi-Monitor-Actions (nur DP-1)
- Catalog-Actions per `spawn`: Fehler von `omarchy` werden nicht gemeldet; Nachtlicht-Status ohne Event
- Hyprland-Zugriff noch nicht hinter Trait (I/O nicht testbar)
- Socket-Pfad (SUN_LEN): Tests mit kurzem `XDG_RUNTIME_DIR` unter `/tmp/claude-1000`
- `duckydeck.service` fehlt noch (Packaging); Kontrasttest braucht `/usr/share/omarchy/themes`
- Hook ruft `duckydeck` über PATH: in der Entwicklung `~/.local/bin/duckydeck` → `target/debug/duckydeck` verlinkt
