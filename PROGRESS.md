# PROGRESS

Kurz halten: Stand, nächster Schritt, offene Probleme. Am Ende jeder Sitzung aktualisieren.

**Aktueller Milestone:** M5b – Actions: Window Management (M5a abgeschlossen)

Hardware: Stream Deck + ist angeschlossen (`0fd9:0084`, Bus 008).

## Erledigt
- M5a: Regler Lautstärke/Mikrofon/Helligkeit (`duckydeck_core::dial`, Daemon-Worker `levels` fasst Drehungen zusammen, Status via `pactl subscribe`)
- MPRIS über `zbus` (`duckydeck_core::media`, Daemon `mpris`): Medientasten, Player-Auswahl, Play/Pause-Icon nach Status
- Touchstrip-Medienansicht (`Renderer::media`), 2 s Regler-Overlay, 1,5 s Nachlauf beim Spulen
- Medientasten grauen aus bei `CanGoNext`/`CanGoPrevious = false`
- Alles am Gerät abgenommen (Next/Previous mit Playlist)

## Nächster Schritt
- M5b: Hyprland-Socket-Modul (eigene Anbindung, Events `workspace`/`activewindow`), `window.workspace` + Workspace-Scroll-Regler

## Geplante Skills (in .claude/skills/ anlegen, wenn der Milestone fertig ist)
- nach M5a: `add-action` nur für Rust-Actions mit Logik – Entscheidung offen (ggf. weglassen)
- nach M7: `shell-plugin` (QML-Regeln, Manifest-Muster, Reload) – Details dann aus CLAUDE.md entfernen

## Offene Probleme / Notizen
- RSS ~27 MB (Debug) statt < 15 MB – laut Nutzer unkritisch; Release-Build noch nicht gemessen
- Strip zeigt immer den automatisch gewählten Player, Tasten mit `player = …` ggf. einen anderen
- Catalog-Actions per `spawn` – Fehler von `omarchy` werden nicht gemeldet (ggf. M5c)
- Long-Press-Bestätigung (Power off) und mehrseitige Profile/Swipe noch nicht am Gerät geprüft
- Erstanbieter-Panels sind `bar-widget`-Plugins (`entryPoints.barWidget`) – bei M7 beachten
- `duckydeck.service` fehlt noch (Packaging); Kontrasttest braucht `/usr/share/omarchy/themes`
