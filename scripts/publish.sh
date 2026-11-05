#!/usr/bin/env bash
# Publishes one extension from `main`, if its version isn't out yet: builds it, packs and
# signs it with the store's key, creates the release `<id>-<version>` with the package, and
# lists it in the catalog (SITE/catalog.json), making SITE/revoked.json when missing.
#
#   scripts/publish.sh extensions/<id> SITE
#
# Environment: STORE_KEY_FILE (the key; CI writes the secret to a file only this user
# reads), GH_TOKEN (to create the release).
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
dir="${1:?an extension folder}"
site="${2:?the pages checkout}"
dir="${dir%/}"
key="${STORE_KEY_FILE:?the store key file}"
chita="$("$root/scripts/tool.sh")"
repo="decomponendiscifris/chita-extensions"

field() { python3 -c 'import json,sys; v=json.load(open(sys.argv[1])); print(v.get(sys.argv[2],""))' "$dir/extension.json" "$1"; }
id="$(field id)"
version="$(field version)"
name="$(field name)"
tag="$id-$version"

if gh release view "$tag" --repo "$repo" >/dev/null 2>&1; then
  echo "$tag is published already"
  exit 0
fi

out="$(mktemp -d)"
trap 'rm -rf "$out"' EXIT
if [ -f "$dir/package.json" ] && [ ! -d "$dir/node_modules" ]; then
  (cd "$dir" && npm ci --no-audit --no-fund)
fi
"$chita" ext build "$dir" --out "$out/dist"
package="$out/$tag.chitaext"
"$chita" ext pack "$out/dist" --key "$key" --out "$package"
"$chita" ext check "$package"

notes="$out/notes.md"
if [ -f "$dir/CHANGELOG.md" ]; then
  # The changelog's section for this version, up to the next heading.
  awk -v v="$version" '
    /^## / { on = index($0, v) > 0; if (on) next }
    on { print }
  ' "$dir/CHANGELOG.md" > "$notes"
fi
[ -s "$notes" ] || echo "$name $version." > "$notes"
gh release create "$tag" "$package" --repo "$repo" --title "$name $version" --notes-file "$notes"
url="https://github.com/$repo/releases/download/$tag/$tag.chitaext"

"$chita" ext catalog --list "$site/catalog.json" --key "$key" "$package" --url "$url" \
  --repository "https://github.com/$repo/tree/main/$dir"
[ -f "$site/revoked.json" ] || "$chita" ext revoke --list "$site/revoked.json" --key "$key"
echo "published $tag"
