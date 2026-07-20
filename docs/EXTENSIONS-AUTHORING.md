# Authoring a phorj extension (DEC-315)

> Companion to the auto-generated catalog `docs/EXTENSIONS.md` (which lists what SHIPS). This guide is
> the hand-written **how-to author your own** — the two supported paths, when to pick each, and the
> rules each must obey. Ruled 2026-07-20 (DEC-315); the cross-language survey behind it (Rust / PHP /
> Go / Python / JVM / Swift / C# / Racket / Zig) is summarized in the decision register.

## The boundary: core vs first-party extension vs userland package

phorj has one crisp classification test — **"could a `.phg` library express this on top of the kernel?"**
— and a *second, independent* axis, **who delivers it**. That gives three buckets:

| Bucket | Rule | Toggleable? | Written in |
|---|---|---|---|
| **CORE** | irreducible; the language can't do real work without it (values, control flow, the thinnest OS seams — File/Process/Environment/Random/Output/Runtime, generics, attributes) | never | Rust + JIT |
| **FIRST-PARTY EXTENSION** | `.phg`-expressible **but** needs a *vetted native dependency*, or native perf to satisfy WIN-OR-FLAG (Invariant 18), or is foundational enough that batteries-included ergonomics matter | Cargo feature (Mandatory / Default / OptIn tier) | Rust + JIT |
| **USERLAND PACKAGE** | `.phg`-expressible **and** needs neither a native dep nor native perf | not a build concern — just present in `vendor/` | phorj `.phg` |

Decision rule: **ship first-party Rust only when native deps, native perf, or batteries-included
ergonomics demand it; otherwise it is a userland `.phg` package.** (This is the DEC-208 / DEC-218
posture generalized — the query builder and web spine are userland.)

---

## Path 1 — a userland `.phg` package (the primary third-party path)

A userland package is *just phorj source*. It inherits **for free, by construction**: the byte-identity
spine (it transpiles to PHP like any app code), correct transpile / lift / LSP (same loader, same AST —
the surfaces cannot go stale for it), and no `phg` rebuild, no feature flag, no native trust. There is
**nothing to quarantine** — no LADDER interaction at all.

**Layout & namespace (DEC-282).** Drop your package under the consuming app's vendor root:

```
<approot>/vendor/<Publisher>/<Name>/…      →  imported as   Publisher.Name.*
```

Folder = package, file = type. `Core.*` is **reserved for first-party** (core + first-party
extensions) and is resolved ahead of all search roots — so `Core.` names can never be squatted, and the
bright line holds: *if it's `Core.`, the core team shipped it; anything else is third-party.*

**Consuming** is already shipped: `phg`'s import-driven lazy loader searches entry-dir → `<approot>/src/`
→ `<approot>/vendor/`, warns `W-SHADOWED` on duplicates, and **never touches the network** (`phg run` /
`check` / `transpile` are offline; determinism Invariant 10).

**Distributing** is handled by the package-manager verbs (DEC-316, shipped): `phg add`, `phg install`,
`phg update`, `phg remove` — the only network-capable commands (`run`/`check`/`transpile` stay offline).
A dependency is declared in a composer.json-style **`phorj.json`** and comes from one of three sources —
a **registry** semver constraint (`"^1.2"`), a **git** repo (`{ "git": url, "ref": tag }`), or a local
**path** (`{ "path": dir }`) — all materialized into `vendor/<Publisher>/<Name>/` and pinned by a tree
SHA-256 in **`phorj.lock`** (re-verified offline on the next install; a tampered/stale tree is refused).
The central registry is a lightweight name→git-URL index (`PHORJ_REGISTRY`), so every fetch is a `git`
checkout or a filesystem copy — no tarballs. Worked example: `examples/package-manager/`.

```console
$ phg add Acme/Json@^1.2                                   # registry
$ phg add Foo/Bar --git https://example.test/bar.git --ref v2.1.0   # git
$ phg add Dev/Local --path ../local                        # local path
$ phg install     # resolve -> vendor/ -> phorj.lock       $ phg update / phg remove Acme/Json
```

**When to choose this:** almost always. Frameworks (router, DI, ORM, forms, CSRF, migrations,
scheduler), application concerns (caching over a seam, queues/jobs), alternative serialization formats
(YAML/TOML), message-catalog i18n — all `.phg`-expressible → userland.

---

## Path 2 — a native Rust extension (the SPI)

Choose this only when a capability needs a **vetted native dependency** or **native perf** and cannot be
a userland package. A native extension is compiled **into** `phg` from source (same `rustc`, behind a
feature flag) — there is **no dynamic ABI**. This mirrors the extensibility patterns that actually work
across languages (Roslyn source generators, Swift macros, Racket `#lang`, Zig `build.zig`, Go's own
recommended "blank-import + static build"): compile-time, source-level, in the trusted toolchain.

### The five pieces of a native extension

1. **A folder** `src/ext/<name>/` — natives + a `.phg` prelude source + tests, colocated (AMENDMENT 2).
   Split by cohesion; every file obeys the 300-soft / 500-hard line cap (Invariant 13).
2. **A registry row** in `src/ext/registry.rs` — the single source of truth (the phorj analogue of a
   ServiceLoader SPI / Python `entry_points`). One `Extension { name, feature, enabled, tier, modules,
   summary, migrated }` row makes the extension visible to the compiler, the disabled-import gate
   (`E-EXTENSION-DISABLED`), `phg extensions`, and the generated `docs/EXTENSIONS.md` at once.
3. **A Cargo feature** gating build inclusion (Mandatory always-in / Default batteries-included /
   OptIn explicit). External crates must clear the External-dependency policy (vetted, minimal).
4. **The trait seam.** Where an extension has pluggable backends, program to a documented,
   semver-stable trait — e.g. the database driver seam `DriverConn`
   (`src/ext/database/natives/driver.rs`) or the mail `Transport` seam. These traits ARE the public SPI:
   a third party implements the trait, adds a registry row, and rebuilds `phg --features their-ext`.
5. **The natives + prelude.** Natives register `NativeFn` rows (each carries its `php:` emitter — the
   transpile facet — single-sourced); the `.phg` prelude is the public `Core.`-style surface on top.

### The LADDER rule is mandatory (Invariant 14)

Native code has **no automatic PHP twin**. Every native extension MUST do one of:

- **Ship a faithful PHP twin** — emit a `__phorj_<ext>_*` helper at the transpiler's runtime-tables
  chokepoint so the PHP leg stays byte-identical (the tool is always acceptable, but the trade is
  surfaced and developer-ruled — Invariant 16); **or**
- **Declare native-only** — a hard `E-TRANSPILE-<EXT>` error on transpile + differential-harness
  quarantine (`pure: false` auto-excludes importing programs) + a disclosure paragraph wherever
  byte-identity is claimed.

A **silent semantic downgrade is forbidden.** This discipline is per-extension and ongoing — it is the
reason native extensions are heavier than userland packages.

### Every surface must know about it (Invariant 17)

A feature that runs but doesn't transpile *and* lift, or is missing from LSP completion / examples /
guides / conformance / playground, is **not done**. The registry `modules` list drives import
completion + the disabled-import gate; the `NativeFn` `php:` facet drives transpile; the (future)
`lift_from` facet (DEC-312) drives lift. Ship the example (Invariant 9) + a winning micro/macro bench
(Invariant 18) in the same change.

---

## Rejected, permanently: dynamic `.so` plugins

The PHP-C-extension / Go-`plugin` style (load a native `.so` at runtime) is **rejected for good**:

- Rust has **no stable ABI by design** (the proc-macro bridge is deliberately unstable; `abi_stable` /
  `stabby` are C-ABI workarounds). Go's own `plugin` package requires exact toolchain + dep lockstep and
  is de-facto dead for exactly this reason.
- A loaded `.so` is arbitrary, un-auditable native code — against the spirit of `#![deny(unsafe_code)]`.
- It has **no PHP twin** → an unavoidable LADDER violation with no faithful mapping.
- It cannot be sandboxed.

It fails the spine, the safety promises, and security simultaneously. Native extension = source-level,
compiled-in, or nothing.

---

## Security / trust model

A userland package runs with **full language capability** (it may `import Core.File`, `Core.Process`,
`Core.HttpClient`, …) — exactly like a Composer package: unprivileged relative to the OS user, fully
privileged relative to the app. phorj has no capability sandbox today; the package manager (DEC-316)
pins every dependency with a tree SHA-256 in `phorj.lock` and refuses a tampered/stale `vendor/` on
install — supply-chain integrity, though not a capability sandbox. A native extension is trusted the
moment you compile it into your `phg` — vet it like any Cargo dependency.
