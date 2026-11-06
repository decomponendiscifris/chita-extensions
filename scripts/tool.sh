#!/usr/bin/env bash
# Fetches the pinned `chita` command line (the author's tools, `chita ext`) into .tool/chita:
# a universal macOS binary published as a release of this repository, checked against its
# SHA-256 before it's used. CI and authors use the same one.
#
#   scripts/tool.sh          fetch when missing or wrong
#   . <(scripts/tool.sh --env)   print `CHITA=.../.tool/chita`
set -euo pipefail

version="0.1.2"
sha256="04f3a5b742b07d3ed7aaf91a3866c87a11bdb3bcfba1dcf03addadd458f72850"
root="$(cd "$(dirname "$0")/.." && pwd)"
tool="$root/.tool/chita"
url="https://github.com/decomponendiscifris/chita-extensions/releases/download/tools-$version/chita"

if [ "${1:-}" = "--env" ]; then
  echo "CHITA='$tool'"
  exit 0
fi

check() { [ -x "$tool" ] && [ "$(shasum -a 256 "$tool" | cut -d' ' -f1)" = "$sha256" ]; }

if ! check; then
  mkdir -p "$root/.tool"
  curl -fsSL --retry 3 -o "$tool.download" "$url"
  got="$(shasum -a 256 "$tool.download" | cut -d' ' -f1)"
  if [ "$got" != "$sha256" ]; then
    rm -f "$tool.download"
    echo "the tool downloaded from $url hashes to $got, not $sha256" >&2
    exit 1
  fi
  mv "$tool.download" "$tool"
  chmod +x "$tool"
fi
echo "$tool"
