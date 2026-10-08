---
name: domain-phorj-php-bridge
description: Use when a phorj task lifts or transpiles - phg lift, the lifter, scout lifts, PHP to Phorj drafts, preg_match/preg_match_all captures, phg transpile, emitted PHP, __phorj_* helpers, the PHP oracle (php -n, PHORJ_REQUIRE_PHP, toolchain.env, PCRE2, bcmath), a native's PHP twin, E-TRANSPILE-* quarantine, declare interop, reserved PHP names, the http prelude, serve, response headers; paths src/lift/**, src/transpile/**, src/native/**, src/cli/http_prelude.rs, examples/lift/**. How an expert works here - ladder, oracle, traps, evidence.
---

Review date: 2026-10-08 22:37   Validation mode: advisory   Core: .claude/rules/expertise-core.md
Delta over CLAUDE.md: the PHP 8.5 floor, oracle resolution (toolchain.env vs CI), PCRE2 split, draining-pipe probe, Invariants 14, 16 and 17 are NOT repeated (CLAUDE.md § Toolchain & quality gate; § Delivery invariants).

## Roles and mental models
- **Bridge builder, not a runtime delegate**: every native has a real Rust `eval`; the `php:` closure is a peer emission target. Never "delegate to PHP and fault on Rust", never weaken security or correctness for byte-identity.  [Source: memory/transpile-is-a-bridge-not-a-runtime.md]
- **Emitter**: "Phorj : PHP :: TypeScript : JavaScript" (D-L9); use PHP's natural construct per position (`match`, ternary, `?? throw`, two-arg `clone`); a `__phorj_*` helper is the safe fallback where an operand kind is unknown, never the default.  [Source: docs/adr/0002-erasure-not-monomorphization.md:10-12; memory/transpile-modernization-and-helper-elimination.md]
- **Lifter**: `phg lift` produces a best-effort DRAFT for human review; draft quality is judged by `phg check` on the draft, not by lift success; count a rewrite's real reach on the target corpus (scout) before claiming it.  [Source: README.md:311-315; memory/resume-2026-09-24-row-5g.md; memory/l4-named-tuples-gate-lessons.md]

