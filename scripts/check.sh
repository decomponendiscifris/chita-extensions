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

# What the store shows: a README (required), an icon (512×512 PNG when there), screenshots
# (PNG, when there), categories (recommended).
[ -f "$dir/README.md" ] || fail "no README.md: the store shows it on the extension's page"
if [ -f "$dir/icon.png" ]; then
  w="$(sips -g pixelWidth "$dir/icon.png" | awk '/pixelWidth/ {print $2}')"
  h="$(sips -g pixelHeight "$dir/icon.png" | awk '/pixelHeight/ {print $2}')"
  [ "$w" = "512" ] && [ "$h" = "512" ] || fail "icon.png is ${w}×${h}; the store wants 512×512"
  say "- icon: 512×512"
else
  say "- no icon.png (the store shows a generic one; add a 512×512 PNG)"
fi
if [ -d "$dir/screenshots" ]; then
  for shot in "$dir"/screenshots/*; do
    case "$shot" in *.png|*.jpg) ;; *) fail "screenshots/ holds $(basename "$shot"): PNG or JPEG only";; esac
  done
  say "- screenshots: $(ls "$dir/screenshots" | wc -l | tr -d ' ')"
fi
categories="$(python3 -c 'import json,sys; print(len(json.load(open(sys.argv[1])).get("categories",[])))' "$manifest")"
[ "$categories" != "0" ] || say "- no categories (the store files it under Other; name up to three)"

# Readable source only.
if find "$dir" -type f \( -name '*.wasm' -o -name '*.a' -o -name '*.so' -o -name '*.dylib' -o -name '*.o' \) -not -path '*/node_modules/*' -not -path '*/dist/*' -not -path '*/build/*' -not -path '*/target/*' | grep -q .; then
  fail "prebuilt modules or binaries in the tree: CI builds from source only"
fi

# The security review, for the reviewer: what the extension asks Chita for, where in the
# source (every ask site, read, write, tool run), what changed in its permissions since
# the base, and which dependencies are new. Nothing here decides; the reviewer reads it.
say ""
say "### Security review"
say ""
if [ -n "$base" ] && git -C "$root" cat-file -e "$base:$manifest" 2>/dev/null; then
  before="$(git -C "$root" show "$base:$manifest" | python3 -c 'import json,sys; p=json.load(sys.stdin).get("permissions",{}); print(json.dumps(p, sort_keys=True))')"
  after="$(python3 -c 'import json,sys; p=json.load(open(sys.argv[1])).get("permissions",{}); print(json.dumps(p, sort_keys=True))' "$manifest")"
  if [ "$before" != "$after" ]; then
    say "- PERMISSIONS CHANGED: were \`$before\`, now \`$after\`. Read twice."
  else
    say "- permissions unchanged: \`$after\`"
  fi
else
  say "- new extension; permissions: \`$(python3 -c 'import json,sys; print(json.dumps(json.load(open(sys.argv[1])).get("permissions",{}), sort_keys=True))' "$manifest")\`"
fi
sites="$(grep -rnE 'run_tool\(|write_file\(|read_file\(|list_folder\(|ask\(|runTool\(|writeFile\(|readFile\(|listFolder\(|RunTool|WriteFile|ReadFile|ListFolder' --include='*.rs' --include='*.ts' "$dir" 2>/dev/null \
  | grep -v -E '^[^:]+/(node_modules|dist|build|target)/' | grep -v -E '/(chita\.ts|tests?)/' || true)"
if [ -n "$sites" ]; then
  say "- where it asks Chita for files, folders and tools ($(printf '%s\n' "$sites" | wc -l | tr -d ' ') sites):"
  say '```'
  printf '%s\n' "$sites" | sed "s#^$root/##" | cut -c1-200 | tee -a "$summary"
  say '```'
else
  say "- it asks Chita for nothing (no ask sites in the source)"
fi
if [ -n "$base" ] && git -C "$root" cat-file -e "$base:Cargo.lock" 2>/dev/null; then
  added="$(diff <(git -C "$root" show "$base:Cargo.lock" | grep -E '^name = ' | sort -u) <(grep -E '^name = ' "$root/Cargo.lock" | sort -u) | grep '^>' | sed 's/^> name = //' | tr -d '"' | paste -sd, - || true)"
  if [ -n "$added" ]; then say "- NEW DEPENDENCIES: $added. Read what they pull in."; else say "- no new dependencies"; fi
fi
if find "$dir" -name 'build.rs' -not -path '*/target/*' | grep -q .; then
  say "- HAS A BUILD SCRIPT (build.rs): it runs on the reviewer's machine and on CI. Read it first."
fi
if grep -rqE 'proc_macro|#\[proc_macro' --include='*.rs' "$dir" 2>/dev/null; then
  say "- defines procedural macros: they run at build time. Read them first."
fi
say ""

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
