# PROGRESS

Kurz halten: Stand, nächster Schritt, offene Probleme. Am Ende jeder Sitzung aktualisieren.

**Aktueller Milestone:** M6 – IPC & CLI (Teil 2: `setup` fertig)

Hardware: Stream Deck + ist angeschlossen (`0fd9:0084`, Bus 008).

## Erledigt
- IPC-Protokoll v1 (`core::ipc`, `docs/ipc.md`), Socket `$XDG_RUNTIME_DIR/duckydeck/duckydeck.sock`
- Daemon: Socket-Server, Status-Events (`device_connected/disconnected`, `profile_changed`) mit vollem Status
- CLI: `status | profile | page | brightness | subscribe | version`, `--json`
- Am Gerät abgenommen (Helligkeit, Ab-/Anstecken über `subscribe`)
- `duckydeck setup [--remove]` (`core::setup`): Plugin-Symlinks `duckydeck.*`, `omarchy bar put duckydeck.widget`, markierter Menüblock, Hook `font-set.d/duckydeck`; Daemon führt es pro Version einmal aus

## Nächster Schritt
- M6 Teil 3: `reload` (vom Font-Hook bereits aufgerufen), `check`, `export`, `get_config`/`list_actions`

## Geplante Skills (in .claude/skills/ anlegen, wenn der Milestone fertig ist)
- nach M5a: `add-action` nur für Rust-Actions mit Logik – Entscheidung offen (ggf. weglassen)
- nach M7: `shell-plugin` (QML-Regeln, Manifest-Muster, Reload) – Details dann aus CLAUDE.md entfernen

## Offene Probleme / Notizen
- Multi-Monitor-Actions (`to_monitor`, `monitor_internal`/`mirror`) nicht testbar (nur DP-1)
- `scripts/check.sh` findet `cargo` nur mit `PATH=$HOME/.cargo/bin:$PATH`
- RSS ~27 MB (Debug) statt < 15 MB – laut Nutzer unkritisch; Release-Build noch nicht gemessen
- Strip zeigt immer den automatisch gewählten Player, Tasten mit `player = …` ggf. einen anderen
- Catalog-Actions per `spawn` – Fehler von `omarchy` werden nicht gemeldet
- Nachtlicht-Status ohne Event: Änderung per Tastatur erst beim nächsten Druck sichtbar
- Aufnahme-Zeit läuft während des Nachbearbeitens (ffmpeg) nach dem Stopp kurz weiter
- Long-Press-Bestätigung (Power off) und mehrseitige Profile/Swipe noch nicht am Gerät geprüft
- Erstanbieter-Panels sind `bar-widget`-Plugins (`entryPoints.barWidget`) – bei M7 beachten
- Hyprland-Zugriff noch nicht hinter Trait (Spec architecture.md: Fake für Tests) – Logik ist in Core testbar, I/O nicht
- `profile`/`brightness` per CLI nur zur Laufzeit (Reset bei Config-Änderung) – vom Nutzer nicht beanstandet
- Socket-Pfad darf nicht zu lang sein (SUN_LEN): Tests mit kurzem `XDG_RUNTIME_DIR` unter `/tmp/claude-1000`
- `duckydeck.service` fehlt noch (Packaging); Kontrasttest braucht `/usr/share/omarchy/themes`
- Menüeintrag „Stream Deck“ öffnet vorerst die Config im Editor – bei M7 auf das Panel umstellen (`packaging/omarchy/menu.jsonc`)
- `setup --remove` nimmt das Widget nicht aus dem Bar-Layout (keine `omarchy bar`-Route zum Entfernen)
