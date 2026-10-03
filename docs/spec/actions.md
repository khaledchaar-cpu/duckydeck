# Actions & Profile

Grundsatz: **Gibt es eine `omarchy`-Route, wird sie verwendet** – das bringt OSD, Benachrichtigungen und Omarchy-Verhalten automatisch mit. Routen stehen in `docs/omarchy-reference.md`. Beim Start prüft der Daemon per `omarchy commands --json`, welche Routen existieren; fehlende Actions werden deaktiviert (Warn-Icon) statt zu crashen.

Jede Action braucht ein Icon (siehe `assets.md`).

## Katalog statt Einzel-Implementierungen

Die meisten Actions sind „Befehl ausführen + Icon zeigen“. Sie werden **deklarativ** in `actions/catalog.toml` beschrieben und von einem generischen `CommandAction` ausgeführt:

```toml
[system.lock]
label = "Lock"
icon  = "lock"
run   = ["omarchy", "system", "lock"]

[system.power]
label   = "Power off"
icon    = "power"
run     = ["omarchy", "system", "shutdown"]
confirm = "long-press"

[toggle.nightlight]
label = "Night light"
icon  = { on = "nightlight", off = "nightlight-off" }
run   = ["omarchy", "toggle", "nightlight"]
state = { kind = "command", check = ["…"] }   # optional, nur wenn es ein Event/Status gibt
```

- `requires = "omarchy capture screenshot"`: Route, die beim Start gegen `omarchy commands --json` geprüft wird (Default: aus `run` abgeleitet).
- Parameter aus dem Profil per Platzhalter als **eigenes Argument** (`"{n}"`), nie String-Verkettung in einer Shell.
- Ein einziger Test prüft den ganzen Katalog: Icons existieren, Routen sind gültig, Platzhalter vollständig, Aufrufe über `RecordingRunner` korrekt.

**Eigener Rust-Code nur für Actions mit Logik:** Regler (Lautstärke, Mikrofon, Helligkeit, Workspace-Scroll), MPRIS/Medien-Strip, Workspace-Status, Fokussieren-oder-Starten, App-läuft-Badge, Aufnahme-Laufzeit, Struktur-Actions (Seite, Ordner, Profil, Multi, Toggle).

Unten steht bei jeder Kategorie, was Katalog (K) und was Rust (R) ist.

## System / Omarchy (M5c) – K
- Sperren, Suspend, Neustart, Herunterfahren (`omarchy system …`; Ausschalten nur per Long-Press)
- Omarchy-Menü öffnen, Theme wechseln, Hintergrund wechseln (`omarchy theme bg next`)
- Schalter über `omarchy toggle …` (Nachtlicht, Idle-Inhibitor …)
- „Nicht stören“ / Benachrichtigungen verwerfen (Shell)
- WLAN-/Bluetooth-/Audio-Panel der Shell öffnen; `omarchy bluetooth power toggle`
- Erinnerung (`omarchy reminder <min>`), Coding-Agent (`omarchy agent`)
- Beliebiger Befehl (Argumentliste, keine Shell-Interpolation)

## Capture (M5c) – K, Aufnahme-Laufzeit R
- Screenshot Bereich/Fenster/Vollbild (`omarchy capture screenshot …`)
- Aufnahme starten/stoppen mit Laufzeitanzeige (`omarchy capture screenrecording …`)
- OCR, QR-Code, Farbwähler

## Multimedia (M5a) – R, Ausgabegerät wechseln K
- **Regler:** Lautstärke (`omarchy audio output volume ±N`, Druck = Mute), Mikrofon (Druck = `omarchy audio input mute`), Helligkeit (`omarchy brightness display ±N%`), Tastaturbeleuchtung
- Status über `pactl subscribe` (Event-Stream, kein Polling)
- Play/Pause, Weiter, Zurück über MPRIS, aktiver Player wählbar
- Touchstrip: Titel, Interpret, Fortschritt (Cover optional)
- Audio-Ausgabegerät wechseln
- v2: Lautstärke pro App

## Window Management (M5b) – Dispatches/Omarchy-Routen K, Workspace-Status und Regler R
- Workspace 1–10 wechseln / Fenster verschieben, aktiver Workspace markiert
- **Regler:** Workspaces durchblättern, Druck = Special-Workspace
- Fenster schließen, Floating, Vollbild, Pseudo, Split
- Omarchy-Routen: Tiled-Fullscreen, Pop-out, Transparenz, Gaps, Layout dwindle ↔ scrolling (`omarchy hyprland …`)
- Fokus/Verschieben in Richtung, Fenster an Monitor, internen Monitor an/aus/spiegeln
- Beliebiger `hyprctl dispatch`

## Launcher (M5d) – `omarchy launch …` K, Fokussieren-oder-Starten und Badge R
- App über `.desktop` bzw. `omarchy launch …` (Browser, Terminal, Editor, Webapps, TUIs)
- Fokussieren-oder-Starten
- Omarchy-Launcher öffnen, URL öffnen
- Badge auf der Taste, wenn die App läuft

## Struktur (M4/M8) – R
Seite / Ordner / Zurück, Profil wechseln, Multi-Action (Sequenz mit Delays), Toggle-Action.

## Profile & Kontext
- Profile mit beliebig vielen Seiten (8 Tasten + 4 Regler + Strip).
- Auto-Profilwechsel über Hyprland-Event `activewindow` (Match `class`/`title`, Regex) – M8.
- Default-Profil „Omarchy“:
  - Tasten: Workspace 1–4, Terminal, Browser, Screenshot, Omarchy-Menü
  - Regler: Lautstärke, Mikrofon, Helligkeit, Workspace-Scroll
  - Strip: Reglerwerte, bei Wiedergabe Medieninfo
