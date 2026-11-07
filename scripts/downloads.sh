#!/usr/bin/env bash
# Counts how many times each listed extension's packages were downloaded from this
# repository's releases, all versions together, and writes the numbers into the catalog,
# signed again with the store's key. GitHub's counts are the store's only measure of use:
# Chita sends nothing back. They are rough: a download isn't an install.
#
#   scripts/downloads.sh SITE
#
# Environment: STORE_KEY_FILE (the key), GH_TOKEN (to read the releases).
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
site="${1:?the pages checkout}"
key="${STORE_KEY_FILE:?the store key file}"
chita="$("$root/scripts/tool.sh")"
repo="decomponendiscifris/chita-extensions"

[ -f "$site/catalog.json" ] || { echo "no catalog yet: nothing to count"; exit 0; }
counts="$(mktemp)"
trap 'rm -f "$counts"' EXIT
gh api "repos/$repo/releases" --paginate \
  --jq '.[] | .assets[] | select(.name | endswith(".chitaext")) | [.name, .download_count] | @tsv' |
python3 -c '
import json, re, sys
listed = {e["id"] for e in json.load(open(sys.argv[1])).get("extensions", [])}
counts = dict.fromkeys(listed, 0)
for line in sys.stdin:
    name, count = line.rstrip("\n").split("\t")
    # Packages are named <id>-<version>.chitaext.
    m = re.fullmatch(r"(.+)-\d+(?:\.\d+){1,2}\.chitaext", name)
    if m and m.group(1) in listed:
        counts[m.group(1)] += int(count)
json.dump(counts, sys.stdout)
' "$site/catalog.json" > "$counts"
"$chita" ext downloads --list "$site/catalog.json" --key "$key" "$counts"
