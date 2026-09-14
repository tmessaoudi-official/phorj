# `project/tag-package/` — template tags in a library package

A tagged template `tag"…"` desugars to a call on its tag: a free function
`(List<string>, List<H>) -> R` (function mode) or a type providing `raw`/`text`/`concat`
(protocol mode). Inside a library package the tag name resolves like any bare identifier — the
CURRENT package's own function or type first, then an import — so a template never picks up
`Main`'s same-named tag (scout row 5h1).

```
tag-package/
└── src/
    ├── main.phg              # package Main — its own `mark` tag; calls Text.badge / Text.shout
    └── Acme/Text/            # package Acme.Text
        ├── mark.phg          #   internal function mark(List<string>, List<string>): string
        ├── Loud.phg          #   internal class Loud { raw / text / concat }
        └── badge.phg         #   badge → mark"badge {name}!", shout → Loud"hey {name}"
```

Output, identical on the interpreter, the VM and the transpiled PHP:

```
badge [phorj]!
hey *phorj*
main (phorj)!
```

## What is rejected

| Tag use | Where | Result |
|---|---|---|
| `mark"…"` with `mark` declared `private` in `mark.phg` | `badge.phg` (another file of `Acme.Text`) | `E-VIS-PRIVATE` — a private tag function is file-scoped like any call to it |
| `mark"…"` with no `mark` in the package, its imports, or `Main` | any package | `E-UNKNOWN-TAG` |

A library tag with no `mark` of its own and no import still reaches `Main`'s `mark` if one exists —
pre-existing and pending a ruling (KNOWN_ISSUES §library-reaches-main).

A member-imported public function of another package works as a tag exactly as it does as a bare
call (`import Acme.Text.mark;` then `mark"…"`). The transpiled PHP carries no trace of the template:
`mark"badge {name}!"` becomes a namespaced call to `mark` with the literal and hole lists.
