# PROGRESS

Kurz halten: Stand, nächster Schritt, offene Probleme. Am Ende jeder Sitzung aktualisieren.

**Aktueller Milestone:** M5c – Actions: System & Capture (M5b abgeschlossen)

Hardware: Stream Deck + ist angeschlossen (`0fd9:0084`, Bus 008).

## Erledigt
- M5b fertig: `window.move` (swap in Richtung), `window.to_monitor`, `window.monitor_internal`/`window.monitor_mirror`, `window.dispatch`
- Katalogfeld `raw`: ganzer `hl.dsp.…`-Ausdruck aus dem Profil (einzeilig, Präfix geprüft)
- Fix: Änderung von `profile` in `config.toml` wechselt beim Live-Reload das Profil
- Am Gerät abgenommen (außer Multi-Monitor-Actions)

## Nächster Schritt
- M5c: Toggle-`state` (Katalogfeld, Status-Events statt Polling) und fehlende System-/Capture-Einträge laut `docs/spec/actions.md` (Bildschirmaufnahme mit Laufzeitanzeige)

## Geplante Skills (in .claude/skills/ anlegen, wenn der Milestone fertig ist)
- nach M5a: `add-action` nur für Rust-Actions mit Logik – Entscheidung offen (ggf. weglassen)
- nach M7: `shell-plugin` (QML-Regeln, Manifest-Muster, Reload) – Details dann aus CLAUDE.md entfernen

## Offene Probleme / Notizen
- Multi-Monitor-Actions (`to_monitor`, `monitor_internal`/`mirror`) nicht testbar (nur DP-1)
- `scripts/check.sh` findet `cargo` nur mit `PATH=$HOME/.cargo/bin:$PATH`
- RSS ~27 MB (Debug) statt < 15 MB – laut Nutzer unkritisch; Release-Build noch nicht gemessen
- Strip zeigt immer den automatisch gewählten Player, Tasten mit `player = …` ggf. einen anderen
- Catalog-Actions per `spawn` – Fehler von `omarchy` werden nicht gemeldet (ggf. M5c)
- Long-Press-Bestätigung (Power off) und mehrseitige Profile/Swipe noch nicht am Gerät geprüft
- Erstanbieter-Panels sind `bar-widget`-Plugins (`entryPoints.barWidget`) – bei M7 beachten
- Hyprland-Zugriff noch nicht hinter Trait (Spec architecture.md: Fake für Tests) – Logik ist in Core testbar, I/O nicht
- `duckydeck.service` fehlt noch (Packaging); Kontrasttest braucht `/usr/share/omarchy/themes`
