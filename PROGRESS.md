# PROGRESS

Kurz halten: Stand, nächster Schritt, offene Probleme. Am Ende jeder Sitzung aktualisieren.

**Aktueller Milestone:** M4 – Config (M3 abgeschlossen)

Hardware: Stream Deck + ist angeschlossen (`0fd9:0084`, Bus 008).

## Erledigt
- M3: `assets/icons.toml` (81 Icons, Tabler 3.48.0 gepinnt + sha256) + `scripts/fetch-icons.sh`; eigene Icons `omarchy-menu`, `play-pause` in `assets/icons/custom/`
- `build.rs` validiert und bettet Icons ein (`icons::get`); Render- und Kontrasttest (≥ 4.5:1, alle Stock-Themes, gemessen an Pixeln)
- Galerie: `cargo run -p duckydeck-core --example gallery` → `target/gallery.png`
- Demo-Bild im Daemon nutzt echte Icons, am Gerät geprüft (sieht gut aus)
- Skill `add-icon` angelegt

- M4 (1/2): `duckydeck_core::config` – Typen + Parser + Validierung für `config.toml` und Profile (Seiten, Ordner, leere Slots), `Loaded::load(dir)`; Default-Profil `examples/profiles/omarchy.toml` eingebettet; Schema in `docs/spec/config.md`

## Nächster Schritt
- M4 (2/2): Live-Reload per inotify im Daemon (letzte gültige Config bleibt, Fehler mit Datei+Zeile), Seiten/Ordner-Navigation im Daemon (Zurück-Taste), Profil am Gerät rendern

## Geplante Skills (in .claude/skills/ anlegen, wenn der Milestone fertig ist)
- nach M5a: `add-action` nur für Rust-Actions mit Logik (Katalog-Einträge brauchen keinen Skill) – ggf. ganz weglassen
- nach M7: `shell-plugin` (QML-Regeln, Manifest-Muster, Reload) – Details dann aus CLAUDE.md entfernen

## Offene Probleme / Notizen
- Erstanbieter-Panels (Audio usw.) sind Plugins vom Typ `bar-widget` mit `entryPoints.barWidget: Panel.qml`, nicht `panel` – beim Spike/M7 berücksichtigen (ggf. `docs/spec/ui.md` anpassen)
- Drehen bei gedrücktem Regler noch nicht am Gerät geprüft (nur Unit-Test)
- `duckydeck.service` existiert noch nicht (kommt mit Packaging); Daemon zum Testen direkt starten
- Kontrasttest liest `/usr/share/omarchy/themes` → setzt Omarchy-Build-Umgebung voraus
- Shells ohne `~/.cargo/bin` im PATH: vor cargo `. ~/.cargo/env`
