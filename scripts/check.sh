#!/usr/bin/env bash
# The checks a pull request must pass, for one extension folder; what a reviewer reads first.
#
#   scripts/check.sh extensions/<id> [BASE]    BASE: a git ref to compare the version with
#                                              (CI: the pull request's base)
#
# Environment: PR_AUTHOR (CI: the pull request's author, for io.github ids).
#
# Read the manifest as Chita does; refuse `network` (not brokered yet); check the id's
# claim; the version must have moved from BASE; build; the module at most 8 MB; the Rust
# tests. Everything goes to the job summary when there is one.
set -euo pipefail

if ! command -v cargo >/dev/null && [ -f "$HOME/.cargo/env" ]; then
  # shellcheck disable=SC1091
  . "$HOME/.cargo/env"
fi

root="$(cd "$(dirname "$0")/.." && pwd)"
dir="${1:?an extension folder}"
base="${2:-}"
dir="${dir%/}"
manifest="$dir/extension.json"
[ -f "$manifest" ] || { echo "no $manifest" >&2; exit 2; }
chita="$("$root/scripts/tool.sh")"
summary="${GITHUB_STEP_SUMMARY:-/dev/null}"
limit=$((8 * 1024 * 1024))

say() { echo "$*"; echo "$*" >> "$summary"; }
fail() { say "FAIL: $*"; exit 1; }

field() { python3 -c 'import json,sys; v=json.load(open(sys.argv[1])); print(v.get(sys.argv[2],""))' "$manifest" "$1"; }
id="$(field id)"
version="$(field version)"
say "## $dir"
say ""
say '```'
"$chita" ext check "$dir" | tee -a "$summary"
say '```'

# The id must be claimable: a domain the reviewer checks, or the pull request's GitHub user.
[ "$(basename "$dir")" = "$id" ] || fail "the folder must be named after the id ($id)"
case "$id" in
  io.github.*)
    user="$(echo "$id" | cut -d. -f3)"
    if [ -n "${PR_AUTHOR:-}" ] && [ "$user" != "$(echo "$PR_AUTHOR" | tr '[:upper:]' '[:lower:]')" ]; then
      fail "$id claims the GitHub user $user; the pull request is from $PR_AUTHOR"
    fi
    say "- id: io.github.$user, the pull request's author";;
  *)
    say "- id: $id. Reviewer: the author must own the domain.";;
esac

# Not brokered yet: refused outright.
network="$(python3 -c 'import json,sys; print(len(json.load(open(sys.argv[1])).get("permissions",{}).get("network",[])))' "$manifest")"
[ "$network" = "0" ] || fail "network permissions aren't accepted yet (Chita doesn't broker the network for extensions)"

# The version moved (when anything users get changed: not for the README or the changelog
# alone), and a changelog mentions it.
if [ -n "$base" ] && git -C "$root" cat-file -e "$base:$manifest" 2>/dev/null; then
  old="$(git -C "$root" show "$base:$manifest" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("version",""))')"
  shipped="$(git -C "$root" diff --name-only "$base" -- "$dir" | grep -v -E '/(README\.md|CHANGELOG\.md)$' || true)"
  if [ "$old" = "$version" ] && [ -n "$shipped" ]; then
    fail "the version is still $version: bump it for every change users get"
  fi
  if [ "$(printf '%s\n%s\n' "$old" "$version" | sort -V | tail -1)" != "$version" ]; then
    fail "the version went from $old to $version: it must go up"
  fi
  [ "$old" = "$version" ] || say "- version: $old → $version"
fi
if [ -f "$dir/CHANGELOG.md" ]; then
  grep -q "$version" "$dir/CHANGELOG.md" || fail "CHANGELOG.md doesn't mention $version"
else
  say "- no CHANGELOG.md (add one: a line per version)"
fi

# Readable source only.
if find "$dir" -type f \( -name '*.wasm' -o -name '*.a' -o -name '*.so' -o -name '*.dylib' -o -name '*.o' \) -not -path '*/node_modules/*' -not -path '*/dist/*' -not -path '*/build/*' -not -path '*/target/*' | grep -q .; then
  fail "prebuilt modules or binaries in the tree: CI builds from source only"
fi

# Build, size, tests.
out="$(mktemp -d)"
trap 'rm -rf "$out"' EXIT
if [ -f "$dir/package.json" ] && [ ! -d "$dir/node_modules" ]; then
  (cd "$dir" && npm ci --no-audit --no-fund)
fi
"$chita" ext build "$dir" --out "$out" | tee -a "$summary"
module="$(field module)"
size="$(stat -f %z "$out/$module")"
[ "$size" -le "$limit" ] || fail "the module is $size bytes; at most $limit"
say "- module: $module, $size bytes"
if [ -f "$dir/Cargo.toml" ]; then
  package="$(python3 -c 'import re,sys; print(re.search(r"^name\s*=\s*\"([^\"]+)\"", open(sys.argv[1]).read(), re.M).group(1))' "$dir/Cargo.toml")"
  (cd "$root" && cargo test --locked --quiet -p "$package")
  (cd "$root" && cargo clippy --locked --quiet -p "$package" --all-targets -- -D warnings)
  say "- tests and clippy: ok"
fi
say "- ok"
