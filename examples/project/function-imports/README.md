# function-imports — cross-package function imports (DEC-197, slice 2)

A multi-file project showing the **two-mode import discipline extended to module functions** across a
package boundary. The library package `App.Text` (in `src/App/Text/util.phg`) exports a few functions;
`package Main` (`src/main.phg`) consumes them three ways:

| Form | Import | Call |
|------|--------|------|
| **bare** (member import) | `import App.Text.banner;` | `banner("…")` |
| **aliased** | `import App.Text.shout as yell;` | `yell("…")` |
| **qualified** (whole-module) | `import App.Text;` | `Text.banner("…")` |

The loader rewrites a bare (or aliased) imported call to the **same mangled FQN** a qualified call
produces (`\App\Text\banner`), so `interpreter ≡ VM ≡ transpiled PHP` is inherited from the proven qualified
cross-package path — including when the call is an arithmetic operand (`addUp(1, 2) + 1`).

Discipline (same as Core stdlib functions, slice 1): a `private` function is file-scoped and cannot be
member-imported cross-package (`E-VIS-PRIVATE`); importing the same bare name from two packages is
`E-IMPORT-CONFLICT` — alias one with `as`; resolution order is `local > same-package fn > imported`.

Run it:

```
phg run   examples/project/function-imports/src/main.phg
phg run --tree-walker examples/project/function-imports/src/main.phg
phg transpile examples/project/function-imports/src/main.phg | php
```
