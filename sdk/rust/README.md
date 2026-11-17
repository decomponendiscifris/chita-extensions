# chita-ext-sdk

The Rust SDK for writing extensions for [Chita](https://github.com/decomponendiscifris/chita-releases),
a very fast file manager for the Mac.

An extension is a WebAssembly module that Chita runs in its own sandbox. This crate holds what
it is written with: the requests it can make, what it gets back, and the entry points Chita
calls.

The extensions themselves, the AssemblyScript SDK and examples are in
[chita-extensions](https://github.com/decomponendiscifris/chita-extensions).

MIT licensed.
