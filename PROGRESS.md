# PROGRESS

Kurz halten: Stand, nächster Schritt, offene Probleme. Am Ende jeder Sitzung aktualisieren.

**Aktueller Milestone:** M8 – Kontext & Release (Auto-Profil, Multi/Toggle, CI erledigt; offen: Packaging, Doku, AUR)

Hardware: Stream Deck + ist angeschlossen (`0fd9:0084`, Bus 008).

## Erledigt
- Shell-Benachrichtigung „Stream Deck + connected“ beim Einstecken (und nach Erst-Setup) – vom Nutzer am Gerät bestätigt
- M8: `packaging/PKGBUILD`, `duckydeck.service`, udev-Regel startet den Dienst beim Einstecken, `LICENSE` (MIT); Paket installiert und am Gerät geprüft: Einstecken startet den Dienst, Setup automatisch, RSS 13,5 MB
- M8: CI-Workflow `.github/workflows/ci.yml` (fmt/clippy/test, `--locked`); noch nie gelaufen – es gibt kein GitHub-Remote. Kontrasttest wird ohne `/usr/share/omarchy/themes` übersprungen
- M8: `structure.profile`/`structure.multi`/`structure.toggle` (`duckydeck_core::compound`) – mit Fake-Device geprüft
- M8: Auto-Profilwechsel (`duckydeck_core::context`, Event `activewindow` + `j/activewindow` beim Verbinden) – mit Fake-Device gegen echtes Hyprland geprüft

## Nächster Schritt
- README/Doku, danach GitHub-Repo `khaledchaar-cpu/duckydeck` + Tag `v0.1.0`, AUR

## Geplante Skills (in .claude/skills/ anlegen, wenn der Milestone fertig ist)
- nach M5a: `add-action` nur für Rust-Actions mit Logik – Entscheidung offen (ggf. weglassen)

## Offene Probleme / Notizen
- Laufender Dev-Daemon nutzt altes Binary (neu starten). Auto-Profil/Toggle am echten Gerät noch nicht angesehen; leere Fenster (Menü/Launcher offen, leerer Workspace) fallen aufs manuelle Profil zurück
- Tooltip-Text beim Hover noch nicht angesehen
- Einmal zeigte das Profil-Dropdown nach Laufzeit-Profilwechsel den alten Wert (vor Shell-Neustart); nicht reproduzierbar
- Menüeintrag wird nur bei neuer Paketversion aktualisiert (Setup-Marker)
- `setup --remove` nimmt das Widget nicht aus dem Bar-Layout (keine Route zum Entfernen)
- `scripts/check.sh` findet `cargo` nur mit `PATH=$HOME/.cargo/bin:$PATH`
- Nicht am Gerät geprüft: Long-Press-Bestätigung, mehrseitige Profile/Swipe; Multi-Monitor-Actions (nur DP-1)
- Catalog-Actions per `spawn`: Fehler von `omarchy` werden nicht gemeldet; Nachtlicht-Status ohne Event
- Hyprland-Zugriff noch nicht hinter Trait (I/O nicht testbar)
- Socket-Pfad (SUN_LEN): Tests mit kurzem `XDG_RUNTIME_DIR` unter `/tmp/claude-1000`
- Paket 0.1.0 ist installiert: Entwicklung mit `systemctl --user stop duckydeck` + `cargo run`; `/usr/bin/duckydeck` ist das Paket-Binary
- Dienst zeigt `is-enabled: disabled` (global über `/usr/lib/…/wants`); abschalten nur per `systemctl --user mask duckydeck` → in README erwähnen
- `duckydeck check` meldet Fehler ohne Shell-Benachrichtigung; der Daemon prüft Action-Ids beim Laden noch nicht (unbekannte → Log „not implemented yet“)
- PKGBUILD-Quelle `$url/archive/v$pkgver.tar.gz` existiert erst mit Repo + Tag; `sha256sums` dann eintragen. Rust beim Nutzer per rustup → lokal `makepkg -d`
