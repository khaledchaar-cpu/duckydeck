# PROGRESS

Kurz halten: Stand, nächster Schritt, offene Probleme. Am Ende jeder Sitzung aktualisieren.

**Aktueller Milestone:** M4 – Config (Code fertig, Gerätetest offen)

Hardware: Stream Deck + ist angeschlossen (`0fd9:0084`, Bus 008).

## Erledigt
- M3: `assets/icons.toml` (81 Icons, Tabler 3.48.0 gepinnt + sha256) + `scripts/fetch-icons.sh`; eigene Icons `omarchy-menu`, `play-pause` in `assets/icons/custom/`
- `build.rs` validiert und bettet Icons ein (`icons::get`); Render- und Kontrasttest (≥ 4.5:1, alle Stock-Themes, gemessen an Pixeln)
- Galerie: `cargo run -p duckydeck-core --example gallery` → `target/gallery.png`
- Demo-Bild im Daemon nutzt echte Icons, am Gerät geprüft (sieht gut aus)
- Skill `add-icon` angelegt

- M4 (1/2): `duckydeck_core::config` – Typen + Parser + Validierung für `config.toml` und Profile (Seiten, Ordner, leere Slots), `Loaded::load(dir)`; Default-Profil `examples/profiles/omarchy.toml` eingebettet; Schema in `docs/spec/config.md`

- M4 (2/3): Live-Reload – `config::Store` (letzte gültige Config bleibt, Fehler per `omarchy notification send`), Daemon-Modul `configwatch` (inotify, Debounce); mit Fake-Device und temporärem `XDG_CONFIG_HOME` geprüft

- M4 (3/3): `duckydeck_core::nav` (Profil/Seite/Ordner, `structure.folder|back|page`, Swipe = Seite wechseln, Reconcile nach Reload); Daemon-Modul `screen` ersetzt das Demo-Bild (Label/Icon aus Binding, sonst aus Action-Id; normale Actions nur geloggt bis M5); im Fake geprüft

## Nächster Schritt
- M4 am echten Gerät abnehmen (Default-Profil, Ordner/Zurück, Swipe, Live-Reload, Helligkeit aus config.toml), dann M5: Action-Katalog

## Geplante Skills (in .claude/skills/ anlegen, wenn der Milestone fertig ist)
- nach M5a: `add-action` nur für Rust-Actions mit Logik (Katalog-Einträge brauchen keinen Skill) – ggf. ganz weglassen
- nach M7: `shell-plugin` (QML-Regeln, Manifest-Muster, Reload) – Details dann aus CLAUDE.md entfernen

## Offene Probleme / Notizen
- toml-Fehlertext ist mehrzeilig (mit Quell-Ausschnitt) – für die Benachrichtigung ggf. auf erste Zeile + „line N“ kürzen
- Erstanbieter-Panels (Audio usw.) sind Plugins vom Typ `bar-widget` mit `entryPoints.barWidget: Panel.qml`, nicht `panel` – beim Spike/M7 berücksichtigen (ggf. `docs/spec/ui.md` anpassen)
- Workspace-Taste hat noch kein Icon (kommt mit Katalog-Defaults in M5)
- Drehen bei gedrücktem Regler noch nicht am Gerät geprüft (nur Unit-Test)
- `duckydeck.service` existiert noch nicht (kommt mit Packaging); Daemon zum Testen direkt starten
- Kontrasttest liest `/usr/share/omarchy/themes` → setzt Omarchy-Build-Umgebung voraus
- Shells ohne `~/.cargo/bin` im PATH: vor cargo `. ~/.cargo/env`
