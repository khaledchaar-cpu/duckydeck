---
name: add-icon
description: Add or change a built-in DuckyDeck icon. Use when an action needs an icon that is not in assets/icons.toml, or an icon motif should be swapped. Finds the Tabler name, updates icons.toml, fetches and validates.
---

# Add an icon

1. **Check first:** `grep -n '<name>' assets/icons.toml` – maybe it exists under another name.
2. **Find the Tabler motif** without listing everything. The pinned version is in `[tabler]` of `assets/icons.toml`:
   ```bash
   v=$(awk -F'"' '/^version/{print $2}' assets/icons.toml)
   curl -s https://registry.npmjs.org/@tabler/icons/-/icons-$v.tgz | tar -tz | grep 'icons/outline/.*<keyword>' | head
   ```
   Only outline icons. Prefer the plainest motif; it must read at 56 px.
3. **Add the entry** in the right category: `<duckydeck-name> = "<tabler-name>"`. Names are unique across categories. Toggle states get two icons plus a line in `[pairs]` (active = inactive).
4. **No Tabler motif?** Draw it in Tabler style into `assets/icons/custom/<name>.svg` (viewBox `0 0 24 24`, `stroke="currentColor"`, stroke-width 2, round caps/joins, or `fill="currentColor"` for solid glyphs) and add `<name> = ""` under `[custom]`.
5. `scripts/fetch-icons.sh` (exits non-zero if a Tabler name is missing).
6. `scripts/check.sh duckydeck-core` – `build.rs` validates every icon (viewBox, only `currentColor`/`none`, no bitmaps/styles), the tests render each one and check contrast in all stock themes.
7. Tell the user to look at `cargo run -p duckydeck-core --example gallery` (→ `target/gallery.png`). Do not open the gallery yourself.

Never edit fetched files under `assets/icons/<category>/` by hand – they are regenerated. Never bump the Tabler version casually: it changes every icon (update `version` and `sha256` together, then re-check the gallery).