## Standards and norms
| Rule | Applies when | Source | Last checked |
|---|---|---|---|
| Emitted PHP is ONE self-contained file of brace namespaces plus a nameless bootstrap; no Composer, no autoloader (PSR-4 cannot autoload free functions); native calls fully qualified with a leading `\` | any emitter change | [Source: docs/adr/0004-single-file-brace-namespace-php.md:15-21] | 2026-10-08 |
| The oracle runs `php -n`: tier-1 functions (standard, PCRE, json) only in core stdlib; tier-2 (mbstring, ctype, tokenizer, fileinfo) FORBIDDEN; tier-3 (gd, curl, intl, pdo) only in a declaring, guarding extension module. UTF-8 validity via `preg_match('//u', $s) === 1` | emitting any PHP function | [Source: docs/specs/UNIFIED-SPEC.md:1746-1754] | 2026-10-08 |
| Default-deny guard `transpiled_examples_use_only_tier1_php_functions` over the `TIER1_PHP` list; BCMath has no substitute, so the oracle loads it with `-d extension=bcmath`; fatals go to stderr via `-d display_errors=stderr` | adding a PHP function | [Source: tests/differential.rs:4004; tests/differential.rs:3580; tests/conformance.rs:63-65] | 2026-10-08 |
| Ladder mechanics: impure natives (`pure: false`) are auto-quarantined by `uses_impure_native`; live native-only codes include `E-CONCURRENCY-NO-PHP`, `E-TRANSPILE-DB`, `E-TRANSPILE-MAIL`, `E-TRANSPILE-HTTPCLIENT`, `E-FOREIGN-RUNTIME` | a native with no faithful PHP | [Source: src/native/file.rs:93; src/cli/explain/transpile_di.rs:134-169; src/ast/decls/functions.rs:39] | 2026-10-08 |
| `Core.Database` was quarantined (`E-TRANSPILE-DB` fires before any emitter); the developer has since ruled "go for case 1", a module-wide move. Check the register before assuming either state | DB on the PHP leg | [Source: docs/specs/UNIFIED-SPEC.md:2226-2231] | 2026-10-08 |
| `Html.text` escaping is pinned to `htmlspecialchars($s, ENT_QUOTES, 'UTF-8')` (PHP defaults changed across versions); the Rust side replicates the 5-char table with `&` first | html output | [Source: docs/specs/UNIFIED-SPEC.md:1509-1516; src/native/html.rs:5] | 2026-10-08 |
| `Regex.compile` = linear `regex` (the subset PHP `preg_*` matches identically); PCRE-only literal syntax is `E-REGEX-UNSUPPORTED` at check time; `compileBacktracking` (fancy-regex) uses PHP's default 1,000,000 backtrack budget; patterns are raw strings `r"..."` because `{n}` collides with interpolation | regex work | [Source: docs/specs/UNIFIED-SPEC.md:1610-1611; src/checker/calls/regex.rs:6; src/ext/regex/engine.rs:64-65; memory/consolidation-session-2026-09-02-gotchas.md] | 2026-10-08 |
| Lift: PHP `/` and `/=` lift to FLOAT division (DEC-529); a `preg_match` filling a captures array lifts to `RegexMatch? m = Regex.first(...)` (DEC-554) | lifter arithmetic and regex | [Source: src/lift/lifter/division.rs:1; src/lift/lifter/preg/captures.rs:1-6] | 2026-10-08 |
| Foreign interop: `declare function`/`declare class` are type-checked, emit no PHP and make the program PHP-only (`E-FOREIGN-RUNTIME`); lifted-vendor `--vendor=stub` is the ruled opt-in, check its build state | foreign PHP symbols | [Source: src/parser/items/decls/declares.rs:1-6; src/lift/project/mod.rs:27] | 2026-10-08 |
| Response-header injection is guarded in the phorj PRELUDE (`HeaderSafety`, at `withHeader`, the `Cookie` constructor and, since 2026-10-08, every line in `serialize()`), never in Rust `respond_once`, because `phg build --php` never runs `respond_once` and the PHP leg would stay exploitable. Rejected: CR, LF, NUL in values, `:` in names | serve, Http, cookies | [Source: docs/specs/UNIFIED-SPEC.md:2488-2499; docs/specs/UNIFIED-SPEC.md:2514-2520; src/cli/http_prelude.rs:36-44] | 2026-10-08 |

## Tools of the trade
- `phg transpile <file>` and `phg lift <dir> -o <outdir>`; the golden `examples/transpile/demo.php` (regenerate with `phg transpile`); lift fixtures under `examples/lift/` (paired `.php`/`.phg`).  [Source: src/cli/help.rs:96; memory/transpile-modernization-and-helper-elimination.md]
- Fault-parity, two lenses per native: (1) exit status: transpile a minimal fault trigger and run it under the oracle; non-zero is consistent, zero is a divergence needing a throwing `__phorj_*` guard; (2) the message body via `agree_err_php`.  [Source: memory/fault-parity-pass-technique.md; tests/differential.rs:378]
- A Docker `php:8.5-cli` (or `php:8.3-cli` for CI's PCRE2) oracle reproduces CI when a local build masks a gap.  [Source: memory/transpile-no-ini-extensions.md]

## What "good" looks like
- A native lands with `src/native/<m>.rs`, a registry row, `<m>_tests.rs` pinned to real `php -n` output, a gated example and a README row; only a byte-identical PHP CORE function earns a faithful twin.  [Source: memory/native-stdlib-wave.md]
- A lifted draft passes `phg check`, runs, and transpiles back (`lift` then `check` then `transpile` is the QA "lift" journey).  [Source: .claude/skills/phg-qa-sweep/SKILL.md:89]
- Every byte-identity claim (README, CHANGELOG, doc comment, test name) carries the exclusion caveat.  [Source: .claude/agents/safety-promises-reviewer.md:74-75]

## Classic failure modes (with detection)
- **OPEN SECURITY ITEM (DEC-568, PENDING)**: neither the builder guard nor the `serialize()` line scan rejects (a) a raw header line starting with a space or tab, which an obsolete line-folding parser joins to the previous header; (b) a raw line with no `:`; (c) a handler-supplied `Content-Length` or `Transfer-Encoding` line, which `serialize()` emits after its own `Content-Length`, so a proxy may frame the body differently from the client (request-smuggling class). None needs CR/LF; widening the rejected set is a language/API decision for the developer. Report it at once if a task touches serve; do not widen it alone.  [Source: docs/specs/UNIFIED-SPEC.md:2522-2527; KNOWN_ISSUES.md:1436-1442]
- **PHP extension masking** (three times: mbstring, BCMath, ctype): a local PHP with the extension compiled in survives `php -n`; CI's shared extension is stripped and the example fatals with "undefined function". Detect: the tier-1 guard, or `phg transpile` every example and grep for `ctype_`, `mb_`, `iconv`.  [Source: memory/transpile-no-ini-extensions.md; tests/differential.rs:4004]
- **Silent PHP success where phorj faults** (DEC-255 found seven): wrapped overflow promotes to float, an out-of-bounds list or map read returns null with a Warning. Fix shape: a checked `__phorj_*` helper, and fault programs run through PHP too.  [Source: memory/session-2026-07-16-full-reopen-audit.md; src/native/list_registry.rs:97]
- **PHP array `<=>` is count-first** (`[2] <=> [1,1]` is -1), hence tuple-only ordering.  [Source: docs/specs/UNIFIED-SPEC.md:1172-1179]
- **Reserved names**: PHP reserves `int`, `float`, `bool`, `null`, `string`, `void`, `iterable`, `object`, `mixed`, `never`, `self`, `parent`, `static` as class names even inside a namespace; enum variants get a transpiler-only scoped name (`php_scoped_variant_name`); `package Main` functions are GLOBAL PHP functions, so a name like `serialize` collides with a builtin.  [Source: memory/core-json-and-injected-types.md; src/transpile/names.rs:29; memory/m6-web-capabilities-direction.md]
- **Wrapper-node hole in the lifter**: a new `PhpExpr::Declared` wrapper passed rustc but a `matches!` on the unwrapped variants stopped hoisting; a lifter scope registry needs save/restore at every PHP scope boundary. Detect: grep `matches!(` arms naming the wrapped variant; probe each scope boundary.  [Source: memory/wrapper-variants-and-scoped-registries.md]
- **Lift fixtures red for the wrong reason**: the docblock parser reads ONE tag per line; a probe that never reaches the rewrite proves nothing. Read the failure, then the fixture.  [Source: memory/lift-fixture-traps.md; memory/lift-probe-reachability.md]
- **`E-UNUSED-IMPORT` is a LOADER check over the raw file**: it never sees prelude injection, and the loose path `cmd_check(src)` never reports it. Verify on the binary with a file.  [Source: src/loader/import_hygiene.rs:2; src/cli/pipeline.rs:504; memory/consolidation-session-2026-09-02-gotchas.md]

## Evidence surfaces (what counts as proof here)
- `all_examples_transpile_and_match_php` and `all_example_projects_transpile_and_match_php` under `PHORJ_REQUIRE_PHP=1` (CLAUDE.md § Toolchain & quality gate); the tier-1 guard; `tests/conformance.rs`; `tests/interop.rs` for foreign-runtime goldens.  [Source: tests/differential.rs:3458; tests/differential.rs:3550; tests/interop.rs:144]
- Transpile snapshot tests per ruled spec, and lift round-trip cases (lift, run, transpile).  [Source: docs/specs/2026-07-23-typed-lsb.md:50-53; docs/specs/2026-07-23-labeled-break-continue.md:54-59]

## Reviewer lenses
1. **Ladder case** - which case does each new native take, is `pure: false` set, does the emission use only tier-1 functions, is there a disclosure?  2. **Downgrade** - does a hard error become a warning, fallback or best-effort emit (a P0)?  3. **Stock oracle** - does `php -n` 8.5 print identical bytes, faults included?  4. **Security asymmetry** - is a guard on one deployment path (native serve vs `phg build --php`) missing on the other?  [Source: .claude/agents/safety-promises-reviewer.md:65-77; docs/specs/UNIFIED-SPEC.md:2495-2499]

## Vocabulary
| Term | Meaning |
|---|---|
| ladder case 1/2/3 | faithful transpile / native-only `E-TRANSPILE-*` plus quarantine / forbidden silent downgrade |
| quarantine | the program is excluded from the PHP leg of the differential |
| tier-1/2/3 | PHP function availability under `php -n` (one of three unrelated tier systems; the others are stdlib determinism and extension Mandatory/Default/OptIn) |
| draft | lifter output that needs human review |
| foreign runtime | code that needs the PHP runtime (`declare`, lifted vendor) |
[Source: docs/specs/UNIFIED-SPEC.md:1429-1441; docs/specs/UNIFIED-SPEC.md:1746-1750; docs/EXTENSIONS.md:11-35]

## Canonical sources
- docs/adr/0004-single-file-brace-namespace-php.md, docs/specs/UNIFIED-SPEC.md (PHP-function tiers, regex, Html, serve hardening), docs/EXTENSIONS-AUTHORING.md (ladder for natives), KNOWN_ISSUES.md, tests/differential.rs. Read 2026-10-08.
