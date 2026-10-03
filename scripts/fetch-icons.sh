#!/usr/bin/env bash
# Fetch the pinned Tabler Icons release and copy the icons listed in
# assets/icons.toml to assets/icons/<category>/<name>.svg.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
toml="$root/assets/icons.toml"
out="$root/assets/icons"

value() { awk -F' *= *' -v k="$1" '/^\[tabler\]/{t=1;next} /^\[/{t=0} t&&$1==k{gsub(/"/,"",$2);print $2}' "$toml"; }
version="$(value version)"
sha256="$(value sha256)"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
tgz="$tmp/tabler.tgz"
curl -fsSL "https://registry.npmjs.org/@tabler/icons/-/icons-$version.tgz" -o "$tgz"
echo "$sha256  $tgz" | sha256sum -c --quiet -
tar -xzf "$tgz" -C "$tmp"
src="$tmp/package/icons/outline"

# Regenerate everything except hand-made custom icons.
find "$out" -mindepth 1 -maxdepth 1 -type d ! -name custom -exec rm -rf {} + 2>/dev/null || true
mkdir -p "$out"

count=0
missing=0
while IFS=$'\t' read -r category name tabler; do
  if [[ ! -f "$src/$tabler.svg" ]]; then
    echo "missing in Tabler $version: $tabler (for $category/$name)" >&2
    missing=1
    continue
  fi
  mkdir -p "$out/$category"
  # Drop the Tabler CSS class; everything else stays as published.
  sed '/^  class="/d' "$src/$tabler.svg" > "$out/$category/$name.svg"
  count=$((count + 1))
done < <(awk -F' *= *' '
  /^\[/ { sec=$0; gsub(/[][]/,"",sec); next }
  sec=="tabler" || sec=="pairs" || sec=="custom" || /^#/ || NF<2 { next }
  { v=$2; gsub(/"/,"",v); print sec "\t" $1 "\t" v }
' "$toml")

cp "$tmp/package/LICENSE" "$out/LICENSE-tabler.txt"
echo "fetched $count icons from Tabler $version"
exit "$missing"
