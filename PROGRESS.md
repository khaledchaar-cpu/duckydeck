# PROGRESS

Kurz halten: Stand, nächster Schritt, offene Probleme. Am Ende jeder Sitzung aktualisieren.

**Aktueller Milestone:** M2 – Rendering & Theme (siehe SPEC.md)

Hardware: Stream Deck + ist angeschlossen (`0fd9:0084`, Bus 008).

## Erledigt
- M1 abgeschlossen. Teil 2: `Surface`-Trait (echtes Gerät / `FakeSurface` → `$XDG_RUNTIME_DIR/duckydeck/fake/deck.png`), eigenes `Input`-Enum mit Fixture-Parser
- Fake-Input: `DUCKYDECK_FAKE_DEVICE=1`, Events im Fixture-Format über stdin (`cat fixture | duckydeckd`)
- `gesture.rs`: Tap/Long-Press (500 ms, feuert noch während des Haltens) für Tasten und Regler, Drehen bei gedrücktem Regler hebt Tap/Long-Press auf; Wartezeit über `sleep_until(deadline)`, kein Polling
- Am Gerät verifiziert: Tap/Long-Press bei Tasten und Reglern, Strip-Tap/Long-Press/Swipe (mit sichtbarem Feedback im Testmuster); Drehen mit Schrittweite bis ±3

## Nächster Schritt
- M2 Teil 1: Tasten-Renderer (`tiny-skia` + `cosmic-text` + `resvg`) und `colors.toml`-Parser in `duckydeck-core`, Snapshot-Tests (`insta`); das Testmuster ersetzen

## Geplante Skills (in .claude/skills/ anlegen, wenn der Milestone fertig ist)
- nach M3: `add-icon` (Tabler-Name suchen, icons.toml, fetch-icons.sh, Validierung)
- nach M5a: `add-action` nur für Rust-Actions mit Logik (Katalog-Einträge brauchen keinen Skill) – ggf. ganz weglassen
- nach M7: `shell-plugin` (QML-Regeln, Manifest-Muster, Reload) – Details dann aus CLAUDE.md entfernen

## Offene Probleme / Notizen
- Erstanbieter-Panels (Audio usw.) sind Plugins vom Typ `bar-widget` mit `entryPoints.barWidget: Panel.qml`, nicht `panel` – beim Spike/M7 berücksichtigen (ggf. `docs/spec/ui.md` anpassen)
- Drehen bei gedrücktem Regler noch nicht am Gerät geprüft (nur Unit-Test)
- Shells ohne `~/.cargo/bin` im PATH: vor cargo `. ~/.cargo/env`
