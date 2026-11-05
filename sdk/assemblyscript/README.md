# The AssemblyScript SDK

`chita.ts` is what `chita ext new --language typescript` writes into an extension's
`assembly/` folder: the host's imports (`log`, `ask`, `take`, `result`), `alloc`, and the
helpers to read a call's argument and give a result back. Copy it next to your
`index.ts`, or let `chita ext new` do it. AssemblyScript has no JSON of its own: a library
such as `json-as` fills in.
