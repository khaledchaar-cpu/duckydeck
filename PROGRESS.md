# PROGRESS

Kurz halten: Stand, nächster Schritt, offene Probleme. Am Ende jeder Sitzung aktualisieren.

**Aktueller Milestone:** M5 – Action-Katalog (M4 abgeschlossen)

Hardware: Stream Deck + ist angeschlossen (`0fd9:0084`, Bus 008).

## Erledigt
- M4: `duckydeck_core::config` – Parser + Validierung für `config.toml` und Profile (Seiten, Ordner, leere Slots); Default-Profil `examples/profiles/omarchy.toml` eingebettet; Schema in `docs/spec/config.md`
- Live-Reload: `config::Store` (letzte gültige Config bleibt, Fehler per `omarchy notification send`) + Daemon-Modul `configwatch` (inotify, 200 ms Debounce)
- `duckydeck_core::nav` (Seite/Ordner/Zurück, Swipe = Seitenwechsel) + Daemon-Modul `screen` (ersetzt Demo-Bild)
- Am Gerät abgenommen: Profil-Anzeige, Live-Reload, Helligkeit, Fehler-Benachrichtigung

## Nächster Schritt
- M5: `actions/catalog.toml` + `CommandAction` laut `docs/spec/actions.md` (Label/Icon-Defaults aus dem Katalog statt aus der Action-Id ableiten)

## Geplante Skills (in .claude/skills/ anlegen, wenn der Milestone fertig ist)
- nach M5a: `add-action` nur für Rust-Actions mit Logik (Katalog-Einträge brauchen keinen Skill) – ggf. ganz weglassen
- nach M7: `shell-plugin` (QML-Regeln, Manifest-Muster, Reload) – Details dann aus CLAUDE.md entfernen

## Offene Probleme / Notizen
- toml-Fehlertext ist mehrzeilig (mit Quell-Ausschnitt) – für die Benachrichtigung ggf. auf erste Zeile + „line N“ kürzen
- Seitenwechsel per Swipe (≥ 100 px) und Ordner-Navigation am Gerät noch nicht mit einem mehrseitigen Profil geprüft
- Erstanbieter-Panels sind Plugins vom Typ `bar-widget` (`entryPoints.barWidget: Panel.qml`), nicht `panel` – bei M7 berücksichtigen
- `duckydeck.service` existiert noch nicht (kommt mit Packaging); Daemon zum Testen direkt starten
- Kontrasttest liest `/usr/share/omarchy/themes` → setzt Omarchy-Build-Umgebung voraus
