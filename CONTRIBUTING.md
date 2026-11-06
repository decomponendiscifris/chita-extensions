# Contributing an extension

Every extension here is reviewed by a person before it's published, every update too. The
rules below are what the review holds you to; the checks in `scripts/check.sh` enforce the
ones a script can.

## Rules

1. **An OSI-approved license.** The store takes your source, builds it, signs it and hands
   the result to other people. That needs a license that lets reviewers read and build it,
   users run it, and maintainers keep fixing it if you stop answering. This repository is
   MIT; an extension may carry its own OSI license file instead.
2. **An id you can claim.** The id is the identity Chita keys everything on: the approval a
   user gave, updates, revocation. It must be something you provably control: a domain you
   own (`com.example.notes`, checked by the reviewer), or `io.github.<your user>.<name>`,
   which CI checks against the pull request's author. Once published, an id belongs to its
   folder's owners (`.github/CODEOWNERS`).
3. **No network permission, for now.** Chita doesn't broker the network for extensions
   yet, so a manifest with `network` is refused by CI. When it does, a network permission
   will need a README saying what is sent, where, and why.
4. **Readable source only.** No prebuilt modules, no vendored binaries, no blobs in source
   files. Dependencies come from crates.io or npm by name and version, with the lockfile
   committed. The only build that's ever signed is CI's, from the merged source.
5. **The least it needs.** Ask for `selected` rather than `folder`, `folder` rather than a
   path, and no tool you don't use. The risk badge users see follows the manifest.
6. **A version bump for every change** users get. `version` in `extension.json`, and a line
   in the extension's CHANGELOG.

## How

Where commands and columns show is up to your manifest: see [sdk/MANIFEST.md](sdk/MANIFEST.md).

1. Fork, and make `extensions/<your id>/` with `.tool/chita ext new` (see the README).
   Write the extension on `sdk/rust` (`--sdk sdk/rust`) or `sdk/assemblyscript`.
2. Run `scripts/check.sh extensions/<your id>` locally: it reads the manifest the way Chita
   does, refuses what the rules refuse, builds, tests, and checks the module's size.
3. Open a pull request. The template asks what the extension does and why it needs each
   permission. CI runs the same checks with no secrets on a fresh runner, and prints the
   manifest and the permissions diff in the job summary for the reviewer.
4. A maintainer reads the diff. Anything that adds a permission or a tool is read twice.
5. Merged, the publish workflow builds it from `main`, packs and signs it with the store's
   key, publishes the package as a release (`<id>-<version>`) and lists it in the catalog.
   Chita picks it up within a day.

## The security review

Every pull request gets a "Security review" section in the job summary, written by
`scripts/check.sh`: whether the permissions changed since `main` and how, every place in the
source where the extension asks Chita for a file, a folder, a write or a tool run, which
dependencies are new, and whether there is a build script or a procedural macro (code that
runs at build time, on the reviewer's machine and on CI). The advisories job checks the
lockfile against the RustSec database. None of this decides anything: the reviewer reads
those places first, and nothing merges without that reading. At runtime the host is
sandboxed whatever the review missed, and revocation covers what slips through.

## What reviewers look at

- What the source does with what it's granted, and whether it needs what it asks for.
- Dependencies added, and what they pull in (the lockfile diff).
- Build scripts and macros: they run on the reviewer's machine and on CI.
- The changelog says what changed; the version moved.
