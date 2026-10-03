# Architektur

## Omarchy-Plattform

DuckyDeck richtet sich nach den Standards, die Omarchy selbst verwendet:

| Baustein | Technik | Bedeutung für DuckyDeck |
|---|---|---|
| **Omarchy-Shell** (`omarchy-shell`) | Quickshell/QML, ein Prozess für Bar, Menü, OSD, Benachrichtigungen, Panels | UI ist ein **Shell-Plugin**, keine eigene App |
| **Plugin-System** | `~/.config/omarchy/plugins/<id>/manifest.json`, Typen `bar-widget`, `panel`, `overlay`, `service`, Hot-Reload | Bar-Widget + Panel (v2: Editor-Overlay) |
| **Omarchy-Menü** | `~/.config/omarchy/extensions/omarchy-menu.jsonc` | Menüeintrag „Stream Deck“ |
| **`omarchy`-CLI** | `omarchy <group> <action>`, `omarchy commands --json` | Fast alle Actions rufen diese Routen auf |
| **OSD** | Plugin `omarchy.osd`, ausgelöst z. B. von `omarchy audio output volume` | Kein eigenes OSD |
| **Benachrichtigungen** | `omarchy.notifications` (freedesktop) | Meldungen via `omarchy notification send --app-name DuckyDeck` (über `CommandRunner`) |
| **Themes** | `~/.local/state/omarchy/current/theme/colors.toml` (`accent`, `background`, `foreground`, `red` …, `mode`) | Einzige Farbquelle |
| **Hooks** | `~/.config/omarchy/hooks/<event>.d/` (`theme-set`, `font-set`, `post-boot`) | Theme: kein Hook, der Daemon beobachtet `current/` per inotify (`omarchy theme set` ersetzt `theme/` per `mv`). Font: Hook `font-set.d/duckydeck` ruft `duckydeck reload` (ab M6), bis dahin wird die Schrift beim Start geladen |
| **Fonts** | `omarchy font set`, Standard JetBrainsMono Nerd Font | Label-Schrift folgt der Systemschrift |
| **Hyprland** | Lua-Config, IPC-Sockets (`.socket.sock` für Befehle, `.socket2.sock` für Events) | Window Management, Kontext-Erkennung – direkt angesprochen, ohne `hyprland`-Crate |

Konkrete Routen und Shell-Komponenten stehen in `docs/omarchy-reference.md` (generiert, nicht von Hand pflegen).

## Hardware: Stream Deck +

| Element | Details | Nutzung |
|---|---|---|
| 8 LCD-Tasten | 120×120 px, 2 × 4 | Actions mit Icon, Label, Live-Status |
| 4 Drehregler | endlos, drückbar | Lautstärke, Mikrofon, Helligkeit, Workspace-Scroll |
| Touchstrip | 800×100 px, Tap/Long-Press/Swipe | 4 Segmente à 200 px; Swipe = Seite wechseln |

USB: VID `0x0fd9`, PID `0x0084`.

## Komponenten

```
                    ┌──────── omarchy-shell (Quickshell) ────────┐
                    │    duckydeck.widget (Bar-Icon + Panel)     │
                    └─────────────────────┬──────────────────────┘
                                          │ Process: `duckydeck subscribe` (stdout)
                                          │          `duckydeck <cmd>` (Befehle)
                                   duckydeck (CLI)
                                          │ JSON-Lines, Unix-Socket
Stream Deck + ◄─ HID ─► duckydeckd (Rust) ◄┘ $XDG_RUNTIME_DIR/duckydeck.sock
                         │
                         ├─ omarchy-CLI (Actions, OSD-Feedback)
                         ├─ Hyprland-Sockets direkt (Events + Dispatch, eigenes Modul ~100 Zeilen)
                         ├─ `pactl subscribe` (Audio-Status als Event-Stream)
                         ├─ MPRIS über D-Bus
                         └─ inotify: Config, colors.toml
```

1. **`duckydeckd`** – Rust-Daemon ohne GUI, systemd-User-Service. Besitzt das Gerät, rendert Tasten/Strip, führt Actions aus, liefert Live-Status.
2. **Shell-Plugins (QML)** – Oberfläche, gerendert von der Omarchy-Shell mit deren Komponenten und Tokens. Kein eigener GUI-Prozess.
3. **`duckydeck`** – CLI für Scripts, Keybindings, Menüeinträge.

Begründung: Die Shell liefert UI, Theme, OSD und Benachrichtigungen; der Daemon macht nur, was QML schlecht kann (HID, Bildrendering, sparsame Eventverarbeitung). Das Gerät läuft weiter, wenn die Shell neu startet.

## IPC

Daemon ↔ CLI: JSON-Lines über Unix-Socket, versioniert mit `"v": 1`, Schema in [`docs/ipc.md`](../ipc.md).

Shell ↔ Daemon **über die CLI**, nicht direkt: Das QML-Plugin startet `duckydeck subscribe` als Quickshell-`Process` und liest Events als JSON-Zeilen von stdout; Befehle laufen als `duckydeck <cmd> --json`. Vorteile: kein Socket-Code in QML, die CLI wird automatisch mitgetestet, Protokolländerungen betreffen nur Rust. Spike M0.5 (2026-10-03, `shell-plugins/spike/`) hat das bestätigt:

