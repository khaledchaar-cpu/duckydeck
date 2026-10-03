# CLAUDE.md – DuckyDeck

Steuerungs-App für das **Elgato Stream Deck +**, ausschließlich für **Omarchy Linux**: Rust-Daemon für das Gerät, Oberfläche als Plugin der Omarchy-Shell (Quickshell/QML).

## Wo steht was
- **Zuerst [PROGRESS.md](PROGRESS.md)** lesen: aktueller Stand und nächster Schritt.
- [SPEC.md](SPEC.md) ist nur der Index (v1/v2-Umfang, Milestones). Details stehen in `docs/spec/*.md` – **nur die Datei lesen, die zur Aufgabe passt.**
- Omarchy-Routen und Shell-APIs: **`docs/omarchy-reference.md`** (generiert von `scripts/gen-omarchy-reference.sh`).

## Harte Regeln
1. **Nur aktuelles Omarchy, nur Stream Deck +** (PID `0x0084`). Keine Abstraktionen für andere Distros, Desktops oder Geräte. Waybar, Walker, mako, SwayOSD gibt es nicht – nicht verwenden.
2. **Omarchy-Standards vor Eigenbau:** Actions rufen `omarchy <group> <action>` auf; UI ist ein Shell-Plugin mit Shell-Komponenten; Feedback über Omarchy-OSD und Shell-Benachrichtigungen; Farben nur aus `colors.toml` (Daemon) bzw. Shell-Tokens (QML).
3. **Plug & Play:** nach der Paketinstallation kein manueller Schritt.
4. **Effizienz:** Daemon < 15 MB RSS, ~0 % CPU im Leerlauf, Events statt Polling.
5. **Nie `/usr/share/omarchy/`, `/etc` oder `~/.config/hypr` verändern.** Nutzerdateien unter `~/.config/omarchy/` nur über `duckydeck setup`.
6. Keine Telemetrie, kein Netzwerk ohne explizite Nutzer-Action. Externe Prozesse nur via `tokio::process::Command` mit Argumentliste, nie über Shell-Strings.

## Stack (kurz)
Rust (Edition 2024, `tokio`) · Crates `duckydeck-core`, `duckydeckd`, `duckydeck-cli` · `elgato-streamdeck` · `tiny-skia` + `cosmic-text` + `resvg` · Hyprland über eigene Socket-Anbindung (kein `hyprland`-Crate) · MPRIS via `zbus` · Audio-Status via `pactl subscribe` · IPC: JSON-Lines über Unix-Socket, die Shell spricht nur über die CLI (`duckydeck subscribe`, `--json`) · Config: TOML · Logging: `tracing`. Keine GUI-Toolkits (GTK/Qt/Web). Neue Dependencies nur mit Begründung.

## Struktur
`crates/` · `actions/` · `shell-plugins/` (QML, je mit `manifest.json`) · `assets/` (Icons via Script, siehe `docs/spec/assets.md`) · `scripts/` · `examples/profiles/` · `packaging/` · `docs/`

## Befehle
```bash
scripts/check.sh <crate>                         # nach jeder Änderung: fmt + clippy + tests, kompakte Ausgabe
scripts/check.sh                                 # ganzer Workspace (macht der Skill session-wrapup)
cargo run -p duckydeckd -- --debug               # Daemon im Vordergrund (vorher: systemctl --user stop duckydeck)
DUCKYDECK_FAKE_DEVICE=1 cargo run -p duckydeckd  # ohne Gerät → Gesamtbild $XDG_RUNTIME_DIR/duckydeck/fake/deck.png
cargo run -p duckydeck-cli -- setup              # Plugins (Symlinks), Menü, Hooks installieren
omarchy-shell shell rescanPlugins                # Plugins neu laden
```

## Konventionen
- `clippy -D warnings` + `rustfmt`. Kein `unwrap()`/`expect()` außer in Tests und Startup; `thiserror` in Libraries, `anyhow` in Binaries.
- **Actions:** Befehl-Actions nur als Eintrag in `actions/catalog.toml` (kein neuer Rust-Code). Rust-Implementierungen des `Action`-Traits nur bei echter Logik (siehe `docs/spec/actions.md`). Jede Action hat ein Icon in `assets/icons.toml`.
- **Prozesse nur über den `CommandRunner`-Trait**; Tests nutzen `RecordingRunner` und führen nie echte `omarchy`-Befehle aus.
- Rendering deterministisch, Snapshot-Tests (`insta`).
- QML: asynchrone IPC mit Timeout; UI-Texte (v1 nur Englisch) zentral in `Strings.qml`.
- Code, Kommentare, Commits auf Englisch.

## Token-sparend arbeiten
- Ein Milestone-Schritt pro Sitzung, am Ende Skill **`session-wrapup`** (danach ist `/clear` sicher).
- Omarchy zuerst in `docs/omarchy-reference.md` nachschlagen, sonst gezielt `grep` / `omarchy <group> --help` – nie ganze Ordner oder `omarchy commands --json` komplett ausgeben.
- Icons aus Tabler über `assets/icons.toml` + `scripts/fetch-icons.sh`, nicht von Hand zeichnen.
- Bilder: nur das Fake-Device-Gesamtbild ansehen. Lange Ausgaben mit `tail`/`grep` filtern.
- Der globale Skill `omarchy` ist für die Desktop-Anpassung des Nutzers gedacht, nicht für die Entwicklung von DuckyDeck – nicht laden.

## Arbeitsweise
- Vor größeren Änderungen kurz den Plan nennen; kleine Fixes direkt umsetzen.
- Klein und oft committen (nach jedem grünen `scripts/check.sh` mit Zwischenstand). Läuft ein Ansatz fest: lieber per `git` zurückrollen und neu ansetzen als lange debuggen.
- Ein Stream Deck + ist angeschlossen: Hardware-Verhalten am echten Gerät prüfen, der Fake ersetzt das nicht.
- Milestones in der Reihenfolge aus SPEC.md; v2-Features nicht vorziehen.
- Shell-APIs und `omarchy`-Routen vor Nutzung verifizieren, nicht raten.
- Widerspricht eine Entscheidung der Spezifikation: nachfragen, danach die betroffene `docs/spec/*.md` aktualisieren.
