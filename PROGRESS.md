# PROGRESS

Kurz halten: Stand, nächster Schritt, offene Probleme. Am Ende jeder Sitzung aktualisieren.

**Aktueller Milestone:** M5b – Actions: Window Management (M5a abgeschlossen)

Hardware: Stream Deck + ist angeschlossen (`0fd9:0084`, Bus 008).

## Erledigt
- Hyprland-Socket-Modul (`duckydeck_core::hypr`, Daemon `hyprland`): Events `.socket2.sock`, Dispatch/Queries `.socket.sock`, Reconnect
- Katalog-Feld `dispatch` (Lua `hl.dsp.…`, Hyprland 0.56 mit Lua-Config), sichere Platzhalter
- Workspace-Tasten (Glyphe, aktiv/belegt/leer), Scroll-Regler (Druck = Scratchpad), Fenster-Dispatches + `omarchy hyprland window …`-Routen
- Gruppe `hyprland` in `docs/omarchy-reference.md` aufgenommen
- Am Gerät abgenommen (Tasten, Regler, Sync mit Tastatur-Wechsel)

## Nächster Schritt
- M5b abschließen: Fenster verschieben in Richtung, Fenster an Monitor, interner Monitor an/aus/spiegeln (`omarchy hyprland monitor internal …`), beliebiger Dispatch (`window.dispatch`, nur Profilwert prüfen)

## Geplante Skills (in .claude/skills/ anlegen, wenn der Milestone fertig ist)
- nach M5a: `add-action` nur für Rust-Actions mit Logik – Entscheidung offen (ggf. weglassen)
- nach M7: `shell-plugin` (QML-Regeln, Manifest-Muster, Reload) – Details dann aus CLAUDE.md entfernen

## Offene Probleme / Notizen
- RSS ~27 MB (Debug) statt < 15 MB – laut Nutzer unkritisch; Release-Build noch nicht gemessen
- Strip zeigt immer den automatisch gewählten Player, Tasten mit `player = …` ggf. einen anderen
- Catalog-Actions per `spawn` – Fehler von `omarchy` werden nicht gemeldet (ggf. M5c)
- Long-Press-Bestätigung (Power off) und mehrseitige Profile/Swipe noch nicht am Gerät geprüft
- Erstanbieter-Panels sind `bar-widget`-Plugins (`entryPoints.barWidget`) – bei M7 beachten
- Hyprland-Zugriff noch nicht hinter Trait (Spec architecture.md: Fake für Tests) – Logik ist in Core testbar, I/O nicht
- `duckydeck.service` fehlt noch (Packaging); Kontrasttest braucht `/usr/share/omarchy/themes`
