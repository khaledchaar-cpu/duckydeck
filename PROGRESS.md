# PROGRESS

Kurz halten: Stand, nächster Schritt, offene Probleme. Am Ende jeder Sitzung aktualisieren.

**Aktueller Milestone:** M7 – Shell-Plugins (Bar-Icon fertig, Panel offen)

Hardware: Stream Deck + ist angeschlossen (`0fd9:0084`, Bus 008).

## Erledigt
- Entscheidung: ein Plugin `duckydeck.widget` (Bar-Icon + Panel) wie Erstanbieter-Panels; Spec angepasst
- Bar-Icon (`shell-plugins/duckydeck.widget`): gedimmt ohne Gerät/Daemon, Tooltip mit Profil/Seite, reconnect per Backoff – im Bar geprüft
- `setup` ruft nach dem Verlinken `omarchy-shell shell rescanPlugins` vor `bar put` (sonst „not a known widget“)

## Nächster Schritt
- M7: Panel (`KeyboardPanel` in `Panel.qml`): Profil, Seite, Helligkeit, „Config öffnen“, „Neu laden“; Menüeintrag auf `omarchy-shell duckydeck.widget toggle` umstellen

## Geplante Skills (in .claude/skills/ anlegen, wenn der Milestone fertig ist)
- nach M5a: `add-action` nur für Rust-Actions mit Logik – Entscheidung offen (ggf. weglassen)
- nach M7: `shell-plugin` (QML-Regeln, Manifest-Muster, Reload) – Details dann aus CLAUDE.md entfernen

## Offene Probleme / Notizen
- Tooltip-Text beim Hover noch nicht angesehen
- `setup --remove` nimmt das Widget nicht aus dem Bar-Layout (keine Route zum Entfernen)
- `scripts/check.sh` findet `cargo` nur mit `PATH=$HOME/.cargo/bin:$PATH`
- RSS ~27 MB (Debug) statt < 15 MB – laut Nutzer unkritisch; Release noch nicht gemessen
- Nicht am Gerät geprüft: Long-Press-Bestätigung, mehrseitige Profile/Swipe; Multi-Monitor-Actions (nur DP-1)
- Catalog-Actions per `spawn`: Fehler von `omarchy` werden nicht gemeldet; Nachtlicht-Status ohne Event
- Hyprland-Zugriff noch nicht hinter Trait (I/O nicht testbar)
- Socket-Pfad (SUN_LEN): Tests mit kurzem `XDG_RUNTIME_DIR` unter `/tmp/claude-1000`
- `duckydeck.service` fehlt noch (Packaging); Kontrasttest braucht `/usr/share/omarchy/themes`
- Hook ruft `duckydeck` über PATH: in der Entwicklung `~/.local/bin/duckydeck` → `target/debug/duckydeck` verlinkt
- `duckydeck check` meldet Fehler ohne Shell-Benachrichtigung; der Daemon prüft Action-Ids beim Laden noch nicht (unbekannte → Log „not implemented yet“)
