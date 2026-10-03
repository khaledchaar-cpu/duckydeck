# PROGRESS

Kurz halten: Stand, nächster Schritt, offene Probleme. Am Ende jeder Sitzung aktualisieren.

**Aktueller Milestone:** M1 – Hardware (siehe SPEC.md)

Hardware: Stream Deck + ist angeschlossen (`0fd9:0084`, Bus 008).

## Erledigt
- M1 Teil 1 (am echten Gerät verifiziert): Verbindung, Tasten/Regler/Touch-Events, udev-Hotplug, Testmuster (Tasten + Strip), Strip-Fix nach Hotplug
- `packaging/70-duckydeck.rules`, Event-Mitschnitt `crates/duckydeckd/tests/fixtures/real-events.txt`
- Erkenntnisse in `docs/spec/architecture.md` (Abschnitt Hardware)

## Nächster Schritt
- M1 Teil 2: Fake-Device (`DUCKYDECK_FAKE_DEVICE=1`) hinter einem gemeinsamen Device-Trait; Gesamtbild `$XDG_RUNTIME_DIR/duckydeck/fake/deck.png`; Events aus der Fixture abspielen; Long-Press-Erkennung für Tasten/Regler im Daemon

## Geplante Skills (in .claude/skills/ anlegen, wenn der Milestone fertig ist)
- nach M3: `add-icon` (Tabler-Name suchen, icons.toml, fetch-icons.sh, Validierung)
- nach M5a: `add-action` nur für Rust-Actions mit Logik (Katalog-Einträge brauchen keinen Skill) – ggf. ganz weglassen
- nach M7: `shell-plugin` (QML-Regeln, Manifest-Muster, Reload) – Details dann aus CLAUDE.md entfernen

## Offene Probleme / Notizen
- Erstanbieter-Panels (Audio usw.) sind Plugins vom Typ `bar-widget` mit `entryPoints.barWidget: Panel.qml`, nicht `panel` – beim Spike/M7 berücksichtigen (ggf. `docs/spec/ui.md` anpassen)
- Shells ohne `~/.cargo/bin` im PATH: vor cargo `. ~/.cargo/env`