- Plugin vom Typ `service` mit `keepLoaded: true`, Entry `Service.qml` (Root-`Item` mit `property var shell`).
- `Process { stdout: SplitParser { onRead: (line) => JSON.parse(line) } }` liefert jede Zeile sofort (keine Pufferung bei `printf` + Zeilenende); ungültige Zeilen per `try/catch` verwerfen.
- Neustart: in `onExited` einen einmaligen `Timer` starten, der `running = true` setzt; Backoff 0,5 s → max. 10 s, Reset nach dem ersten gültigen Event. Lief stabil über viele Zyklen.
- Plugin deaktivieren beendet den Kindprozess, keine Waisen.
- `console.log` landet im Journal: `journalctl --user | grep omarchy-shell` (Präfix `[duckydeck…]`).
- Plugin-Pfad im QML: `Qt.resolvedUrl("datei")`; die echte CLI wird über `PATH` (`duckydeck`) gestartet.
- `omarchy plugin enable` kennt ein neues Plugin erst nach `omarchy-shell shell rescanPlugins`.

- Requests: `status`, `set_profile`, `set_page`, `set_brightness`, `reload`, `subscribe` (v2 für den Editor: `get_config`, `list_actions` – Format wird dann festgelegt)
- Events: `device_connected`, `device_disconnected`, `profile_changed`, `key_state`, `config_error`


## Prozessaufrufe & Tests

Alle externen Prozesse (omarchy, pactl, notify-send, Apps) laufen über einen `CommandRunner`-Trait in `duckydeck-core`: Ausnahme: der Dauer-Stream `pactl subscribe` läuft direkt über `tokio::process::Command` (Argumentliste), weil der Trait nur beendete Befehle kennt.
- Produktion: `TokioRunner` (`tokio::process::Command`, Argumentliste, Timeout, kein Shell-String).
- Tests: `RecordingRunner` zeichnet nur auf, was aufgerufen worden wäre. Kein Test führt echte `omarchy`-Befehle aus.

Genauso wird der Hyprland-Zugriff hinter einem Trait gekapselt (Fake mit vorgegebenen Events für Tests).

## Plug & Play

1. Das AUR-Paket (`omarchy pkg aur add duckydeck`) liefert:
   - `/usr/lib/udev/rules.d/70-duckydeck.rules` (`uaccess`, kein root)
   - `/usr/lib/systemd/user/duckydeck.service` (`WantedBy=graphical-session.target`, Preset aktiviert)
   - Shell-Plugins, Menüerweiterung und Hook-Scripts unter `/usr/share/duckydeck/`
2. `duckydeck setup` (beim ersten Daemon-Start automatisch, idempotent):
   - Plugins nach `~/.config/omarchy/plugins/duckydeck.*` verlinken, Widget per `omarchy bar put duckydeck.widget` einhängen
   - Menüeintrag in `omarchy-menu.jsonc` (eigener, markierter Block)
   - Hook `font-set.d/duckydeck` per `omarchy hook install` (Theme-Wechsel erkennt der Daemon selbst)
   - `duckydeck setup --remove` macht alles rückgängig
   - Quellen: Plugins aus `$DUCKYDECK_SHARE_DIR/shell-plugins` bzw. `/usr/share/duckydeck/shell-plugins` (Debug-Build ohne Installation: Repo); Menüblock und Hook sind ins Binary eingebettet (`packaging/omarchy/`)
   - Marker `~/.local/state/duckydeck/setup` (Version bzw. `removed`): der Daemon führt das Setup beim Start nur bei neuer Version aus, ein `--remove` bleibt bestehen
3. Einstecken → Hotplug (auch nach Suspend/Resume) → Default-Profil, Shell-Benachrichtigung „Stream Deck + connected“. Ziel: < 1 s.

## Nicht-funktionale Anforderungen

- Daemon: < 15 MB RSS, ~0 % CPU im Leerlauf, Taste → Aktion < 50 ms.
- Shell-Plugins blockieren nie die Shell (asynchron, Timeouts); Panel öffnet < 100 ms.
- `Restart=on-failure`; Shell-Neustarts und Omarchy-Updates brechen das Gerät nicht.
- Kein root, keine Telemetrie, kein Netzwerk ohne explizite Nutzer-Action.
- Nie `/usr/share/omarchy/` verändern; Nutzerdateien nur über `duckydeck setup`.
- Lizenz MIT (Icons: Tabler Icons, MIT).

## Offene Fragen

- Plugin-API der Shell ist jung: Kompatibilität über `schemaVersion` der Manifeste prüfen?
- Upstream-Beitrag als offizielles Omarchy-Plugin?

## Hardware-Erkenntnisse (M1, fw 2.0.3.7)

- `elgato-streamdeck` (sync API) mit zwei HID-Handles: eines für Ausgabe, eines im Lese-Thread. `read(None)` ist in der Crate **nicht-blockierend** (Busy-Loop) → immer `read(Some(lange))`, blockiert in `poll(2)`.
- Der Lese-Thread startet erst **nach** dem ersten vollständigen Zeichnen; parallel startende Reads lassen das Gerät das Strip-Bild verwerfen.
- Nach dem Einstecken löscht die bootende Firmware den Strip einmal (1,0–1,5 s nach dem udev-`add`); Tasten sind nicht betroffen → Strip 2 s nach Hotplug neu zeichnen. Kein `reset()` vor dem Zeichnen.
- Gesten (Tap, Long-Press, Swipe auf dem Strip) liefert die Firmware selbst; Encoder-Twist mit Schrittweite (±1, ±2). Long-Press für Tasten/Regler misst der Daemon selbst (`gesture.rs`, 500 ms, feuert noch während des Haltens).
- Hotplug über udev-Monitor (`udev`-Crate, Subsystem `hidraw`) in einem eigenen Thread (Monitor ist nicht `Sync`).
- Leerlauf gemessen: 0 CPU-Ticks/10 s, ~7,6 MB RSS.
- Zugriffsrechte: ohne eigene Regel nur dank fremder Regel (`/etc/udev/rules.d/50-companion-desktop.rules`) → `packaging/70-duckydeck.rules` (uaccess) ist Pflicht.
