# PROGRESS

Kurz halten: Stand, nächster Schritt, offene Probleme. Am Ende jeder Sitzung aktualisieren.

**Aktueller Milestone:** M5a – Actions: Medien (M5 abgeschlossen)

Hardware: Stream Deck + ist angeschlossen (`0fd9:0084`, Bus 008).

## Erledigt
- M5: `actions/catalog.toml` (System, Capture, Launcher) + `duckydeck_core::catalog` (Platzhalter mit `defaults`, `confirm = "long-press"`, Routenableitung)
- Katalog-Test gegen `docs/omarchy-reference.md`: Icons, Routen, Platzhalter, Aufrufe über `RecordingRunner`
- Daemon: Routenprüfung beim Start per `omarchy commands --json` (fehlend → Warn-Icon), Labels/Icons aus dem Katalog, Ausführung detached
- Am Gerät abgenommen: Terminal, Browser, Screenshot, Menü

## Nächster Schritt
- M5a: Regler Lautstärke/Mikrofon/Helligkeit (`omarchy audio …`, `omarchy brightness display ±N%`) als Rust-Actions, Status via `pactl subscribe`

## Geplante Skills (in .claude/skills/ anlegen, wenn der Milestone fertig ist)
- nach M5a: `add-action` nur für Rust-Actions mit Logik (Katalog-Einträge brauchen keinen Skill) – ggf. ganz weglassen
- nach M7: `shell-plugin` (QML-Regeln, Manifest-Muster, Reload) – Details dann aus CLAUDE.md entfernen

## Offene Probleme / Notizen
- Catalog-Actions laufen per `spawn` – Exit-Code/Fehler von `omarchy` werden nicht gemeldet; ggf. bei M5c auf `run` + Benachrichtigung umstellen
- Long-Press-Bestätigung (Power off) noch nicht am Gerät geprüft
- toml-Fehlertext ist mehrzeilig (mit Quell-Ausschnitt) – für die Benachrichtigung ggf. auf erste Zeile + „line N“ kürzen
- Seitenwechsel per Swipe (≥ 100 px) und Ordner-Navigation am Gerät noch nicht mit einem mehrseitigen Profil geprüft
- Erstanbieter-Panels sind Plugins vom Typ `bar-widget` (`entryPoints.barWidget: Panel.qml`), nicht `panel` – bei M7 berücksichtigen
- `duckydeck.service` existiert noch nicht (kommt mit Packaging); Daemon zum Testen direkt starten
- Kontrasttest liest `/usr/share/omarchy/themes` → setzt Omarchy-Build-Umgebung voraus
