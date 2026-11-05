# Chita extensions

The extensions for [Chita](https://github.com/decomponendiscifris), the fast file manager
for macOS, and the store they're published from. Everything here is open source, reviewed by
pull request, built from the reviewed source by this repository's own CI, and signed with
the store's key. Chita installs only what it can check: a package whose signature matches its
files, and a catalog signed by the store.

## For users

Chita's store window lists what's here. Until it ships, a package (`.chitaext`) from the
[releases](https://github.com/decomponendiscifris/chita-extensions/releases) installs from
Settings › Extensions › Install…, or by opening it; the install sheet shows "Signed by the
Chita extensions store", what the extension will be able to do, and its risk.

The catalog Chita reads is `catalog.json` on this repository's pages, next to
`revoked.json`, the list of extensions the store pulled back. Both are signed.

## For authors

An extension is a WebAssembly module with a manifest (`extension.json`), run by Chita in a
sandboxed process and given only what the manifest asks for and the user approved. It can
add commands, a column to the file list, and a panel to the Inspector. It's written in Rust
on `sdk/rust`, or in AssemblyScript (a TypeScript dialect) on `sdk/assemblyscript`.

The author's tools are `chita ext`: `scripts/tool.sh` downloads the pinned `chita` command
line into `.tool/`. Then:

```sh
.tool/chita ext new extensions/io.github.you.notes --id io.github.you.notes --name Notes
.tool/chita ext build extensions/io.github.you.notes        # manifest and module into dist/
.tool/chita ext run extensions/io.github.you.notes/dist command --in ~/Documents
.tool/chita ext check extensions/io.github.you.notes         # what it declares, its risk
```

`chita ext new` writes a crate depending on the SDK by path: in this repository, point it at
`../../sdk/rust` (`--sdk sdk/rust`). `scripts/check.sh extensions/io.github.you.notes` runs
what CI runs on a pull request.

Read [CONTRIBUTING.md](CONTRIBUTING.md) before opening a pull request: the rules on
licenses, ids, permissions and binaries are what the review holds you to.

## Layout

- `extensions/<id>/`: one folder per extension, named by its id, with `extension.json`, the
  source, a README and a CHANGELOG.
- `sdk/rust`: the Rust SDK (`chita-ext-sdk`). `sdk/assemblyscript/chita.ts`: the
  AssemblyScript SDK. Both come from Chita's repository, where they're developed.
- `scripts/`: what CI runs (`check.sh` on pull requests, `publish.sh` on merge) and
  `tool.sh`.
- `.github/workflows/`: `check.yml` (pull requests: no secrets, fresh runners) and
  `publish.yml` (merges to `main`: build, sign with the store key, release, catalog).

## Security

See [SECURITY.md](SECURITY.md): how to report a problem, and how revocation works.
