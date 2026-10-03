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
| `learn` | – | wie `subscribe`, zusätzlich Events `slot_pressed` mit `slot: {kind: key\|dial, index (ab 1)}`; solange die Verbindung offen ist, führt das Gerät keine Actions aus (Tasten, Regler, Strip-Tippen wählen nur den Slot; Swipe blättert weiter) und wechselt kein Profil per Fenster. Endet mit der Verbindung (auch bei Absturz des Editors) |
| `preview` | `profile`, `page` (ab 1, Default 1) oder `folder` | rendert die Seite ohne das Gerät zu ändern: Status plus `preview` = `{rev, keys[8], dials[4]}` (PNG-Pfade unter `$XDG_RUNTIME_DIR/duckydeck/preview/`, `rev` im Dateinamen; ältere Bilder werden gelöscht) |
| `list_actions` | – | Status plus `actions`: alle Actions für den Editor (`library::Item`: `id`, `group`, `label`, `icon`, `slot` `key`/`dial`, `params` mit `name`/`default`/`optional`, `long_press`, `available` = Omarchy-Route vorhanden) |

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

`device_connected`, `device_disconnected`, `profile_changed` (Profil, Seite, Ordner oder Helligkeit), nur bei `learn`: `slot_pressed`. Jedes Event enthält den vollständigen Status – Clients müssen nichts zusammensetzen. Langsame Clients verlieren ältere Events (Puffer 32), das nächste Event ist trotzdem vollständig.
Noch nicht umgesetzt: `key_state`, `config_error`; erst v2 (Editor): `get_config`.

## CLI

`duckydeck check | export <profil> [--json] | edit …` arbeiten ohne Daemon direkt auf den Dateien. `duckydeck status | actions | learn | preview <profil> [<seite>|<ordner>] | reload | profile <name> | page <n> | brightness <0-100> | subscribe | version`, jeweils mit `--json` (Antwort-Zeile unverändert). `subscribe` gibt immer JSON-Zeilen aus, die erste ist die Status-Antwort. Exit-Code 1 bei Fehlern des Daemons, 2 bei falscher Bedienung.
