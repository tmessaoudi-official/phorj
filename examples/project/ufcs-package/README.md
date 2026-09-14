# `project/ufcs-package/` — UFCS on a library package's own functions

UFCS lets you call a free function method-style: `name.shout()` means `shout(name)`. Inside a
library package it resolves that package's OWN free functions, from any of its files, under the
same visibility rules as a plain call (DEC-527).

```
ufcs-package/
└── src/
    ├── main.phg              # package Main — calls Text.greet / Text.cheer
    └── Acme/Text/            # package Acme.Text
        ├── shout.phg         #   internal function shout(string): string
        └── greet.phg         #   private wrap(...); greet/cheer call name.shout() and name.wrap()
```

Output, identical on the interpreter, the VM and the transpiled PHP:

```
hello, phorj!
<world> world!
```

## What is rejected

| Call | Where | Result |
|---|---|---|
| `s.wrap()` | `shout.phg` (another file of `Acme.Text`) | `E-VIS-PRIVATE` — `wrap` is `private` to `greet.phg` |
| `s.shout()` | `main.phg` (package `Main`) | an error: UFCS reaches only the CURRENT package's functions. Call it qualified. |

The `E-VIS-PRIVATE` for a method-style call comes from the checker (with a line and column); the
same violation as a bare call, `wrap(s)`, is rejected earlier by the loader. Both carry the same
code.

Calling another package's function method-style is deferred, imported or not — see
`KNOWN_ISSUES.md` §ufcs-cross-package. The transpiled PHP carries no trace of UFCS:
`name.shout()` becomes the namespaced call `\Acme\Text\shout($name)`.
