# TRANSPILE-NS-PRELUDE — injected preludes must resolve from the global helper block

**Status: BUILT 2026-08-23 (DEC-455.11).** Unblocks DEC-331 S3.3d, which could not start while any
PROJECT using `Core.Http` emitted PHP that fatalled.

## Decisions Log

- [2026-08-23 09:10] AGREED: fix by **central `use \Main\<Class>;` aliasing** in the trailing global
  `namespace { }` block — NOT by extending the per-family `\Main\` qualification that
  `emit_json_helpers` already does. The per-family form is what failed: it was applied to Json and
  never carried to Http, Regex, Decimal or Session, and it would have to be remembered again for
  every prelude added later.
- [2026-08-23 09:10] AGREED: alias **classes/interfaces/traits/enums+variants only, never functions.**
  The helper bodies call PHP builtins bare (`count`, `strlen`, `implode`) and a
  `use function \Main\count;` would hijack them. Class aliases carry no matching hazard: the helpers
  spell every builtin CLASS fully qualified (`\RuntimeException`, `\OutOfRangeException`, `\Closure`),
  and the global block declares no classes of its own [Verified by grepping the emitted PHP].
- [2026-08-23 09:10] AGREED: the gate is a real **example PROJECT**, not a golden-text unit test —
  `examples/project/preludes/`, permanently covered by `all_example_projects_transpile_and_match_php`.
  It exercises three prelude families so the fix is proven generic rather than shaped to the one
  fatal first observed.

## The defect

The namespaced (multi-file / project) emit buckets injected prelude classes into `namespace Main {}`,
but their `__phorj_*` runtime helpers are emitted into the trailing global `namespace { }` block,
where they name those classes **unqualified**. PHP resolves the bare name against the global
namespace and fatals.

```
PHP Fatal error:  Uncaught Error: Class "RequestBody" not found
#0 …(450): __phorj_http_parse_request('GET /orders HTT...')
#1 …(504): Main\Request::parse('GET /orders HTT...')
```

**Control:** the identical program as a FLAT single file transpiles and matches the interpreter byte
for byte. That is why no test caught it — every `Core.Http` example is a flat single file, and no
example project imported a prelude that ships helpers.

**Blast radius:** every injected prelude with a `__phorj_*` helper — Http, Regex, Json, Decimal,
Session — used from any multi-file project. Json alone was already correct, via the per-family
`\Main\` prefix in `emit_json_helpers`.

## The fix

`src/transpile/program_emit.rs`, `emit_program_namespaced`: emit `use \Main\<name>;` for every
non-function entry of the already-computed `main_names` vec at the TOP of the global block, before
the bootstrap statement (a `use` only binds names that follow it). Gated on
`split != SplitPass::File`, matching the helper-emission condition — a per-file split pass emits no
helpers, so it needs no aliases.

This is the same mechanism DEC-325 already uses to alias Main-bucket names into each **non-Main**
package block; the global block was simply never given the same treatment.

## Verification

| Step | Evidence |
|---|---|
| Reproduced | `examples/project/preludes/` added → `all_example_projects_transpile_and_match_php` fails with `Class "RequestBody" not found`, the exact stated fatal |
| Fixed | same test green; interpreter and PHP legs byte-identical |
| Sabotage | delete the alias loop → the project oracle goes red with the same fatal; restore byte-for-byte |
| Not a regression | `all_example_projects_match_between_backends` green throughout (the defect was PHP-leg only) |

## Out of scope, recorded not waived

`__phorj_reflect_of` and `__phorj_debug_enums` key their static tables on `get_class($v)` **string**
values. Under namespaced emission `get_class` returns `Main\X` while the table key is built from the
bare Phorj name, so those two lookups look like they should miss — a sibling of this defect that
`use` aliasing cannot fix, because a string is not a resolved name. Sites that strip the prefix
explicitly (`__phorj_round_mode`, `__phorj_class_name`, the `Secret` redaction) are already
namespace-safe [Verified: read in the emitted PHP].

**This sibling is [Inferred] from reading the emitters, NOT measured — deliberately recorded at that
grade rather than upgraded by assumption.** Two attempts to build a probe project that dumps an enum
were spent on language-surface errors (`unknown identifier` on the bare-variant form) rather than on
the question, and the attempt was stopped rather than allowed to consume the slice. What IS
established: **no example project exercises either table** — `phg transpile` of
`examples/project/shapes` emits neither `__phorj_debug_enums` nor `__phorj_reflect_of`, because both
are gated on use [Verified: grepped the emit, zero hits]. So whatever their behaviour is, nothing
currently gates it. That coverage hole is the real finding; it is recorded in KNOWN_ISSUES for a
slice of its own.
