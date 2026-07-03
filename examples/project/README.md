# Multi-file projects (M5)

Single `.phg` files are great for scripts, but real programs span many files and packages. Phorj's
**project model** (milestone M5) is Go-shaped: every file declares a `package`, the folder layout
*is* the package path, and cross-package functions are imported and called leaf-qualified — and it
all transpiles to idiomatic namespaced PHP.

Each subdirectory here is a self-contained project, discovered by its `phorj.toml`. Like every
other example, each one runs byte-identically on both backends — `tests/differential.rs` finds every
project root and asserts `run` ≡ `runvm` (and that it runs at all).

## `tempconv/` — a two-package Celsius→Fahrenheit converter

```
tempconv/
├── phorj.toml                     # module = "acme/tempconv", source = "src"
└── src/
    ├── main.phg                    # package Main   — the runnable entry
    └── Acme/
        ├── Convert/                # package Acme.Convert  (folder = path)
        │   ├── temp.phg            #   cToF(c) = scale(c) + 32
        │   └── scale.phg           #   scale(c)  = c * 9 / 5
        └── Label/                  # package Acme.Label
            └── label.phg           #   tag(name, v) -> "{name} = {v}F"
```

Run it (the CLI walks up to `phorj.toml`, loads the whole project, and runs `package Main`):

```console
$ phg run examples/project/tempconv/src/main.phg
freezing = 32F
boiling = 212F

$ phg runvm examples/project/tempconv/src/main.phg   # byte-identical
freezing = 32F
boiling = 212F
```

### What it demonstrates

1. **Mandatory packages + folder = path.** Each file's first line is a `package` declaration, never
   inferred. A dotted library package must live in the matching directory under the source root:
   `package Acme.Convert;` ⇒ `src/Acme/Convert/`. The reserved `package Main;` is the runnable entry
   and is folder-exempt. A mismatch is a load error (`E-PKG-PATH`).
2. **Cross-package qualified calls + aliasing.** `main` imports a package and calls its functions
   *leaf-qualified* — `import Acme.Convert;` then `Convert.cToF(0)` (Go's `import "fmt"` →
   `fmt.Println`). An import can be renamed with `as`: `import Acme.Label as Fmt;` binds the leaf
   `Fmt`, so the call is `Fmt.tag(...)`.
3. **Same-package calls across files.** A package may span multiple files. In `Acme.Convert`,
   `cToF` (temp.phg) calls `scale` (scale.phg) by its **bare** name — same package, no
   qualification — and the loader resolves both consistently.

### The PHP it transpiles to

`phg transpile examples/project/tempconv/src/main.phg` emits one PHP `namespace` block per package
plus a bootstrap that invokes `main` last (so every function is declared before it runs):

```php
<?php
namespace Acme\Convert {
    function scale(int $c): int { return $c * 9 / 5; }
    function cToF(int $c): int { return \Acme\Convert\scale($c) + 32; }
}
namespace Acme\Label {
    function tag(string $name, int $value): string { return ($name) . " = " . ($value) . "F"; }
}
namespace Main {
    function main(): void { /* … */ }
}
namespace {
    \Main\main();
}
```

Package segments map **1:1** to PHP namespaces (`Acme.Convert` ⇒ `Acme\Convert`) — segments are
PascalCase at the source, so there is no casing transform; cross-package calls emit fully-qualified
(`\Acme\Convert\cToF`). It runs under a bare `php out.php` — no Composer and no autoloader (PSR-4
can't autoload free functions, and Phorj is function-heavy).

> The conversions use **exact integer arithmetic** (0→32, 100→212) on purpose: a non-whole result
> would render differently under PHP's float `/` than under Phorj's integer `/`, so the example
> sticks to values that are identical across all three. The `run` ≡ `runvm` spine is always identical
> regardless.

## Scope

Library packages export **functions and types** — a `class`/`enum`/`interface` in a library package
is consumed cross-package via `import Pkg.Path.TypeName;` (see `shapes/`). Git-based
dependencies (`[require]` in `phorj.toml`), `phorj.lock`, and vendoring ship in M5 S3 (see
`withdeps/`). Casing is enforced: package/folder segments are PascalCase (`E-PKG-CASE`), types are
PascalCase, functions/variables are camelCase.

## The other projects here

`tempconv/` is the walkthrough above; each sibling project is self-contained, `phorj.toml`-discovered,
and byte-identity-gated the same way:

| Project | What it demonstrates |
|---|---|
| `funcvalues/` | **cross-package lambdas + first-class function values** (M3 S3) — a library package's functions use a lambda (calling a same-package function) and a bare function-value reference, both resolving across the package boundary |
| `genericbox/` | **cross-package generic types** (M-RT generics-all) — a library package's generic class used from `Main` via the terminal `import Pkg.Path.TypeName;` form; type parameters infer and erase across the boundary |
| `inherit/` | **cross-package inheritance + parent dispatch** (M-RT S6/B1a) — a `Main` class extends a library base, inherits its constructor and field, overrides an `open` method, and calls up via both the bare and the named-ancestor `parent` forms |
| `jsonmulti/` | **`Core.Json` in a multi-package project** — building, stringifying, and parsing JSON from a `Main` entry that also imports a library package (the injected `Json` enum is a `Main` type, so its variants live in `\Main\`) |
| `mixins/` | **cross-package traits** (M-RT S8) — compose two library-package traits into a `Main` class via `import Pkg.Path.TraitName` + `use TraitName;` (a trait is still not a type — `Loud x` as an annotation is `E-USE-AS-TYPE`) |
| `shapes/` | **cross-package types** (M-RT) — a library package exports a `class` + `interface` + `enum`, consumed from `Main`; nominal subtyping, `instanceof`, and enum `match` all cross-package, erasing to namespaced PHP |
| `visibility/` | **declaration visibility** — `public`/`internal`/`private` on top-level declarations, loader-enforced and erased from PHP (see `visibility/README.md`) |
| `withdeps/` | a **vendored git dependency** (M5 S3) — `[require]`, `phg vendor`, `phorj.lock`, and an offline `vendor/` (see `withdeps/README.md`) |
