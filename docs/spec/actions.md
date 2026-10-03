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
state = { command = ["omarchy", "toggle", "nightlight", "--status"], json = "enabled" }
```

- `choices = { mode = ["smart", "region"] }`: erlaubte Werte eines Platzhalters; andere Werte meldet `check`, der Editor bietet sie als Auswahl an.
- `dispatch = 'hl.dsp.window.close()'` statt `run`: Hyprland-Dispatcher als Lua-Ausdruck (Hyprland ≥ 0.55 mit Lua-Config), direkt über `.socket.sock` gesendet. Platzhalterwerte dürfen nur Buchstaben, Ziffern und `_+-:` enthalten (bleiben im Lua-String); `{ … }` ohne Bezeichner ist eine Lua-Tabelle, kein Platzhalter. `raw = ["expr"]` erlaubt für diesen Platzhalter einen ganzen Ausdruck aus dem Profil (muss mit `hl.dsp.` beginnen, einzeilig).
- `requires = "omarchy capture screenshot"`: Route, die beim Start gegen `omarchy commands --json` geprüft wird (Default: aus `run` abgeleitet).
- Parameter aus dem Profil per Platzhalter als **eigenes Argument** (`"{n}"`, auch innerhalb eines Arguments wie `"--size={n}"`), nie String-Verkettung in einer Shell. `defaults = { mode = "smart" }` liefert Werte, die das Profil nicht setzt.
- `confirm = "long-press"`: Tap tut nichts, nur Long-Press führt aus (Herunterfahren, Neustart, Abmelden). Andere Actions laufen bei Tap und Long-Press.
- Ohne passende `omarchy`-Route (z. B. Suspend: `systemctl suspend`) entfällt die Routenprüfung.
- `state` (Toggle-Status, braucht ein `{ on, off }`-Icon; unbekannt = `off`), genau eine Quelle:
  - `file = "~/…"`: per inotify beobachtet (nächstes existierendes Verzeichnis); existiert = an, mit `json = "key"` zählt ein boolescher Schlüssel darin.
  - `command = [...]`: läuft beim Start und 0,3 s/2,5 s nach jedem Druck der Action, kein Polling (Nachtlicht: Änderungen per Tastatur erst beim nächsten Druck sichtbar – weder hyprsunset noch der Shell-Dienst melden Änderungen). Ohne `json` zählt der Exit-Code.
  - `elapsed = true` (nur mit `file`): solange an, zeigt die Taste statt des Labels die Zeit seit mtime (`m:ss`); Neuzeichnen sekündlich nur dann.
- `text` (Status-Text statt Label, Entscheidung 2026-10-03; schlägt auch ein Label aus dem Profil, das Icon muss die Funktion allein tragen), genau eine Quelle: `command = [...]` (erste stdout-Zeile oder mit `json = "key"` ein String) oder `builtin = "audio-output"` (Kurzname des Standard-Ausgangs: `node.nick`, sonst Beschreibung). Gelesen beim Start, 0,3 s/2,5 s nach jedem Druck und bei den Events in `refresh = ["audio", "media"]` (`pactl subscribe` bzw. MPRIS-Player/Status-Wechsel). Leer/Fehler = Label. Weitere Quellen: `builtin = "bluetooth-device"` (verbundenes Gerät, „Name +N“), Trigger `bluetooth`/`power-profile` (D-Bus `PropertiesChanged` von BlueZ bzw. power-profiles-daemon, auch als `refresh` bei `state`). `active = "<platzhalter>"`: Text wird nicht gezeigt, die Taste ist hervorgehoben (Akzent), wenn er dem Wert des Platzhalters entspricht (aktives Energieprofil). `builtin = "reminder"`: Fälligkeit der nächsten Erinnerung („22:54 +1“, aus `omarchy reminder show --json`), neu gelesen zur Fälligkeit; `builtin = "keyboard-backlight"`: Stufe „2/3“ aus `/sys/class/leds/*kbd_backlight` (kein Event, nur nach Druck).
- Grundsatz (Nutzer 2026-10-03): Wo eine Action sinnvolle Live-Infos hat, zeigt die Taste sie – nur bei echtem Mehrwert.
- `choice_icons = { panel = { "omarchy.audio" = "volume", … } }`: Icon je Platzhalterwert (Wert aus dem Profil, sonst `defaults`), hat Vorrang vor `icon`; z. B. Shell-Panels, Fokus-Richtung.
- Ein einziger Test prüft den ganzen Katalog: Icons existieren, Routen sind gültig, Platzhalter vollständig, Aufrufe über `RecordingRunner` korrekt.

**Eigener Rust-Code nur für Actions mit Logik:** Regler (Lautstärke, Mikrofon, Helligkeit, Workspace-Scroll), MPRIS/Medien-Strip, Workspace-Status, Fokussieren-oder-Starten, App-läuft-Badge, Aufnahme-Laufzeit, Struktur-Actions (Seite, Ordner, Profil, Multi, Toggle).

Unten steht bei jeder Kategorie, was Katalog (K) und was Rust (R) ist.

## System / Omarchy (M5c) – K
- Sperren, Suspend, Neustart, Herunterfahren (`omarchy system …`; Ausschalten nur per Long-Press)
- Omarchy-Menü öffnen, Theme wechseln, Hintergrund wechseln (`omarchy theme bg next`)
- Schalter über `omarchy toggle …` (Nachtlicht, Idle-Inhibitor …)
- „Nicht stören“ / Benachrichtigungen verwerfen (Shell: `omarchy-shell notifications dismissAll`, keine `omarchy`-Route)
- Shell-Panel öffnen (`system.panel`, `omarchy-shell shell toggle <plugin-id> {}`; Audio, Bluetooth, Netzwerk, Power). WLAN an/aus gibt es bewusst nicht: keine `omarchy`-Route, nur das Netzwerk-Panel (Entscheidung 2026-10-03)
- Bluetooth an/aus mit Zustand (`omarchy bluetooth power toggle`, Zustand per Exit-Code von `… is-on`)
- Energieprofil (`omarchy powerprofiles set autodetect <profil>`, eine Taste pro Profil), Touchpad umschalten, Tastaturbeleuchtung als Taste (`omarchy brightness keyboard cycle`)
- Erinnerung (`omarchy reminder <min>`), Coding-Agent (`omarchy agent`)
- Offen (Rust, 2. Schritt): Regler Tastaturbeleuchtung, Zyklus der Energieprofile mit Anzeige des aktiven Profils
- Beliebiger Befehl (Argumentliste, keine Shell-Interpolation)

## Capture (M5c) – K, Aufnahme-Laufzeit R
- Screenshot Bereich/Fenster/Vollbild (`omarchy capture screenshot …`)
- Aufnahme starten/stoppen mit Laufzeitanzeige (`omarchy capture screenrecording …`)
- OCR, QR-Code, Farbwähler (`hyprpicker -a` wie Omarchys Super+Print, keine Route)

## Multimedia (M5a) – R, Ausgabegerät wechseln K
- **Regler:** Lautstärke (`omarchy audio output volume ±N`, Druck = Mute), Mikrofon (Druck = `omarchy audio input mute`), Helligkeit (`omarchy brightness display ±N%`), Tastaturbeleuchtung
- Status über `pactl subscribe` (Event-Stream, kein Polling)
- Play/Pause, Weiter, Zurück über MPRIS, aktiver Player wählbar
- Touchstrip: Titel, Interpret, Fortschritt (Cover optional) über die ganze Breite, solange der aktive Player spielt (1,5 s Nachlauf, weil Player beim Spulen kurz pausieren); Regler anfassen zeigt 2 s die Reglerwerte. Position wird bei Status-/Titelwechsel und `Seeked` gelesen und hochgerechnet
- Medientasten `media.play_pause|next|previous` (ausgegraut, wenn der Player `CanGoNext`/`CanGoPrevious = false` meldet), optional `args = { player = "spotify" }`; sonst der zuletzt gestartete spielende Player
- Audio-Ausgabegerät wechseln (`media.output_switch`), Medienquelle wechseln (`media.source_switch`, `omarchy audio source switch next|previous`) – beide Katalog
- v2: Lautstärke pro App

## Window Management (M5b) – Dispatches/Omarchy-Routen K, Workspace-Status und Regler R
- Workspace 1–10 wechseln / Fenster verschieben (`window.workspace`, `window.move_to_workspace`, `args = { n }`); Taste zeigt die Ziffer: aktiv = Akzent + Balken, belegt = Vordergrund, leer = gedimmt. Status aus Events `workspacev2`/`focusedmonv2`, Belegung per `j/workspaces` nur nach Fenster-/Workspace-Events
- **Regler** `window.workspace_scroll`: Workspaces durchblättern (`e±N`), Druck = Scratchpad; Strip zeigt die aktive Nummer
- Fenster schließen, Floating, Vollbild, Pseudo, Split
- Omarchy-Routen: Tiled-Fullscreen, Pop-out, Transparenz, Gaps, Layout dwindle ↔ scrolling (`omarchy hyprland …`)
- Fokus (`window.focus`), Verschieben in Richtung (`window.move`, tauscht wie Omarchys SUPER+SHIFT+Pfeil), Fenster an Monitor (`window.to_monitor`, `monitor = "+1"` oder Name), internen Monitor an/aus/spiegeln (`window.monitor_internal`/`window.monitor_mirror`, `mode = toggle|on|off`)
- Beliebiger Dispatch (`window.dispatch`, `expr = 'hl.dsp.…'`)

## Launcher (M5d) – `omarchy launch …` K, Fokussieren-oder-Starten und Badge R
- App über `.desktop` bzw. `omarchy launch …` (Browser, Terminal, Editor, Webapps, TUIs). `launcher.app` (`app = "<desktop-id>"`) startet wie der Omarchy-App-Launcher per `uwsm-app -- gtk-launch <id>.desktop`; Icon auf dem Gerät vorerst `generic-app`, Label setzt der Editor. App-Liste über `duckydeck apps [--json]` (XDG-`.desktop`-Einträge, sichtbare Apps; die Shell-`AppLibrary` bekommen nur Plugins vom Typ `menu`), Parameter-Typ `app` → Suchfeld
- Noch offen aus M5d: Fokussieren-oder-Starten, Badge bei laufender App
- Fokussieren-oder-Starten
- Omarchy-Launcher öffnen, URL öffnen
- Badge auf der Taste, wenn die App läuft

## Struktur (M4/M8) – R
Seite / Ordner / Zurück, Profil wechseln, Multi-Action (Sequenz mit Delays), Toggle-Action.
- **Regler** `structure.page_scroll` (Entscheidung Nutzer 2026-10-03): Drehen blättert die Seiten des Profils (zyklisch wie der Swipe, aus einem Ordner heraus), Druck = Seite 1; Strip zeigt „Seite/Anzahl“.

## Profile & Kontext
- Profile mit beliebig vielen Seiten (8 Tasten + 4 Regler + Strip).
- Auto-Profilwechsel über Hyprland-Event `activewindow` (Match `class`/`title`, Regex via `regex-lite`; angegebene Felder müssen alle passen). Erstes passendes Profil in Id-Reihenfolge gewinnt; ohne Match gilt das manuell gewählte Profil (`config.profile` bzw. letzte Wahl über CLI/Panel). Eine manuelle Wahl bleibt bis zum nächsten Fensterwechsel. Fokus ohne Fenster (Menü, leerer Workspace) behält das Profil des letzten Fensters. Ungültige Regex = Config-Fehler.
- Default-Profil „Omarchy“:
  - Tasten: Workspace 1–4, Terminal, Browser, Screenshot, Omarchy-Menü
  - Regler: Lautstärke, Mikrofon, Helligkeit, Workspace-Scroll
  - Strip: Reglerwerte, bei Wiedergabe Medieninfo
