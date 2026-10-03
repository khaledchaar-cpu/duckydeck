# IPC – Daemon ↔ CLI (v1)

Socket: `$XDG_RUNTIME_DIR/duckydeck/duckydeck.sock` (Verzeichnis `0700`). Ein JSON-Objekt pro Zeile, jede Nachricht trägt `"v": 1`. Typen: `duckydeck-core/src/ipc.rs`.
Der Daemon bricht den Start ab, wenn schon ein Daemon auf dem Socket lauscht; eine verwaiste Socket-Datei wird ersetzt.

## Requests

| `cmd` | Felder | Wirkung |
|---|---|---|
| `status` | – | nur Status |
| `set_profile` | `profile` | Profil wechseln, Seite 1 (Laufzeit, `config.toml` bleibt unverändert) |
| `set_page` | `page` (ab 1) | Seite des aktiven Profils, schließt einen offenen Ordner |
| `set_brightness` | `brightness` (0–100) | Helligkeit bis zur nächsten Config-Änderung |
| `reload` | – | Systemschrift, Theme und Config neu laden, neu zeichnen (Font-Hook) |
| `subscribe` | – | Antwort wie `status`, danach Events bis der Client trennt |

```json
{"v":1,"cmd":"set_page","page":2}
```

## Antwort

```json
{"v":1,"ok":true,"status":{"connected":true,"serial":"A00…","profile":"omarchy","profiles":["omarchy"],"page":1,"pages":1,"brightness":60}}
{"v":1,"ok":false,"error":"unknown profile \"nope\""}
```

`status.folder` und `status.serial` fehlen, wenn kein Ordner offen bzw. kein Gerät verbunden ist.

## Events (nach `subscribe`)

```json
{"v":1,"event":"profile_changed","status":{…}}
```

`device_connected`, `device_disconnected`, `profile_changed` (Profil, Seite, Ordner oder Helligkeit). Jedes Event enthält den vollständigen Status – Clients müssen nichts zusammensetzen. Langsame Clients verlieren ältere Events (Puffer 32), das nächste Event ist trotzdem vollständig.
Noch nicht umgesetzt: `get_config`, `list_actions`, `key_state`, `config_error`.

## CLI

`duckydeck status | reload | profile <name> | page <n> | brightness <0-100> | subscribe | version`, jeweils mit `--json` (Antwort-Zeile unverändert). `subscribe` gibt immer JSON-Zeilen aus, die erste ist die Status-Antwort. Exit-Code 1 bei Fehlern des Daemons, 2 bei falscher Bedienung.
