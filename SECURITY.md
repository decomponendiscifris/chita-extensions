# Security

## Reporting a problem

An extension that does something it shouldn't, a package that doesn't match its source, a
hole in the review: report it privately, not in a public issue.

- GitHub: [report a vulnerability](https://github.com/decomponendiscifris/chita-extensions/security/advisories/new)
  (private to the maintainers).
- Mail: federico.lattanzi@urial.it.

You'll get an answer within three days.

## Revocation

The store can pull an extension back after it was published: `revoked.json`, signed with
the store's key, lists ids and the versions revoked (or every version) with a reason. Chita
fetches it daily, turns a revoked extension off with the reason shown in Settings, and
refuses to install a revoked version.

The policy is **revoke first, discuss after**: a credible report of an extension harming
users gets it revoked before the discussion with its author, and un-revoked when the
problem is understood and fixed. Revocation is kept by the maintainers with
`chita ext revoke` and the store's key; it never happens by CI on its own.

## Keys

Packages and the catalog are signed with the store's key, kept as a secret of this
repository's `store` environment, used only by the publish workflow on `main`. Chita trusts
a list of store keys, so a key can be retired by a Chita update while packages signed with
the new one already work. A suspected compromise is treated as a revocation of everything
signed since, plus a rotation.
