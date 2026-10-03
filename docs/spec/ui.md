# Oberfläche (Shell-Plugins)

Alle Plugins bauen ausschließlich auf Komponenten und Tokens der Omarchy-Shell auf (`/usr/share/omarchy/shell/Ui/`, `Commons/`). Keine eigenen Farben, Abstände oder Schriften. Vorbild für Struktur und Manifest: First-Party-Plugins in `/usr/share/omarchy/shell/plugins/` – benötigte APIs stehen zusammengefasst in `docs/omarchy-reference.md`.

UI-Texte in v1 nur Englisch, zentral in einer Datei pro Plugin (`Strings.qml`), damit eine zweite Sprache später in einem Durchgang ergänzt werden kann.

## v1 (M7)

Ein einziges Plugin wie die Erstanbieter-Panels (Entscheidung Nutzer 2026-10-03): **`duckydeck.widget`** (`bar-widget`, Entry `Panel.qml` auf Basis von `Ui/Panel`), IPC-Target `duckydeck.widget` (`open|close|toggle`).
- **Bar-Icon:** Zustand verbunden/getrennt (gedimmt), Tooltip mit aktivem Profil. Klick öffnet das Panel. Status per `duckydeck subscribe` (`Service.qml`, Neustart mit Backoff).
- **Panel** (`KeyboardPanel` am Icon): Profil wählen, Seite wählen, Gerätehelligkeit, „Config öffnen“ (öffnet `~/.config/duckydeck/` im Editor via `omarchy launch editor`), „Neu laden“.
- **Menüeintrag** „Stream Deck“ im Omarchy-Menü als Submenü: „Panel“ → `omarchy-shell duckydeck.widget toggle`, „Edit“ → `omarchy-shell shell summon duckydeck.editor` (wie `duckydeck edit` ohne Argumente).
- Bearbeiten der Layouts in v1 über TOML (siehe `config.md`) mit Live-Reload; das Gerät dient als Vorschau.

## v2: Editor `duckydeck.editor` (Entscheidungen 2026-10-03)

Eigenes Plugin vom Typ `overlay`, zusätzlich zum Panel aus v1 (das Panel bleibt die schnelle Bedienung). Öffnen über den Knopf „Edit layout“ im Panel, den Menüeintrag „Stream Deck → Edit“ und `duckydeck edit`; schließen mit Esc.

**Layout:** links Action-Bibliothek, Mitte Gerätevorschau mit Profil-/Seitenleiste darüber, rechts Inspector.

### Gerätevorschau
- 8 Tasten, 4 Regler, Touchstrip im Maßstab des Geräts; die Bilder rendert der Daemon (identisch zum Gerät).
- Auswahl per Klick oder Pfeiltasten. Ordner öffnen per Doppelklick/Enter, Pfad als Breadcrumb.
- **Lernmodus:** Solange der Editor offen ist, löst das Gerät keine Actions aus. Ein Tastendruck oder eine Reglerberührung wählt den Slot im Editor, ein Swipe blättert mit. Beim Schließen gilt wieder der Normalbetrieb.
- Auto-Profilwechsel ist bei offenem Editor pausiert (das bearbeitete Profil bleibt sichtbar).

### Profile und Seiten
- Profil wählen, anlegen (leer oder als Kopie), umbenennen, löschen (mit Bestätigung; das Standardprofil `omarchy` kann nicht gelöscht, nur zurückgesetzt werden).
- Seiten: hinzufügen, löschen, umsortieren (Knöpfe ← / →). Ordner entstehen, indem man `structure.folder` auf eine Taste legt.
- Profil anlegen über „+ New profile…“ im Profil-Dropdown: nur Name eingeben (leer oder Kopie des aktuellen), die Id entsteht aus dem Namen. Inspector mit Reitern „Key / Dial“ und „Profile“.
- Profil-Einstellungen: Name und Auto-Wechsel (`match`) als App-Auswahl („Never“, aktuelles Fenster vor dem Editor, installierte Apps über `StartupWMClass`/Desktop-Id). Unter „Advanced“ Regex für Fensterklasse und **Fenstertitel** (`match.title`, z. B. für Terminal-Apps; beide müssen passen).

### Action-Bibliothek
- Gruppiert nach Kategorie (System, Capture, Medien, Launcher, Fenster, Struktur) plus **Apps** (installierte `.desktop`-Einträge, belegen `launcher.app`; App-Icons später).
- Suche über Label, Id und Kategorie; Tippen startet die Suche sofort.
- Nur zum gewählten Slot passende Actions sind aktiv (Regler-Actions nur für Regler); fehlende Omarchy-Routen ausgegraut mit Hinweis.

### Inspector
- Action (wechselbar), Label (leer = Standardlabel), Icon (eingebaute Icons mit Suche; eigene Bilddateien erst später).
- Parameter als passende Felder: Auswahl bei festen Werten, Zahlenfeld, Text. Ungültige Werte werden sofort markiert (gleiche Prüfung wie `duckydeck check`).
- Multi/Toggle: Liste der Schritte bzw. Zustände, jeder Eintrag aufklappbar und wie ein Slot bearbeitbar; Verzögerung bei Multi als Zahlenfeld.
- Slot leeren, kopieren, einfügen.

### Bearbeiten und Speichern
- Drag & Drop: Bibliothek → Slot belegt; Slot → Slot tauscht. Gleiches per Tastatur (Enter belegt, Strg+X/Strg+V verschiebt).
- **Sofort speichern:** jede Änderung wird direkt in die Profil-TOML geschrieben, das Gerät zeigt sie live. Rückgängig/Wiederholen mit Strg+Z/Strg+Umschalt+Z für die gesamte Editor-Sitzung, auch für „Seite entfernen“ und „Profil löschen“ (`duckydeck edit <id> restore <toml>` schreibt die vorher gesicherte Datei zurück).
- Geschrieben wird mit `toml_edit`: Kommentare und Formatierung bleiben erhalten, Hand-Editieren bleibt gleichwertig. Ändert sich die Datei extern, lädt der Editor neu.
- Die UI schreibt nie selbst Dateien, sondern ruft CLI-Befehle auf (`duckydeck edit …`); der Daemon/CLI validiert vor dem Schreiben.

### Tastatur
- Tab wechselt zwischen Bibliothek, Vorschau und Inspector; `/` fokussiert die Suche; alles ohne Maus bedienbar.

### Nicht im Editor (spätere v2-Punkte)
- Eigene Icon-Dateien, Lautstärke pro App, Script-Actions, zweite Sprache.

### Milestones
| # | Inhalt |
|---|---|
| M9a | API: Katalog/Apps/Icons als JSON, Vorschaubilder je Slot, Lernmodus, `duckydeck edit`-Schreibbefehle mit `toml_edit` |
| M9b | Overlay-Grundgerüst: Gerätevorschau, Auswahl, Profil-/Seitenleiste, Lernmodus |
| M9c | Bibliothek mit Suche, Belegen per Enter/Drag & Drop, Undo |
| M9d | Inspector inkl. Parameter, Icons, Multi/Toggle |
| M9e | Profil- und Seitenverwaltung, Tastatur-Feinschliff |
