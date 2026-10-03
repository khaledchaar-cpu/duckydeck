# PROGRESS

Kurz halten: Stand, nächster Schritt, offene Probleme. Am Ende jeder Sitzung aktualisieren.

**Aktueller Milestone:** M7 – Shell-Plugins fertig → nächster: M8 Kontext & Release

Hardware: Stream Deck + ist angeschlossen (`0fd9:0084`, Bus 008).

## Erledigt
- M7 fertig: `duckydeck.widget` (Bar-Icon + Panel, Tastatur j/k/h/l/Enter), Menüeintrag öffnet das Panel – vom Nutzer am Gerät geprüft, Profilwechsel mit Testprofil
- Skill `shell-plugin` angelegt (QML-Regeln, Test-Loop); CLAUDE.md verweist darauf

## Nächster Schritt
- M8: Auto-Profilwechsel nach aktivem Fenster (Hyprland-Events) – zuerst Abschnitt Kontextwechsel in `docs/spec/actions.md` lesen

## Geplante Skills (in .claude/skills/ anlegen, wenn der Milestone fertig ist)
- nach M5a: `add-action` nur für Rust-Actions mit Logik – Entscheidung offen (ggf. weglassen)

## Offene Probleme / Notizen
- Tooltip-Text beim Hover noch nicht angesehen
- Einmal zeigte das Profil-Dropdown nach Laufzeit-Profilwechsel den alten Wert (vor Shell-Neustart); nicht reproduzierbar
- Menüeintrag wird nur bei neuer Paketversion aktualisiert (Setup-Marker)
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
