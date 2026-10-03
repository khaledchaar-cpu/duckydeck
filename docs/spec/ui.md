# Oberfläche (Shell-Plugins)

Alle Plugins bauen ausschließlich auf Komponenten und Tokens der Omarchy-Shell auf (`/usr/share/omarchy/shell/Ui/`, `Commons/`). Keine eigenen Farben, Abstände oder Schriften. Vorbild für Struktur und Manifest: First-Party-Plugins in `/usr/share/omarchy/shell/plugins/` – benötigte APIs stehen zusammengefasst in `docs/omarchy-reference.md`.

UI-Texte in v1 nur Englisch, zentral in einer Datei pro Plugin (`Strings.qml`), damit eine zweite Sprache später in einem Durchgang ergänzt werden kann.

## v1 (M7)

Ein einziges Plugin wie die Erstanbieter-Panels (Entscheidung Nutzer 2026-10-03): **`duckydeck.widget`** (`bar-widget`, Entry `Panel.qml` auf Basis von `Ui/Panel`), IPC-Target `duckydeck.widget` (`open|close|toggle`).
- **Bar-Icon:** Zustand verbunden/getrennt (gedimmt), Tooltip mit aktivem Profil. Klick öffnet das Panel. Status per `duckydeck subscribe` (`Service.qml`, Neustart mit Backoff).
- **Panel** (`KeyboardPanel` am Icon): Profil wählen, Seite wählen, Gerätehelligkeit, „Config öffnen“ (öffnet `~/.config/duckydeck/` im Editor via `omarchy launch editor`), „Neu laden“.
- **Menüeintrag** „Stream Deck“ im Omarchy-Menü → `omarchy-shell duckydeck.widget toggle`.
- Bearbeiten der Layouts in v1 über TOML (siehe `config.md`) mit Live-Reload; das Gerät dient als Vorschau.

## v2

- **`duckydeck.editor`** (`overlay`): Vorschau des Geräts (Bilder liefert der Daemon), Action-Bibliothek mit Suche, Inspector (Action, Parameter, Icon, Label), Drag & Drop, vollständig per Tastatur bedienbar.
