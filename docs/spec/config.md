# Konfiguration & CLI

## Dateien
- `~/.config/duckydeck/config.toml` – global (Helligkeit, Screensaver-Zeit, Default-Profil)
- `~/.config/duckydeck/profiles/<name>.toml` – ein Profil mit Seiten
- Live-Reload per inotify. Ungültige Config → letzte gültige bleibt aktiv, Fehler als Shell-Benachrichtigung mit Datei und Zeile.
- Optional: ein Omarchy-Theme kann `duckydeck.toml` mitbringen (Tasten-Hintergrund, Icon-Farben), das die aus `colors.toml` abgeleiteten Werte überschreibt.

Da Layouts in v1 von Hand bearbeitet werden, gilt: Format kurz und lesbar halten, `examples/profiles/` mit kommentierten Beispielprofilen pflegen, `duckydeck check` validiert ohne Neustart.

## Schema (M4)

`config.toml` (alle Felder optional): `brightness = 60` (0–100), `screensaver_minutes = 10` (0 = aus), `profile = "omarchy"` (Datei-Stamm unter `profiles/`).

Profil: kommentiertes Referenzbeispiel = Default-Profil [`examples/profiles/omarchy.toml`](../../examples/profiles/omarchy.toml) (eingebettet, Id `omarchy`, durch eine gleichnamige Nutzerdatei überschreibbar).
- `name`, optional `match = { class, title }` (Regex, M8), `[[pages]]` (≥ 1) mit `keys` (≤ 8) und `dials` (≤ 4)
- Slot: `{ action, args = {…}, label, icon }`; `{}` = leer. Parameter wie `step` immer in `args`.
- Ordner: `[folders.<name>]` mit ≤ 7 Tasten (letzte Taste = automatisches Zurück), geöffnet per `{ action = "structure.folder", args = { folder = "<name>" } }`.
- Unbekannte Felder sind Fehler; ob eine Action-Id existiert, prüft der Katalog (M5).

## CLI

```
duckydeck status | reload | check
duckydeck profile <name> | page <n> | brightness <0-100>
duckydeck export <profil> > profil.toml
duckydeck setup [--remove]
```
