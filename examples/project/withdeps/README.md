# `withdeps` — an app with a vendored dependency (DEC-282)

This app depends on an external library package, `Acme.Strutil`, consumed **offline** from the
committed `vendor/` tree. There is no manifest and no lockfile in the language: `phg` NEVER
downloads code — a future package-manager extension fetches/updates `vendor/`; the compiler only
reads what is on disk.

## Layout

```
withdeps/
├── src/
│   └── main.phg                    # package Main — imports & calls Acme.Strutil
└── vendor/                         # committed offline dependency tree
    └── Acme/Strutil/               #   vendor/<Publisher>/<Name>/ — folder = package
        └── text.phg                #   package Acme.Strutil
```

## Run it

```sh
phg run   src/main.phg               # bytecode VM
phg run --tree-walker src/main.phg   # tree-walking interpreter (byte-identical)
phg transpile src/main.phg | php
```

All three print the same two lines — the vendored dependency is consumed exactly like a
first-party package:

```
== Phorj deps ==
vendored offline!
```

## How dependencies resolve (DEC-282)

`import Acme.Strutil;` searches, in order: the entry's own directory, `<approot>/src/`, then
`<approot>/vendor/` — first match wins. A first-party `src/Acme/Strutil/` would deliberately
shadow the vendored copy (the standard local-override escape hatch), and the shadow is never
silent (`W-SHADOWED` names both paths). An unresolvable import is `E-MODULE-NOT-FOUND`, listing
exactly what was searched — with the hint that `phg` never fetches.

Determinism is the committed tree itself: what is vendored is what runs, byte-for-byte, on every
machine and in CI, with zero network. Version selection, updating, lockfiles, and registries are
the future extension's concern — deliberately OUTSIDE the language.
