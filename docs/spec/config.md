# Konfiguration & CLI

## Dateien
- `~/.config/duckydeck/config.toml` – global (Helligkeit, Screensaver-Zeit, Default-Profil)
- `~/.config/duckydeck/profiles/<name>.toml` – ein Profil mit Seiten
- Live-Reload per inotify (Config-Ordner, `profiles/`, bei fehlendem Ordner das nächste existierende Elternverzeichnis; 200 ms Debounce, nur `*.toml`). Ungültige Config → letzte gültige bleibt aktiv, Fehler als Shell-Benachrichtigung mit Datei und Zeile.
- Optional: ein Omarchy-Theme kann `duckydeck.toml` mitbringen (Tasten-Hintergrund, Icon-Farben), das die aus `colors.toml` abgeleiteten Werte überschreibt.

Da Layouts in v1 von Hand bearbeitet werden, gilt: Format kurz und lesbar halten, `examples/profiles/` mit kommentierten Beispielprofilen pflegen, `duckydeck check` validiert ohne Neustart.

## Schema (M4)

`config.toml` (alle Felder optional): `brightness = 60` (0–100), `screensaver_minutes = 10` (0 = aus), `profile = "omarchy"` (Datei-Stamm unter `profiles/`).

Profil: kommentiertes Referenzbeispiel = Default-Profil [`examples/profiles/omarchy.toml`](../../examples/profiles/omarchy.toml) (eingebettet, Id `omarchy`, durch eine gleichnamige Nutzerdatei überschreibbar).
- `name`, optional `match = { class, title }` (Regex, siehe Kontextwechsel in `actions.md`), `[[pages]]` (≥ 1) mit `keys` (≤ 8) und `dials` (≤ 4)
- Slot: `{ action, args = {…}, label, icon }`; `{}` = leer. Parameter wie `step` immer in `args`.
- Ordner: `[folders.<name>]` mit ≤ 7 Tasten (letzte Taste = automatisches Zurück), geöffnet per `{ action = "structure.folder", args = { folder = "<name>" } }`.
- Seiten: `{ action = "structure.page", args = { n = 2 } }` bzw. `args = { to = "next" | "prev" }`; Swipe auf dem Touchstrip (≥ 100 px) wechselt ebenfalls die Seite (nach links = nächste, zyklisch).
- Profil: `{ action = "structure.profile", args = { profile = "<id>" } }` (zählt als manuelle Wahl, siehe Kontextwechsel).
- Multi: `{ action = "structure.multi", args = { steps = [{ action = "…", args = {…} }, { delay_ms = 300 }, …] } }` – Schritte nacheinander, `delay_ms` 0–10000. Ein langer Druck gilt für alle Schritte (Long-Press-Bestätigung).
- Toggle: `{ action = "structure.toggle", args = { states = [{ action, args, label, icon }, { … }] } }` – genau 2 Zustände, abwechselnd ausgeführt; die Taste zeigt den Zustand, den der nächste Druck ausführt. Zustand nur im Speicher (Reset bei Config-Reload/Neustart).
- In Multi/Toggle sind keine Struktur-Actions erlaubt außer `structure.profile`. `duckydeck check` prüft verschachtelte Actions und Profil-Ids.
- Ohne `label` zeigt eine Ordner-Taste den Ordnernamen, andere Struktur-Tasten nur ihr Icon.
- Unbekannte Felder sind Fehler; ob eine Action-Id existiert, prüft der Katalog (M5).

## CLI

```
duckydeck status | reload | check
duckydeck profile <name> | page <n> | brightness <0-100>
duckydeck export <profil> > profil.toml
duckydeck setup [--remove]
duckydeck actions | preview <profil> [<seite>|<ordner>]     # Editor (v2), über den Daemon
duckydeck edit <profil> set <seite|ordner> key|dial <n> '<json>'
duckydeck edit <profil> clear <seite|ordner> key|dial <n>
duckydeck edit <profil> swap <seite|ordner> key|dial <n> <seite|ordner> <n>
```

`edit` arbeitet ohne Daemon direkt auf `profiles/<id>.toml` (`toml_edit`: Kommentare/Format bleiben), schreibt atomar und lehnt Änderungen ab, die neue `check`-Probleme erzeugen. Das Standardprofil wird beim ersten Edit nach `profiles/omarchy.toml` kopiert. Leere Slots am Ende eines Arrays werden entfernt.
