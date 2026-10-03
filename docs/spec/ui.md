# Oberfläche (Shell-Plugins)

Alle Plugins bauen ausschließlich auf Komponenten und Tokens der Omarchy-Shell auf (`/usr/share/omarchy/shell/Ui/`, `Commons/`). Keine eigenen Farben, Abstände oder Schriften. Vorbild für Struktur und Manifest: First-Party-Plugins in `/usr/share/omarchy/shell/plugins/` – benötigte APIs stehen zusammengefasst in `docs/omarchy-reference.md`.

UI-Texte in v1 nur Englisch, zentral in einer Datei pro Plugin (`Strings.qml`), damit eine zweite Sprache später in einem Durchgang ergänzt werden kann.

## v1 (M7)

- **`duckydeck.widget`** (`bar-widget`): Icon in der Bar, Zustand verbunden/getrennt, Tooltip mit aktivem Profil. Klick öffnet das Panel.
- **`duckydeck.panel`** (`panel`): Profil wählen, Seite wählen, Gerätehelligkeit, „Config öffnen“ (öffnet `~/.config/duckydeck/` im Editor via `omarchy launch editor`), „Neu laden“.
- **Menüeintrag** „Stream Deck“ im Omarchy-Menü → öffnet das Panel.
- Bearbeiten der Layouts in v1 über TOML (siehe `config.md`) mit Live-Reload; das Gerät dient als Vorschau.

## v2

- **`duckydeck.editor`** (`overlay`): Vorschau des Geräts (Bilder liefert der Daemon), Action-Bibliothek mit Suche, Inspector (Action, Parameter, Icon, Label), Drag & Drop, vollständig per Tastatur bedienbar.
