# M — Bidirectional Gap Matrix: PHP 8.5 ↔ Phorj

> **Agent M output — full-audit fleet, Stage 2.** Inputs: `D-php-surface.md` (869 rows: 173 SYN,
> 631 FN, 20 RT, 45 DEF) and `E-phorj-surface.md` (code-verified at `ccb2403`). Where a verdict
> needed confirmation beyond E, it was checked against source or the built binary
> (`phg 1.0.0-nightly.0`) — those rows carry `[Verified: …]`. Roadmap citations refer to
> `ROADMAP.md` and `docs/specs/2026-06-21-php-parity-and-beyond.md` (the parity SSOT, "§3 rollup"
> / "§5 reject-list").
>
> **Verdict key:** `CB` COVERED-BETTER · `CE` COVERED-EQUAL · `P` PARTIAL · `GP` GAP-planned ·
> `GU` GAP-unplanned · `GD` GAP-by-design · `N/A` meaningless in Phorj's model.
>
> **Spot-verifications run for this matrix** (binary `target/release/phg`):
> lambda keyword is `function(int x) => e` (`fn` is retired — E's PJ-SYN-005 wording is stale);
> `String.length("héllo")` = **6 (bytes)**; trailing comma in calls accepted; top-level `const`
> rejected; `catch (A | B e)` parses (union multi-catch); `s[i]` string indexing rejected;
> `break 2` rejected; `int...` variadics rejected; float `==` type-checks (no lint yet).

---

## PASS 1 — PHP → Phorj (can Phorj do everything PHP does?)

### 1.1 SYN rows (173, one verdict each)

| ID | Verdict | Phorj counterpart / justification |
|---|---|---|
| SYN-001 | N/A | Phorj is not an HTML-embedded template language; typed `html"…"` (PJ-KW-004) is the deliberate alternative |
| SYN-002 | N/A | same |
| SYN-003 | N/A | same |
| SYN-004 | CE | `Output.print/printLine` (PJ-NAT-OUTPUT); interpolation replaces multi-arg echo |
| SYN-005 | CE | `;` + `{}` blocks (PJ-SYN-002) |
| SYN-006 | P | `//` and `/*…*/` [Inferred: fmt comment side-channel]; `#` line comment absent (bare `#` not a token, PJ-KW-003); `#[` attributes match PHP |
| SYN-007 | CB | always-strict; no per-file coercion switch (removes the DEF-019 config-changes-semantics class) |
| SYN-008 | N/A | no tick model |
| SYN-009 | N/A | UTF-8 source only |
| SYN-010 | CB | static `package`/`import` model (PJ-SYN-010, PJ-PROJ-002); no runtime file inclusion (fixes DEF-020) |
| SYN-011 | CB | same |
| SYN-012 | N/A | no include |
| SYN-013 | GD | closed no-eval language (§5: E-importer-stageC rationale — "dynamic PHP is un-importable into a closed no-`eval` language") |
| SYN-014 | P | `main(List<string> args) -> int` exit codes + `panic()` abort; no arbitrary mid-program `exit(status)` |
| SYN-015 | GD | no shell-exec operator (determinism spine + injection class) |
| SYN-016 | GU | `#` not a token; `phg build` binaries are the scripting answer (minor) |
| SYN-017 | N/A | data-after-code done properly: versioned CRC `.phorj` section container (PJ-CLI `build`) |
| SYN-018 | CB | sigil-free, uniformly case-sensitive + enforced casing `E-NAME-CASE`/`E-TYPE-CASE` (fixes DEF-018) |
| SYN-019 | GD | §5 reject A-compact-extract (`$$x`) |
| SYN-020 | CB | typed first-class function values (PJ-SYN-005) replace string dispatch; dynamic `$o->$m()` deliberately absent |
| SYN-021 | GD | static construction only (`E-NEW-REQUIRED`); Core.Reflection is read-only introspection |
| SYN-022 | GD | §5 reject A-func-static (fixes DEF-010) |
| SYN-023 | GD | same reject; class `static` fields cover persistent state |
| SYN-024 | P | class constants only (PJ-OOP-001) [Verified: top-level `const MAX = 10;` is a parse error] |
| SYN-025 | GD | constants/config must be compile-time (memory: config-must-be-compile-time; fixes DEF-019) |
| SYN-026 | GU | no `__LINE__`/`__FILE__` surface (spans exist internally only) |
| SYN-027 | CB | `Environment.get/all`, `Process.arguments`, Http `Request` object — read-only, typed (fixes DEF-010/DEF-045) |
| SYN-028 | N/A | no globals to restrict |
| SYN-029 | N/A | no http stream wrapper |
| SYN-030 | P | `Math.pi/e/infinity/nan`; no `PHP_INT_MAX`/`PHP_EOL`/OS-constant surface |
| SYN-031 | CB | `0x`/`0b`/`0o` + `_` (PJ-KW-005); legacy silent-octal `0755` removed |
| SYN-032 | CB | floats + exponents, plus `decimal` `19.99d`; int overflow is a checked fault, not silent float promotion (fixes DEF-022) |
| SYN-033 | CE | raw strings `r"…"`/`r#"…"#` (PJ-KW-004) |
| SYN-034 | CE | `\n \t \u{…}` escapes (PJ-KW-004) |
| SYN-035 | CB | one uniform `{expr}` interpolation, span-tracked; no unquoted-key quirks |
| SYN-036 | CE | `{expr}` |
| SYN-037 | N/A | never existed |
| SYN-038 | CE | `"""…"""` text blocks with JEP-378 auto-dedent (PJ-KW-004) |
| SYN-039 | CE | raw strings |
| SYN-040 | CB | `[…]` List, `[k => v]` Map, `Set` — distinct typed collections (fixes DEF-005) |
| SYN-041 | CE | case-sensitive keywords |
| SYN-042 | CB | one typed closure value; no string/array callables (PJ-SYN-005) |
| SYN-043 | CB | checked `x as T` → `T?` + explicit `Core.Conversion` matrix; `discard` = `(void)` cast (fixes DEF-041) |
| SYN-044 | CE | 17 built-in type words + unions/intersections/optionals/function types (PJ-TY-003); `mixed`/`callable` absent by design; `iterable` — see SYN-162 |
| SYN-045 | CB | no implicit coercion at all (fixes DEF-004) |
| SYN-046 | CB | explicit `String.parseInt/parseFloat -> T?` |
| SYN-047 | CB | typed Map keys, `"1"` ≠ `1` (`E-MAP-KEY`; fixes DEF-028) |
| SYN-048 | CB | typed handles (Channel/Task/Regex/Secret classes), no opaque resources (PJ-RT-001) |
| SYN-049 | CB | checked overflow/div-zero faults (PJ-RT-003); `**`; `Math.integerDivide` |
| SYN-050 | CE | statement-only `x++`/`x--` (PJ-SYN-002); no string-increment quirk (fixes DEF-029) |
| SYN-051 | CE | string concat via `+`/`Op::Concat` [Inferred]; no precedence trap |
| SYN-052 | P | `+= -= *= /= %= ??=`; no `**=`, no bitwise compounds `&= \|= ^= <<= >>=` |
| SYN-053 | CB | single typed `==` — no loose/strict split (fixes DEF-004); `<=>` absent (`List.sortWith` covers; J-spaceship in §3 M-RT rollup) |
| SYN-054 | CB | `&& \|\| !` only; low-precedence `and/or/xor` assignment trap removed |
| SYN-055 | CE | `& \| ^ ~ << >>` (int-only; `bytes` type replaces string byte-ops) |
| SYN-056 | CB | expression-`if` with mandatory `else`; no nested/short-ternary traps (fixes DEF-017) |
| SYN-057 | CE | `??`/`??=` on typed optionals |
| SYN-058 | CE | `?.` |
| SYN-059 | GD | §5/GA-M12 V-error-suppression-stance (fixes DEF-007) |
| SYN-060 | CB | `instanceof` with smart-cast flow narrowing (PJ-SYN-007); no dynamic string RHS |
| SYN-061 | CB | explicit `Map.merge`/`List.concat` semantics (fixes DEF-026) |
| SYN-062 | CE | `x \|> f` shipped in M3 S3 — pre-dates PHP 8.5's pipe |
| SYN-063 | GP | A-variadics/spread (§3 M-RT ergonomics) [Verified: `int...` parse error] |
| SYN-064 | P | `List.concat`/`flatten` cover; no literal `...` splat |
| SYN-065 | GD | §5 reject A-references (fixes DEF-006) |
| SYN-066 | CE | `obj with { f = v }` (PJ-SYN-004 CloneWith) — Phorj shipped 8.5's clone-with first |
| SYN-067 | P | `s[i]` rejected [Verified: "type `string` cannot be indexed"]; `String.substring` + for-in over string cover |
| SYN-068 | CE | `Index` on arbitrary expressions (PJ-SYN-004) [Inferred] |
| SYN-069 | CE | `if/else if/else`; no alternative endif syntax (by design) |
| SYN-070 | CE | `while` + `do…while` (parser/stmts.rs:659) |
| SYN-071 | CE | `CFor` `for(;;)` (PJ-SYN-002) |
| SYN-072 | CB | for-in/`foreach (e as x)` over list/range/string/Map two-binding; no by-ref dangling-reference pitfall (fixes DEF-006) |
| SYN-073 | GD | §5 reject A-switch (fall-through footgun); exhaustive `match` covers (fixes DEF-027) |
| SYN-074 | CB | compile-time exhaustiveness + payload/type/struct patterns + guards (PJ-SYN-006) vs PHP's value-only runtime `UnhandledMatchError` |
| SYN-075 | P | single-level only [Verified: `break 2` parse error]; B-labeled-break in §3 M-RT ergonomics → the labelled part is GAP-planned |
| SYN-076 | GD | no `goto` |
| SYN-077 | CB | totality-checked (`E-MISSING-RETURN`) |
| SYN-078 | P | `throw` is a statement (PJ-SYN-002); no throw-in-`??`/ternary expression form |
| SYN-079 | CB | compile-time checked `throws` + multi-catch via union type [Verified: `catch (E1 \| E1)` parses, dies on E-UNION-ARITY dup only] + `finally` |
| SYN-080 | GP | generators/`yield` — marathon A2 (memory: session-naming-and-b1 "NEXT=A2"); `yield` currently only PHP-reserved-guarded |
| SYN-081 | CB | uncolored green threads: `spawn`/`Channel`/`Task.join`, deterministic identical scheduling on both backends (PJ-SYN-009, PJ-RT-004) |
| SYN-082 | CB | case-sensitive + real overloading (PJ-OOP-002 — PHP has none); no conditional/nested declaration (by design) |
| SYN-083 | P | literal defaults on free functions only (`E-DEFAULT-PARAM-CONTEXT` bans method/ctor defaults; no `new` in defaults) |
| SYN-084 | CB | types mandatory and always enforced — no strict_types gate |
| SYN-085 | CB | `T?` with compile-time strictNullChecks (PJ-SYN-007); PHP nullable is runtime-only |
| SYN-086 | CB | unions + exhaustive match-over-union + narrowing (PJ-TY-006) — PHP unions have no narrowing |
| SYN-087 | CE | `A & B` (PJ-TY-006, transpiles to PHP 8.1 `A&B`) |
| SYN-088 | CE | `A \| B & C` — `&` binds tighter (PJ-TY-002), DNF expressible |
| SYN-089 | N/A | literal types are PL nicety; `bool`/`T?` cover the use |
| SYN-090 | P | `void` ✓, `never` ✓, `empty` extra (PJ-TY-004); `mixed` absent by design; `static`/`self` return types absent |
| SYN-091 | GP | A-variadics (§3) [Verified: parse error] |
| SYN-092 | GP | A-named-args (§3 M-RT ergonomics) |
| SYN-093 | GD | §5 reject A-references |
| SYN-094 | CE | `function(int x) -> T { … }` lambdas, by-value capture (simpler than `use()`) [Verified] |
| SYN-095 | CE | `function(int x) => e` expression body [Verified: runs; `fn` keyword retired — E PJ-SYN-005 wording stale] |
| SYN-096 | CB | a bare named-fn reference *is* a value — no `(...)` syntax needed |
| SYN-097 | GD | no `this`-rebinding (`bind`/`bindTo`/`call` are the dynamic-scope footgun class Phorj removes) |
| SYN-098 | CE | [Verified: `f(1, 2,)` runs] |
| SYN-099 | N/A | `T?` explicit from day one |
| SYN-100 | CB | must-use is the **default** (`E-UNUSED-VALUE`) + `discard`; PHP's `#[NoDiscard]` is opt-in per function |
| SYN-101 | CB | module loader resolves functions statically (fixes DEF-032) |
| SYN-102 | CE | mandatory `new` (`E-NEW-REQUIRED`); dynamic-expression class names absent by design |
| SYN-103 | CB | definite-assignment analysis (`E-FIELD-UNINITIALIZED`) — no runtime "uninitialized typed property" state |
| SYN-104 | CE | static fields/methods + visibility (PJ-OOP-001) |
| SYN-105 | CB | dynamic properties never existed; unknown field = compile error |
| SYN-106 | CB | immutable-by-default, `mutable` opt-in — the correct default; PHP had to bolt `readonly` on |
| SYN-107 | GP | A-asym-vis (§3 M-RT class surface) |
| SYN-108 | CE | property hooks `T name { get => e; set(v) {…} }` (PJ-OOP-001) |
| SYN-109 | CE | `constructor(public T x)` promotion |
| SYN-110 | CE | class consts: literal init, visibility, `E-CONST-*`; no `final const`/dynamic fetch (latter by design) |
| SYN-111 | P | explicit `parent.m()`/`parent(A).m()` (PJ-OOP-001); late static binding absent (A-lsb, §3) |
| SYN-112 | CB | **multiple** inheritance with explicit conflict errors (`E-MI-*`) + final-by-default `open` opt-in + `abstract` |
| SYN-113 | P | overrides checked but invariant (`E-OVERRIDE-SIG`); no co/contravariance |
| SYN-114 | CE | interfaces: multi-`extends`, nominal subtyping, `instanceof` RHS (PJ-OOP-003) |
| SYN-115 | CE | traits with methods/state/ctors/abstract-reqs/hooks + `use/rename/exclude`; conflicts are errors (arguably better than `insteadof`) |
| SYN-116 | CB | payload variants + generic enums (`Option<T>`/`Result<T,E>`) — PHP enums carry no payload; backed-enum sugar (`->value`/`cases()`) GAP-planned (A-backed-enums, §3) |
| SYN-117 | GU | no anonymous classes |
| SYN-118 | P | `#[Route]` only; no user-defined attributes (`E-UNKNOWN-ATTRIBUTE`) — **→ [HEAD `af3aad3`: improved but STILL PARTIAL — user-defined attributes are now declarable + applyable with compile-time-type-checked args (DEC-194, git `bf05648`/`451fb89`), which is better than PHP on the targets it supports. But narrower than PHP's feature: attributes attach only to classes + free functions (2 of PHP's 7 targets — no method/property/param/const/enum; AST `attrs` on `ClassDecl`/`FunctionDecl` only), and are "inert metadata until a later slice reads them via reflection" — attribute-**reflection**, the primary purpose, does not exist yet. Verdict unchanged (P). NOT counted as a mover in §4.6.]** |
| SYN-119 | P | the *capabilities* are language defaults (Override→`E-OVERRIDE-SIG` always-on; NoDiscard→`E-UNUSED-VALUE` default; SensitiveParameter→`Secret<T>` typed); userland `#[Deprecated]` absent |
| SYN-120 | P | `Reflection.className/typeName` (PJ-NAT-REFLECTION) |
| SYN-121 | CB | explicit value(COW)/handle split + single structural `==` (mutation milestone; fixes DEF-024's invisibility) |
| SYN-122 | GU | lazy objects = DI-framework machinery |
| SYN-123 | GU | no weak refs (Rc model, no tracing GC) |
| SYN-124 | GP | A-magic-stringable (§3 M-RT class surface) |
| SYN-125 | CE | `constructor` |
| SYN-126 | GD | §5 reject A-destruct (Rc/Drop has no deterministic finalization; also fixes DEF-035) |
| SYN-127 | GD | §5 reject A-magic-dynamic |
| SYN-128 | GD | same |
| SYN-129 | GD | same; property hooks cover computed properties |
| SYN-130 | GD | same; `set` hooks |
| SYN-131 | GD | §5 reject A-isset-empty |
| SYN-132 | GD | same |
| SYN-133 | N/A | no native object serialization (legacy hook) |
| SYN-134 | N/A | same |
| SYN-135 | GD | no serialize mechanism at all (fixes DEF-013 RCE class); `Core.Json` is the data path |
| SYN-136 | GD | same |
| SYN-137 | GP | A-magic-stringable (§3) |
| SYN-138 | GP | A-magic-invoke (§3 M-RT class surface) |
| SYN-139 | N/A | no `var_export` |
| SYN-140 | N/A | `with { }` clone-update covers the use without a hook |
| SYN-141 | GU | `inspect::render` exists but no user hook |
| SYN-142 | CB | mandatory packages, folder=path enforced (`E-PKG-PATH`), PascalCase (`E-PKG-CASE`) |
| SYN-143 | CE | `import` + `as` + `import type`; no group-use/wildcard (by design — PHP has no `use A\*` either for the type case) |
| SYN-144 | CB | fully static resolution, no runtime global fallback (fixes DEF-033) |
| SYN-145 | N/A | no namespace operator needed |
| SYN-146 | CB | whole-project loader, functions included (fixes DEF-032) |
| SYN-147 | CE | contextual-keyword machinery + `E-RESERVED-NAME` PHP-interop guard (PJ-KW-006) |
| SYN-148 | CB | three deliberate tiers: checked `throws` / `Result` / uncatchable faults (PJ-SYN-008) |
| SYN-149 | CB | 12 fault classes, byte-identical traces (PJ-RT-003); engine errors are deliberately uncatchable bugs (the anti-Java decision) |
| SYN-150 | P | user-defined `Error` classes; no shipped standard exception taxonomy |
| SYN-151 | N/A | no warning/exception split brain to bridge (fixes DEF-008) |
| SYN-152 | CB | compile-time E-/W- codes + `phg explain`; nothing runtime-configurable |
| SYN-153 | GD | faults crash with traces; `serve --dev` renders errors; no global mutable handlers |
| SYN-154 | CE | `panic()`/`todo()`/`unreachable()` intrinsics |
| SYN-155 | CB | `assert()` **never stripped**, byte-identical across profiles (PJ-CLI-003); PHP compiles asserts out in prod |
| SYN-156 | P | stack traces ✓ run≡runvm; cause-chain GAP-planned (A-fault-cause-chain, §3 M-faults) |
| SYN-157 | GU | no atexit/shutdown hooks |
| SYN-158 | CB | escape-from-main is a **compile** error (`E-UNCAUGHT-THROW`); runtime faults exit nonzero with trace |
| SYN-159 | CB | asymmetry made explicit & documented (List/Map/Set COW values, Instance handle) + immutable default |
| SYN-160 | GD | §5 reject A-isset-empty; optionals + explicit `isEmpty` (fixes DEF-014) |
| SYN-161 | CB | typed struct/list destructuring with mandatory diverging `else` on refutable patterns (fixes DEF-036) |
| SYN-162 | GP | A-iterators / J-iter-protocol (§3 M11); generators = marathon A2 |
| SYN-163 | CB | typed keys (fixes DEF-028) |
| SYN-164 | CB | uniformly case-sensitive + enforced conventions (fixes DEF-018) |
| SYN-165 | CB | `0o` only |
| SYN-166 | CB | zero runtime config (config-must-be-compile-time; fixes DEF-019) |
| SYN-167 | CB | `Response` value object; serialization is explicit (fixes DEF-042) |
| SYN-168 | N/A | persistent-process model (`phg serve`) is the deliberate opposite; PHP itself is migrating there (see RT-005) |
| SYN-169 | CB | Rc+COW, acyclic by construction, deterministic reclamation; cycle collector deferred to v2 only if mutation needs it |
| SYN-170 | CE | 14 contextual keywords (PJ-KW-002) |
| SYN-171 | N/A | D's own n/a marker |
| SYN-172 | P | methods have `this`; lambdas cannot capture `this` (`E-LAMBDA-THIS`; this-capture a documented deferral) |
| SYN-173 | P | `W-DEPRECATED` side-table for stdlib + DEPRECATION.md policy; userland deprecation attribute absent |

**SYN tally (173):** CB 56 · CE 37 · PARTIAL 20 · GAP-planned 9 · GAP-unplanned 7 ·
GAP-by-design 24 · N/A 20.
**SYN coverage** (COVERED=1, PARTIAL=0.5; N/A+GD excluded): (93 + 10) / 129 = **79.8%**.

### 1.2 FN groups (631 rows — compressed where a family is uniform, itemized where verdicts differ)

Per-group verdict distribution `C/P/GP/GU/GD/NA` (counts sum to the group's row count).
"C" merges COVERED-BETTER and COVERED-EQUAL; better-than-PHP cases are named.

| Group (rows) | C | P | GP | GU | GD | NA | Notes — what's covered, what's named-missing |
|---|--|--|--|--|--|--|---|
| **FN-STR** (93) | 30 | 6 | 9 | 35 | 3 | 10 | Covered: length/contains/starts/ends/indexOf(**int?** — fixes DEF-009)/lastIndexOf/substring/count/replace/repeat/case/capitalize/trim×3/split/join/reverse/pad×2/lines/parseInt-Float-Bool, plus md5/sha1/crc32→`Core.Hash`, htmlspecialchars→**`Core.Html` typed XSS-safe (CB)**, str_getcsv→`Core.Csv`, bin2hex/hex2bin→`Core.Encoding`, number_format→`Math.numberFormat`, strcasecmp→equalsIgnoreCase, strval→`Conversion.toString`. GP: **sprintf family (7 rows, A-sprintf §3 M11)** — **→ [HEAD `af3aad3`: 4 of 7 now COVERED — `Core.String.format` = PHP-`%` sprintf with a compile-time-type-checked directive engine (`%s %d %f %e %E %g %G %x %X %o %b %%` + flags/width/precision/`%N$`-positional; DEC-199, git `9bc6612`…`130b0cb`). It closes sprintf(053)/printf(054, via `Output.print`)/vsprintf(055)/vprintf(056) — its `(spec, list)` calling convention accepts a runtime `List` arg (src/checker/calls.rs:347), so the array-form is expressible without variadics. STILL GP: fprintf/vfprintf(057/058 — need stream handles, FN-FS gap) and sscanf(059 — inverse parse, no `String.scan`). Counted in §4.6.]**, chr/ord (M-codepoint-int, M-text). GD: setlocale/nl_langinfo/strcoll (locale — §5 no-ICU). GU highlights: str_split(fixed-chunk), ucwords, wordwrap, strtr, similar_text/soundex/metaphone/levenshtein, strtok/strpbrk/strspn, str_increment/decrement, strip_tags. N/A: addslashes×5 (no magic-quotes/SQL-string model), utf8_encode/money_format/hebrevc (removed/deprecated), crypt (→ Cryptography) |
| **FN-ARR** (74) | 26 | 12 | 2 | 22 | 2 | 10 | Covered: map/filter/reduce (**uniform subject-first order — fixes DEF-003**), keys/values/has, append/concat/merge, reverse, indexOf/find/contains (**typed-strict — fixes DEF-027**), slice/take/drop, sum/max/min, unique, chunk, fill, count(length/size), range(`a..b` syntax), first/last, any/all, sort/sortWith (**pure, returns — fixes DEF-025**), array_find/any/all/first/last (8.4/8.5 parity), list()→destructuring. P: pop/shift/splice (immutable idiom via slice), diff/intersect (Set.difference/intersection for sets only), asort/ksort family (sortWith composes), array_walk (map covers), SORT_* flags. GP: remaining L-list-breadth (zip — deferred B3). GD: compact/extract (§5 reject). N/A: internal-pointer family (current/key/next/reset/end — no array cursor), each (removed), array_is_list (types make it meaningless) |
| **FN-MATH** (37) | 17 | 3 | 11 | 4 | 0 | 2 | Covered: abs/ceil/floor/round(+`Core.Decimal` RoundingMode — **CB, fixes DEF-023**)/fmod(`%` on floats)/intdiv/pow/sqrt/exp/log/log10/pi/sin-cos-tan/max-min/isNaN-isFinite-isInfinite + rand/mt_rand→`Core.Random` (seeded deterministic). GP: asin/acos/atan/atan2/hyperbolics/hypot/deg2rad/log2/log1p/expm1 (G-math-breadth §3 M11), BigInt/GMP (N-bigint, M-NUM-2). P: constants row (pi/e only), BCMath (decimal covers money, not arbitrary precision), base-conversions (hex literals + Encoding partial). **GAP: random_int/random_bytes CSPRNG** (security-relevant — no crypto-safe source; Random is deliberately deterministic). N/A: lcg_value (deprecated) |
| **FN-PCRE** (11) | 4 | 2 | 0 | 4 | 0 | 1 | Covered: preg_match(matches/find/findGroups), match_all(findAll), replace, split (`Core.Regex`, feature-gated). GU: **preg_replace_callback**(+_array), preg_filter, preg_quote. P: preg_grep (filter+matches composes), modifier surface (regex-crate subset of `i m s x u`). N/A: last_error (typed API) |
| **FN-JSON** (6) | 3 | 0 | 0 | 1 | 0 | 2 | encode/decode/validate via parse→`Json?` + stringify(Pretty) — decode is **CB: typed `Json` enum, `Json.Null` ≠ error (fixes DEF-044)**. GU: JsonSerializable protocol. N/A: last_error/JsonException (optional-return model) |
| **FN-DATE** (27) | 5 | 5 | 2 | 8 | 2 | 5 | Covered: DateTimeImmutable→`Core.Time` Instant/Date (**immutable-only — fixes DEF-011**), DateInterval→Duration, time/microtime/hrtime→nowMilliseconds/monotonicNanos. GP: DateTimeZone/IANA tz + default-timezone (N-tz-iana, M-TIME-2). GD: mutable DateTime (deliberately absent), **strtotime (DWIM parser — fixes DEF-039)**. P: format chars (toIso only), mktime/checkdate (factories partial), date(). GU: DatePeriod, getdate/localtime/gettimeofday, date_parse, sun_info, sleep/usleep. N/A: strftime/strptime/sunrise (deprecated), ~26 procedural aliases (OO-only by design) |
| **FN-FS** (55) | 8 | 2 | 7 | 34 | 0 | 4 | Covered: file_get/put_contents→read(**string?**)/write/append, exists, size, copy, rename, delete, basename/dirname/pathinfo→`Core.Path`. GP: mkdir/rmdir/opendir/scandir/glob/chdir-getcwd/touch (G-dir/G-file-more, M-Batteries §3). GU: **the entire fopen stream-handle family (16 rows)**, stat/perms/chmod/chown/umask, symlinks, realpath, tempnam/tmpfile, fnmatch, disk_*, parse_ini, popen, chroot. N/A: move_uploaded_file, fgetss (removed), include-path, clearstatcache |
| **FN-HASH** (8) | 0 | 1 | 4 | 3 | 0 | 0 | P: `hash()` breadth = only crc32/md5/sha1/sha256. GP: **hash_hmac, hash_equals (timing-safe!), hkdf, pbkdf2** (G-crypto digests, §3 M8). GU: hash_file, streaming HashContext, hash_algos |
| **FN-CRYPT** (8) | 2 | 0 | 0 | 5 | 0 | 1 | Covered: password_hash/verify → `Core.Cryptography` (**CB: Argon2id default, no bcrypt legacy**). GU: needs_rehash/get_info/algos, **sodium (~110 fns), openssl (~60 fns)** — both families entirely absent. N/A: crypt (legacy DES/MD5) |
| **FN-DB** (10) | 0 | 0 | 10 | 0 | 0 | 0 | **Entire database surface absent.** GAP-planned: ROADMAP M6 "Postgres connectivity". No PDO/mysqli/SQLite equivalent exists today — the single largest migration blocker |
| **FN-CURL** (13) | 0 | 0 | 13 | 0 | 0 | 0 | **No HTTP client.** GAP-planned: M6 deferral (determinism gate) + post-M-DX audited developer question (plan `2026-07-01-post-m-dx-four-lane-backlog`); `phg vendor` is the only network-touching code today |
| **FN-MB** (22) | 0 | 2 | 12 | 4 | 0 | 4 | GP: the M-text programme (M-codepoint-len, M-unicode-case, M-grapheme, M-normalization §3) replaces the mb_* second-family model with one correct API — **note: `String.length` is byte-based today [Verified: "héllo" → 6]**. P: case ops (ASCII-correct today). GU: convert_kana, mimeheader, numericentity, detect_encoding. N/A: mb_ereg legacy family (Core.Regex is the answer), http_input/output plumbing |
| **FN-ICONV** (6) | 0 | 0 | 2 | 2 | 0 | 2 | GP: charset conversion contract (M-encoding-contract, M-text). GU: mime encode/decode. N/A: get/set_encoding globals |
| **FN-SPL** (39) | 2 | 2 | 4 | 26 | 0 | 5 | Covered: SplFixedArray→`[T; N]` (**CB: static literal-index bounds**), class_implements/parents→`Reflection.interfaces/parents`. P: SplStack/SplQueue (List covers). GP: Iterator/IteratorAggregate/Traversable protocol (A-iterators §3 M11) + directory iterators (M-Batteries). GU: heaps, priority queue, SplObjectStorage (no object Map keys — HKey is int/bool/string), ArrayAccess protocol, the 12-row iterator-decorator zoo (eager higher-order List fns cover most uses), SplFileInfo/Object family. N/A: autoload fns (no autoloading — superior model), Serializable (deprecated) |
| **FN-CTYPE** (11) | 4 | 0 | 0 | 7 | 0 | 0 | Covered: alnum/alpha/digit/xdigit → `Validation.isAlnum/isAlpha/isInt/isHex`. GU: cntrl/graph/lower/print/punct/space/upper |
| **FN-FILTER** (9) | 0 | 2 | 0 | 3 | 0 | 4 | P: filter_var validate subset (`Core.Validation` int/number; email/URL absent). GU: sanitize filters, flags, filter_var_array. N/A: filter_input family (superglobal-driven) |
| **FN-SESS** (10) | 0 | 0 | 10 | 0 | 0 | 0 | **No session layer.** GAP-planned(deferred): K-auth-csrf-session (§3 M6 deferred web-security). Every stateful web app needs this |
| **FN-STREAM** (15) | 0 | 0 | 0 | 13 | 0 | 2 | No stream/context/wrapper/filter abstraction (the internal `Transport` trait is not user-facing). GU across the board; N/A: sapi_windows rows |
| **FN-SOCK** (10) | 0 | 0 | 0 | 10 | 0 | 0 | No raw socket API (serve owns the socket internally) |
| **FN-XML** (12) | 0 | 0 | 0 | 12 | 0 | 0 | **No XML/DOM/XPath/SimpleXML/XMLReader-Writer at all** (Core.Html is emission-only) |
| **FN-FINFO** (4) | 0 | 0 | 0 | 4 | 0 | 0 | No MIME sniffing |
| **FN-ZLIB** (7) | 0 | 0 | 0 | 7 | 0 | 0 | No compression |
| **FN-ZIP** (1) | 0 | 0 | 0 | 1 | 0 | 0 | No archives |
| **FN-PHAR** (1) | 0 | 0 | 1 | 0 | 0 | 0 | GP: P-phar (§3 M6); `phg build` already covers the self-contained-app use natively |
| **FN-INTL** (18) | 0 | 0 | 6 | 10 | 2 | 0 | GP: grapheme_*, Normalizer, IntlChar subset (M-text S2/S3) + number/date/message formatters (tier-3 extension policy, §3 defer). GD: Collator, Transliterator (§5 — need ICU data, can't honor zero-dep + `php -n` oracle). GU: calendars, timezones-intl, break iterators, Spoofchecker, UConverter, ResourceBundle, idn, ListFormatter |
| **FN-GD** (7) | 0 | 0 | 0 | 7 | 0 | 0 | No image processing (~110 fns family) |
| **FN-REFL** (15) | 0 | 5 | 0 | 5 | 2 | 3 | P: ReflectionClass/Object/Property/Enum introspection subset via `Core.Reflection` kind/className/typeName/fields/methods/parents/interfaces (names only, read-only). GD: ReflectionMethod/Function *invoke* (dynamic dispatch violates the static model). GU: parameter introspection, attributes reflection, ReflectionGenerator/Fiber, extensions. N/A: ReflectionType (types are compile-time), ReflectionReference, base classes |
| **FN-RAND** (4) | 1 | 1 | 0 | 0 | 0 | 2 | Covered: Randomizer→`Core.Random` (**CB for testing: seeded/deterministic by design — the O-deterministic-seam**). P: engines (one PRNG; **no Secure/CSPRNG engine**). N/A: interface/error rows |
| **FN-PROC** (16) | 1 | 1 | 1 | 6 | 6 | 1 | Covered: getenv→`Environment` (read-only — CB). GP: getopt (G-args, M-Batteries). GD: the pcntl fork/signal family (6 rows — green-threads single-threaded model replaces it). GU: **exec/system/proc_open (no subprocess API)**, posix family, getmypid, loadavg. P: set_time_limit (serve `--timeout`). N/A: escapeshell (no shell) |
| **FN-OB** (10) | 1 | 0 | 0 | 0 | 0 | 9 | Covered-better: header surface → Http `Response` value (fixes DEF-042). N/A: the whole ob_* buffer stack — no output-buffer model to control |
| **FN-VAR** (27) | 7 | 4 | 0 | 2 | 3 | 11 | Covered: gettype→Reflection.kind/typeName, intval-floatval-boolval→Conversion/parse (**T? — CB**), is_numeric→Validation.isNumber, get_class→className, method_exists→methods, is_a→instanceof, memory_get_*→`Core.Runtime`. P: var_dump/print_r (debugger + `--dump-on-fault`; no user dump fn — A-printf-debug §3), get_object_vars. GD: serialize/unserialize (fixes DEF-013), get_defined_vars, class_exists (compile-time knowledge). GU: var_export, get_declared_*. N/A: the is_int/is_string/... family (static types make runtime type-tests meaningless), settype, debug_zval_dump |
| **FN-FUNC** (8) | 2 | 0 | 0 | 2 | 1 | 3 | Covered: call_user_func→first-class fn values, Closure. GD: func_get_args (static signatures). GU: forward_static_call, register_shutdown_function. N/A: function_exists (compile-time), ticks, create_function (removed) |
| **FN-URL** (10) | 3 | 0 | 3 | 3 | 0 | 1 | Covered: urlencode/rawurlencode→`Url.encodeForm/encodeUriComponent`, base64→`Encoding`. GP: **parse_url + http_build_query + the 8.5 Uri objects** (G-url §3 M6 — should land spec-compliant, leapfrogging DEF-030). GU: parse_str, get_headers, get_meta_tags |
| **FN-NET** (9) | 0 | 0 | 1 | 8 | 0 | 0 | GP: syslog→G-log/Q-corelog (§3 M11). GU: DNS family, gethostby*, inet_*, fsockopen, **mail()** (DEF-031 — absent, unplanned), ftp family |
| **FN-MISC** (18) | 2 | 1 | 2 | 3 | 3 | 7 | Covered: phpversion→`--version`, token_get_all→`phg tokenize` (of Phorj itself — CB). GP: error_log (G-log), uniqid (G-uuid, deferred). P: debug_backtrace (traces exist; no user API). GD: ini_* (compile-time config), FFI (§5 E-php-ffi reject), extension model. GU: php_uname, cli process title, readline. N/A: phpinfo, gc_*, opcache_* (VM native — fixes DEF-043), highlight, get_defined_constants |

**FN tally (631):** COVERED 118 · PARTIAL 49 · GAP-planned 100 · GAP-unplanned 251 ·
GAP-by-design 24 · N/A 89.
**FN coverage** (N/A+GD excluded): (118 + 24.5) / 518 = **27.5%** (row-weighted; usage-weighted
figure in Pass 4).

### 1.3 RT rows (20)

| ID | Verdict | Justification |
|---|---|---|
| RT-001 | CB | zero-ini model: config is compile-time; build profiles baked into artifacts (PJ-CLI-003) — the DEF-019 fix generalized |
| RT-002 | P | limits are hardcoded (`src/limits.rs`) + `serve --timeout`; no memory_limit equivalent |
| RT-003 | P | one runtime (`phg run/serve/build`) + PHP front-controller transpile bridge; no FPM/mod_php equivalents (different model) |
| RT-004 | N/A | persistent-process model is the deliberate opposite contract |
| RT-005 | CB | `phg serve` is native long-running — what PHP needs FrankenPHP/Swoole to retrofit |
| RT-006 | CB | bytecode VM is native; no out-of-language cache required (fixes DEF-043) |
| RT-007 | GP | no JIT; AOT is v2 (I-aot, §3 v2) — **→ [HEAD `af3aad3`: now P — a Cranelift method-level JIT ships as a DEFAULT feature (git `3725052`), unboxed int/float + control-flow paths (≈49× fibrec vs `--no-jit`), byte-identical VM fallback. PARTIAL not COVERED: only unboxed numeric paths JIT; the boxed-value object/enum/method JIT is the queued next step. Counted in §4.6.]** |
| RT-008 | P | `phorj.toml` + `phorj.lock` + `phg vendor` (exact-pin, offline, hash-verified — arguably safer); **no registry (by design, ADR-0005)**, **no transitive deps (documented deferral)** |
| RT-009 | CB | loader resolves functions and types alike — Composer's `files`-eager-include hack unnecessary (fixes DEF-032) |
| RT-010 | P | PSR-7/15 shape adopted at the value level (`handle(Request) -> Response`, Core.Http); PSR-3 logging GAP-planned (G-log); PSR-6/11/14 absent |
| RT-011 | P | `phg test` + `Core.Test` 8 assertions + deterministic Random/Time seam; no mocking, data providers, or coverage |
| RT-012 | CB | the checker **is** the language — real enforced generics vs docblock-PHPStan; `phg explain` codes |
| RT-013 | P | `phg debug` REPL + DAP server (interpreter-only, Dev-only); no profiler (F-profiler, §3 M13) |
| RT-014 | P | `serve --dev` error pages + faults-to-stderr; structured request logging GAP-planned (Q-serve-reqlog, §3 M6) |
| RT-015 | GD | no dynamic extension model (§5 rejects .so plugins + E-php-ffi); `.d.phg` declare-interop is the extension seam |
| RT-016 | CB | `phg build` native cross-OS static binaries (Linux glibc/musl, aarch64, Windows PE) — what static-php-cli/FrankenPHP-embed hack on |
| RT-017 | CE | `phg serve --dev` (127.0.0.1:8080, workers, timeout) |
| RT-018 | CE | SEMVER/STABILITY/DEPRECATION docs shipped (rock-3); pre-1.0 so cadence untested |
| RT-019 | CE | stdin `-`, `Process.arguments()`, `main -> int` exit codes |
| RT-020 | GU | no framework ecosystem (no Laravel/Symfony equivalent) — honest zero |

**RT tally (20):** CB 6 · CE 3 · PARTIAL 7 · GAP-planned 1 · GAP-unplanned 1 · GAP-by-design 1 · N/A 1.
**RT coverage:** (9 + 3.5) / 18 = **69.4%**.

### Pass 1 grand totals (824 verdict rows = 173 SYN + 631 FN + 20 RT)

| Verdict | SYN | FN | RT | Total |
|---|--|--|--|--|
| COVERED (better+equal) | 93 (56 CB + 37 CE) | 118 | 9 | **220** |
| PARTIAL | 20 | 49 | 7 | **76** |
| GAP-planned | 9 | 100 | 1 | **110** |
| GAP-unplanned | 7 | 251 | 1 | **259** |
| GAP-by-design | 24 | 24 | 1 | **49** |
| N/A | 20 | 89 | 1 | **110** |

---

## PASS 2 — Phorj → PHP (capabilities with no PHP counterpart)

The brag inventory: each row is a Phorj capability PHP cannot express, with the PHP pain it removes.

| # | Phorj capability | PHP pain removed |
|---|---|---|
| 1 | **Real enforced generics** — functions, methods, classes, enums (`Box<T>`, `Result<T,E>`), call-site inference, invariance checked (PJ-TY-005) | generics live only in docblocks policed by third-party PHPStan/Psalm (DEF-012) |
| 2 | **Compile-time null-safety** — non-optional `T` is never null; `??`/`?.`/`opt!`/if-let/smart-cast (PJ-SYN-007) | the billion-dollar `Call to a member function on null` production error |
| 3 | **Exhaustive `match` with patterns** — payload binding, type patterns, struct destructuring, guards, or-patterns, checked at compile time (PJ-SYN-006) | `UnhandledMatchError` at runtime; `switch` fall-through |
| 4 | **Totality checking** — `E-MISSING-RETURN`, `never`, `W-UNREACHABLE`, duplicate-arm detection | functions silently fall off the end returning null |
| 5 | **Three-tier error model** — checked `throws E` verified at every call site (`E-CALL-UNHANDLED`), `Result<T,E>`, uncatchable faults for bugs | `@throws` is an unenforced comment; warnings-vs-exceptions split brain (DEF-008) |
| 6 | **Flow narrowing** — `instanceof`/`!`/`&&`/`\|\|`/early-return narrow unions; match-over-union exhaustive | `if ($x instanceof A)` narrows nothing for the checker PHP doesn't have |
| 7 | **`decimal` primitive** (i128 fixed-point, `19.99d`, RoundingMode, exact-or-fault `/`; float×decimal mix is a compile error) | float-for-currency, the largest class of real-world PHP money bugs (DEF-023) |
| 8 | **`bytes` vs `string` split** + `b"…"` literals | one byte-string type pretending to be text (DEF-016 root) |
| 9 | **Typed XSS-safe HTML channel** — nominal `Html`/`Attr`, `html"…"`, auto-escaping holes (`E-HTML-HOLE`) | echo-a-string templating = XSS by default (DEF-020) |
| 10 | **`Secret<T>`** + `W-SECRET` sink lint | credentials are ordinary strings that leak into logs/traces |
| 11 | **Immutable-by-default** (`mutable` opt-in) + COW value collections | spooky aliasing, defensive cloning (DEF-006/DEF-024) |
| 12 | **Checked arithmetic** — overflow/div-zero/inexact are faults with identical traces on both backends | silent int→float overflow (DEF-022) |
| 13 | **Uncolored green threads** — `spawn f(x)` → `Task<T>`, `Channel<T>`, deterministic interleaving on both backends | Fibers are a low-level primitive with no scheduler; async is a third-party ecosystem fork |
| 14 | **Byte-identity dual backends + PHP oracle** — interpreter ≡ VM ≡ transpiled PHP under a real `php`, CI-gated (PJ-TOOL-005) | PHP has one implementation and no conformance corpus; BC is asserted, never proven |
| 15 | **`phg build`** — cross-OS standalone executables (glibc/musl/aarch64/Windows) with embedded program | deploy = interpreter + ini + opcache + vendor tree (DEF-043) |
| 16 | **PHP→Phorj lifter** (`phg lift`) + Phorj→PHP transpiler — a two-way migration bridge | one-way manual rewrites |
| 17 | **`.d.phg` declaration files** for foreign PHP (M8.5) — the TypeScript `.d.ts` model | no typed boundary to untyped code |
| 18 | **Toolchain in the box** — `phg test`/`benchmark`(+`--vs-php`+memory)/`format`/`lsp`/`debug --dap`/`explain`/`tokenize`/`disassemble` | PHPUnit/CS-Fixer/Xdebug/psalm are third-party assemblies |
| 19 | **Stable diagnostic codes + `phg explain`** — 196 codes, caret spans, did-you-mean, ratchet-tested explain coverage | error messages as unstructured prose |
| 20 | **Must-use by default** (`E-UNUSED-VALUE` + `discard`) | dropped return values (half the stdlib signals via return) — PHP got opt-in `#[NoDiscard]` only in 8.5 |
| 21 | **Method + return-type overloading** (`<Type>f()` selector) lowered to one dispatching PHP method | one symbol per name; userland `func_get_args` switchboards |
| 22 | **Multiple inheritance with compile-time conflict resolution** (`E-MI-CONFLICT`, `parent(A).m()`) | single inheritance + trait copy-paste conflicts (DEF-037) |
| 23 | **Enums with payloads** + generic enums | PHP enums are pure/backed constants only — no data |
| 24 | **`[T; N]` fixed lists** with static literal-index bounds checking | SplFixedArray checks at runtime |
| 25 | **Package hygiene as errors** — folder=path, one-public-type-per-file, casing (`E-PKG-*`, `E-FILE-*`, `E-NAME-CASE`) | PSR-4 is a convention enforced by no one |
| 26 | **Deterministic dependency model** — exact-pin lockfile, content-hash verify, offline-only builds, vendored source | Packagist supply chain + version ranges + install-time scripts |
| 27 | **Deterministic test seam** — seedable `Random`, `Time.freeze` | untestable time/rand without mockery |
| 28 | **Build profiles that cannot change behavior** — Dev/Release are observability-only; asserts never stripped (byte-identity across profiles) | `zend.assertions` makes prod semantically different from dev (DEF-019 family) |
| 29 | **Definite-assignment analysis** for fields (`E-FIELD-UNINITIALIZED`) | "typed property must not be accessed before initialization" at runtime |
| 30 | **UFCS** — `xs.map(f)` resolves to `List.map(xs, f)` with ambiguity errors | needle/haystack roulette (DEF-001) |
| 31 | **Expression-if with mandatory else**; expression-oriented `match` | statement/expression split; nested-ternary history (DEF-017) |
| 32 | **Text blocks with auto-dedent** + raw strings | heredoc indentation dance |
| 33 | **`with { }` record-update on instances** | shipped before PHP 8.5's `clone($o, [...])` |
| 34 | **WASM playground** sharing the real checker/VM | no official in-browser PHP |
| 35 | **Whole-project checking** — every `.phg` under the source root parsed/checked as one unit, cross-file diagnostics | file-at-a-time compile; cross-file breakage found at runtime |

---

## PASS 3 — DEF-045 defect scorecard

Adversarially checked: FIXED requires the mechanism to exist in E or be verified in code, not just
be plausible.

| DEF | Verdict | How |
|---|---|---|
| DEF-001 needle/haystack | **FIXED** | stdlib charter subject-first arg order + UFCS (`xs.contains(y)`) |
| DEF-002 naming chaos | **FIXED** | naming overhaul: full words, camelCase, charter-governed (`Core.String.startsWith`, no `strncasecmp`) |
| DEF-003 callback order | **FIXED** | uniform subject-first: `List.map(xs,f)`, `filter(xs,f)`, `reduce(xs,init,f)` |
| DEF-004 juggling / two equalities | **FIXED** | static types; single typed `==`. (Residual: float `==` compiles without lint [Verified] — IEEE issue, not PHP juggling; J-float-eq-lint pending) |
| DEF-005 array conflation | **FIXED** | List / Map / Set / `[T; N]` distinct types |
| DEF-006 spooky references | **FIXED** | no references; value/handle split explicit; foreach binds values |
| DEF-007 `@` suppression | **FIXED** | operator absent; errors are typed or fatal |
| DEF-008 errors-vs-exceptions split | **FIXED** | one coherent three-tier model, checker-enforced |
| DEF-009 `false` error returns | **FIXED** | `T?` optionals (`indexOf -> int?`); no `0\|false` trap |
| DEF-010 mutable global state | **FIXED** | no superglobals/`global`; `Environment` read-only; immutable default |
| DEF-011 mutable DateTime | **FIXED** | `Core.Time` classes are immutable-only |
| DEF-012 no generics | **FIXED** | erased generics across functions/methods/classes/enums, checker-enforced |
| DEF-013 unserialize RCE | **FIXED** | no native object (de)serialization exists; `Core.Json` is data-only |
| DEF-014 isset/empty conflation | **FIXED** | optionals + explicit `isEmpty`; no truthiness predicates (by-design reject) |
| DEF-015 locale-sensitive core | **FIXED** | no `setlocale` surface at all; behavior never varies by host locale (ICU features deferred to an explicit opt-in tier) |
| DEF-016 four string families | **PARTIALLY-FIXED — inherited sub-flaw** | ONE `Core.String` family + separate `bytes` type fixes the four-family chaos; **but `String.length` counts BYTES today [Verified: `"héllo"` → 6] — strlen's exact flaw inherited until M-text codepoint semantics land. High-priority finding.** |
| DEF-017 left-assoc ternary | **FIXED** | no ternary; expression-if with mandatory else |
| DEF-018 case-sensitivity mess | **FIXED** | uniformly case-sensitive + enforced casing conventions |
| DEF-019 foot-gun inis | **FIXED** | zero runtime config; profiles cannot change semantics (byte-identity across profiles) |
| DEF-020 include/template RCE | **FIXED** | no include/eval; typed auto-escaping `html"…"` |
| DEF-021 extract/variable-variables | **FIXED** | rejected (§5); scope is statically known |
| DEF-022 silent int→float overflow | **FIXED** | checked arithmetic → `IntOverflow` fault; float keys impossible (typed Map keys) |
| DEF-023 float display duality | **FIXED** | single-sourced Ryū round-trip float rendering on all backends; `decimal` for business data |
| DEF-024 value/object asymmetry | **PARTIALLY-FIXED** | asymmetry deliberately retained (List=COW value, Instance=handle) but made explicit, documented, and defanged by immutable-by-default. Adversarial note: it is still one `=` with two meanings at the call site |
| DEF-025 in-place bool sorts | **FIXED** | `List.sort/sortWith` are pure and return the list |
| DEF-026 three merge semantics | **FIXED** | explicit `concat` vs `merge`; no `+` on maps |
| DEF-027 loose in_array/switch | **FIXED** | typed `contains`; `switch` absent, `match` strict+exhaustive |
| DEF-028 key juggling | **FIXED** | `"1"` and `1` are distinct typed keys |
| DEF-029 string increment | **FIXED** | no `++` on strings |
| DEF-030 parse_url non-conformance | **PARTIALLY-FIXED** | nothing wrong shipped (encode/decode only) — but no URL *parser* exists yet; G-url (§3 M6) is planned to land spec-compliant. Absent ≠ fixed |
| DEF-031 mail() injection | **N/A** | no mail facility (unplanned) — nothing to inject |
| DEF-032 no fn autoloading | **FIXED** | loader resolves free functions statically; single-file PHP emission avoids PSR-4's function hole |
| DEF-033 namespace fallback | **FIXED** | explicit imports; zero runtime name fallback |
| DEF-034 named args froze param names | **N/A** | no named args yet; **when A-named-args lands, param names become API — decide renaming policy up front** |
| DEF-035 destructor fatality | **FIXED** | destructors deliberately absent (A-destruct reject) — the hazard class cannot exist |
| DEF-036 silent destructuring nulls | **FIXED** | refutable destructure requires diverging `else` (`E-DESTRUCTURE-NEEDS-ELSE`); fields type-checked |
| DEF-037 trait copy-paste | **PARTIALLY-FIXED** | conflicts are hard errors with explicit `rename`/`exclude` (vs silent `insteadof`); but composition is still copy-in — trait state duplicates per using class, same as PHP |
| DEF-038 LSB four-way confusion | **FIXED** | by simplification: explicit `parent.m()`/`parent(A).m()`; no `self::`/`static::` distinction exists (LSB itself is a planned feature — keep the four-way trap out when it lands) |
| DEF-039 strtotime DWIM | **FIXED** | deliberately absent; only explicit Time factories |
| DEF-040 resource\|false chains | **FIXED** | typed handles + `T?` returns |
| DEF-041 lossy silent casts | **FIXED** | `as` → `T?`; `Conversion.floatToIntExact -> int?`; no implicit narrowing |
| DEF-042 output/header coupling | **FIXED** | `Response` value serialized once; headers are data |
| DEF-043 per-request compile model | **FIXED** | compiled bytecode VM + standalone binaries; no out-of-language cache |
| DEF-044 json_decode null ambiguity | **FIXED** | `Json.parse -> Json?` — `Json.Null` value ≠ parse failure |
| DEF-045 superglobal request model | **FIXED** | `handle(Request) -> Response` at the value level; body/headers are typed data |

**Scorecard: 39 FIXED · 4 PARTIALLY-FIXED (DEF-016, -024, -030, -037) · 2 N/A (DEF-031, -034) ·
0 fully INHERITED.** The one adversarial red flag: **byte-based `String.length` (DEF-016) is a
genuinely inherited strlen flaw** until M-text ships codepoint semantics.

---

## PASS 4 — Completion percentage model

### 4.1 Method

Coverage per domain = (COVERED×1 + PARTIAL×0.5) / (rows − N/A − GAP-by-design). N/A and
GAP-by-design are excluded from the denominator: Phorj should not be penalized for deliberately
removing footguns or for rows meaningless in its model. All weights are judgment calls and are
flagged as such; the row counts and per-group arithmetic are mechanical, so the number is
recomputable after each milestone by re-running Pass 1 verdicts.

### 4.2 Raw row-parity (no weighting — the pessimistic floor)

- SYN: 103 / 129 = 79.8%
- FN: 142.5 / 518 = 27.5%
- RT: 12.5 / 18 = 69.4%
- **Pooled: (103 + 142.5 + 12.5) / (129 + 518 + 18) = 258 / 665 = 38.8%**

This treats `mysqli_stmt_bind_param` and `foreach` as equal-weight rows — useful as a floor, wrong
as a headline.

### 4.3 Usage-weighted stdlib (judgment: groups tiered by real-code frequency)

Tier ×3 (daily): STR ARR MATH JSON DATE FS PCRE URL VAR FUNC HASH CRYPT.
Tier ×2 (common): DB CURL SESS REFL RAND PROC MISC CTYPE FILTER MB SPL OB NET.
Tier ×1 (occasional): ICONV STREAM SOCK XML FINFO ZLIB ZIP PHAR INTL GD.

Per-tier score/denominator (from §1.2, N/A+GD excluded):
- T1: score 124.5 / den 303 → 41.1%
- T2: score 18.5 / den 140 → 13.2%
- T3: score 0 / den 75 → 0%

Weighted stdlib = (3×124.5 + 2×18.5 + 1×0) / (3×303 + 2×140 + 1×75) = 410.5 / 1264 = **32.5%**

### 4.4 Domain-weighted PHP-parity %

| Domain | Weight (judgment) | Coverage | Contribution |
|---|--|--|--|
| Language syntax, semantics, type system, error model (SYN) | 35 | 79.8% | 27.9 |
| Stdlib, usage-weighted (FN) | 40 | 32.5% | 13.0 |
| Runtime, deployment, ecosystem (RT) | 25 | 69.4% | 17.4 |
| **PHP-parity %** | 100 | | **≈ 58%** |

Weight rationale (stated, contestable): PHP's practical surface is stdlib-heavy (40); the language
core is what migrating code is *written in* (35); runtime/ecosystem determines deployability (25).
Moving stdlib weight ±10 points moves the headline ±4.7 points — the number is weight-sensitive
and should always be quoted with the weights.

### 4.5 Vision % (parity + the beyond-PHP programme)

Vision denominator = 70% PHP-parity + 30% roadmap-programme completion (the §3 rollup milestones,
judged against E/current state; v2 and post-1.0 M13 excluded as out-of-vision-scope):

M-RT 100 · M-faults 100 · M5-followups 25 · M2.5-P3 50 · M6 70 · M8/M8.5 60 · M9 70 ·
M11+M4 70 · M7/M12 70 · GA-M12 60 · M-NUM 80 · M-TIME 70 · M-text 40 · M-Test 85 ·
M-perf 30 · M-Batteries 50 → mean = 1030/16 = **64.4%** [judgment-graded per milestone]

**Vision % = 0.70 × 58.3 + 0.30 × 64.4 = 40.8 + 19.3 ≈ 60%**

Both headline numbers, honestly labeled: **PHP-parity ≈ 58% (row-parity floor 39%) · Vision ≈ 60%.**
The dominant drag on both is the same thing: stdlib breadth (FS/streams, DB, HTTP client,
sessions, XML, intl) — the language itself is at ~80% parity with substantial better-than coverage.

> ⚠ **§4.2–4.5 above are the RATIFIED full-pass at the E-surface baseline `ccb2403` (2026-07-01).
> They are preserved as the model's derivation/audit trail. The CURRENT headline lives in §4.6.**

### 4.6 Recompute at HEAD `af3aad3` (2026-07-10 — Wave C milestone close)

**Method — a systematic verdict-scan at HEAD, not a memory-delta.** Every SYN `P`/`GP` row (29) and
every RT `P`/`GP` row (8) was re-walked against HEAD, and all 35 FN groups were checked against every
`src/native/` feat/fix commit since the `ccb2403` E-surface baseline. This is the full-re-pass the
recompute rule (§4.1) mandates at a milestone close. The scan yields **exactly 2 moved rows** — the marathon
(2026-07-04→10, 203 commits) was **perf + language-polish, not stdlib breadth**, so parity barely
moved. Explicitly checked-and-ruled-out (no flip): **SYN-118 user-defined attributes** (DEC-194
shipped, but attach to only 2 of PHP's 7 targets — classes + free functions — with no
attribute-reflection yet, so the feature stays PARTIAL, not COVERED — the improvement is a richer P
justification, not a verdict flip); FN-MATH trig/hyperbolic breadth (11 GP rows — `math.rs` added
*no* `asin/acos/atan/hypot/log2/…`, [Verified: git diff]); `str_split`/`mb_str_split`
(`String.characters` is codepoint-wise ≠ PHP's byte-wise `str_split`, and sits inside the M-text GP
programme whose blocker — byte `String.length`, DEF-016 — is unmoved); Wave A/B (type-system + Option/
Result combinators land on rows already scored CB); `Math.tryAdd/trySub/tryMul` + `#[UncheckedOverflow]`
(beyond-PHP, no PHP-row counterpart — PASS-2 material).

**The percentage chain (each link a dated re-score of the same ratified model, §11 ledger):**
`≈58% (ccb2403 full-pass, §4.2–4.5)` → `≈59% (§11.2 re-score, 2026-07-03: FN-HASH/CSPRNG/INI/RAND)` →
`≈60% (this recompute, HEAD af3aad3)`. Each step is an additive delta on the prior, evidence-per-row.

| # | Row(s) | Was | Now | Δ | Evidence [Verified] |
|---|---|---|---|--|---|
| 1 | FN-STR-053/054/055/056 (sprintf/printf/vsprintf/vprintf) | GP | COVERED (×4) | T1 +4 | `Core.String.format` full directive engine (git `9bc6612`…`130b0cb`); runtime-list arg accepted (`calls.rs:347`) |
| 2 | RT-007 (JIT) | GP | P | RT +0.5 | Cranelift unboxed JIT, default feature (git `3725052`) |

**Recomputed arithmetic (delta on the §11.2 baseline — SYN 79.8%, T1 131.5/303, RT 69.4%):**
- SYN: unchanged (SYN-118 stays P) ⇒ 103/129 = **79.8%**
- FN usage-weighted: T1 131.5→135.5/303 ⇒ (3×135.5 + 2×19.0 + 1×0)/1264 = 444.5/1264 = **35.2%** (was 34.2%)
- RT: PARTIAL 7→8, GP 1→0 ⇒ (9 + 4)/18 = 13/18 = **72.2%**
- **PHP-parity = 0.35×79.8 + 0.40×35.2 + 0.25×72.2 = 27.9 + 14.1 + 18.1 = ≈ 60%**
- Raw row-parity floor: SYN 103/129 · FN 154/518 · RT 13/18 ⇒ (103 + 154 + 13)/665 = 270/665 = **≈ 41%**

**Vision %** — programme mean re-based from §11.2's 65.3%. Only two milestones take a cleanly
attributable bump: M11+M4 70→75 (sprintf directive engine — A-sprintf is a named §3 M11 item) and
**M-perf 30→40** (JIT-default + inline method cache + `#[UncheckedOverflow]` + `Math.try*` shipped —
*infrastructure only; the HARD PERF MANDATE is still unmet: only fibrec + unchecked-int-add WIN, and
methodcall/objalloc/enum remain LOSS* [Speculative]). User attributes + DI v1 are conservatively NOT
credited — the original 16-milestone vision vector has no slot that maps to them, and inventing one
would be fabrication (they are real beyond-PHP wins, just untracked by this vector). Mean = 1060/16 =
66.3%. **Vision = 0.70×60.1 + 0.30×66.3 = 42.1 + 19.9 = ≈ 62%.**

**Grade:** row flips **[Verified]** (commits + source cited); the headline figure **[Inferred]** (an
additive delta on the ratified §11.2 arithmetic, not a fresh 665-row re-tally); the milestone-programme
weights **[Speculative]** (judgment, ±, quoted with the model). Moving stdlib weight ±10 pts still
moves the headline ±~5 pts — quote with the 35/40/25 weights.

**The finding that matters more than the number:** the marathon (2026-07-04→10, 203 commits) moved
parity **+1 pt (59→60)**; across the full 252 commits since `ccb2403` (2026-07-01), **+2 (58→60)**.
Small either way because the ONLY stdlib-breadth movers in the whole span were crypto (§11.2,
pre-marathon) and sprintf (this re-pass) — the dominant parity drag is untouched: TOP-20 #1 (DB), #2
(HTTP client), #3 (sessions), #5 (FS breadth), #12 (XML), #19 (intl) all sit exactly where the
full-pass left them. This is the evidence that validates the locked next-session
order: **② boxed-value JIT is the *perf* lever** (the unmet mandate), **③ web spine is the *parity*
lever** — §11.3 projects the DB+HTTP+sessions wave (W3) as the jump to ≈65–66%. String.format closed
the sprintf *directive engine* (a real but ~4-row contribution against a 518-row stdlib denominator).

### 4.7 Recompute at HEAD `bea7f61` (2026-07-13 — Phase-A close + ownership stack + DEC batch)

**Scope of the span (af3aad3 → bea7f61, sessions 5–6, ~45 commits):** Phase A perf-tail closed
(ALL 21 micros ≥ 1.0× vs release-php+JIT, protocol-ratcheted medians), Ω-0 footgun audit, Ω-1
slice 1 (Core.Sql per-verb DBAL), JIT widening W1–W9 + S8 (strings/lists/maps/instances/enums/
union-Dyn cells — the FULL sqlbuild pipeline compiles end-to-end), the ownership stack
(chain-accumulator, Rc-sharing, L2b receiver-transfer + field-TAKE; sqlbuild macro 0.27→0.36 vs
php), the DEC-201..206 adjudication batch RULED, and DEC-202 SHIPPED (builtin-class
E-RESERVED-NAME).

**Row flips [Verified: commits + gate output]:**

| # | Row(s) | Was | Now | Δ | Evidence |
|---|---|---|---|--|---|
| 1 | RT-007 (JIT) | P | COVERED | RT +0.5 | The JIT is no longer "unboxed numerics only": strings, lists, maps, instances (wide two-slot), enums, union cells and the whole Core.Sql pipeline compile (sessions 5–6); default feature; 21/21 micros ≥ php+JIT [Verified: microbench-gate PASS 21 WIN / 0 flips, quiet-box 2026-07-13] |

Explicitly checked-and-ruled-out: **Core.Sql DBAL flips no FN-DB rows** (the rows are driver
EXECUTION functions — mysqli/PDO query/bind/fetch; a query *builder* without `Core.Db` execution
covers none of them — the builder is credited in the Vision programme instead); **DEC-202 flips no
SYN row** (a new reject-rule, not a coverage change); DI v1 / user attributes stay uncredited (§4.6
rationale unchanged).

**Recomputed arithmetic (additive delta on §4.6):**
- SYN: unchanged **79.8%** · FN usage-weighted: unchanged **35.2%**
- RT: 13→13.5 / 18 = **75.0%**
- **PHP-parity = 0.35×79.8 + 0.40×35.2 + 0.25×75.0 = 27.9 + 14.1 + 18.75 ≈ 61%**
- Raw floor: (103 + 154 + 13.5)/665 ≈ **41%**

**Vision %** — programme deltas: **M-perf 40→70** (the HARD PERF MANDATE is now MET on the entire
micro suite — all 21 ≥ 1.0×, several far above (objalloc 9.5×, match 7.2×, trycatch 33×), JIT
default, gate ratcheted; NOT 100 because the sqlbuild macro-bench is still 0.36 vs php and deep
object-graph workloads remain VM-bound [Verified: gate output + pinned sqlbuild timings]);
**GA-M12 60→62** (Core.Sql DBAL = the query-building half of the DB story; execution half — Core.Db
driver — still unshipped [Inferred: MASTER-PLAN Ω-1 status]). Mean = (1060 + 30 + 2)/16 = 1092/16
= **68.3%**. **Vision = 0.70×60.75 + 0.30×68.3 = 42.5 + 20.5 ≈ 63%.**

**Grade:** RT-007 flip **[Verified]**; headline **[Inferred]** (additive delta, same ratified
model); programme grades **[Speculative]** (judgment, quoted with the 35/40/25 weights).

**The same finding, sharpened:** two more sessions of perf + language work moved parity
+1 (60→61) and Vision +1 (62→63) — *the model is doing its job*: the dominant drag is UNCHANGED
stdlib breadth (DB execution #1, HTTP client #2, sessions #3, FS #5, XML #12, intl #19). The
projected jump to ≈65–66% parity is the W3 web spine (Core.Db + HTTP + sessions) — which is
exactly Ω-1's remainder, queued right after the sqlbuild ≥1.0 gate (META-1).


---

## TOP-20 highest-impact gaps (impact = frequency in real PHP code × migration blockage)

| # | Gap | Class | Evidence / plan |
|---|---|---|---|
| 1 | **Database access** (PDO/mysqli/SQLite — all 10 FN-DB rows) | GAP-planned | ROADMAP M6 "Postgres connectivity"; nothing exists today. Blocks essentially every real app |
| 2 | ~~**HTTP client**~~ **→ SHIPPED (§4.9, DEC-273)** — Core.HttpClient: GET/POST/PUT/DELETE/HEAD/PATCH + headers + Bearer/Basic + cookies + timeout | ✅ 9 C + 3 P | Was the #2 blocker; now covered. Remaining GU: curl multi-handle / low-level SSL opts |
| 3 | **Sessions / cookies / auth** (10 FN-SESS rows) **→ PARTIAL (§4.9, DEC-242)** — Core.Session + class Cookie shipped | 2 C + 3 P | K-auth-csrf-session (§3 M6). CSRF + full session-config still open |
| 4 | **sprintf/printf format family** (7 FN-STR rows) | GAP-planned | A-sprintf (§3 M11). Ubiquitous in ported code; interpolation covers only simple cases |
| 5 | **Filesystem breadth** (mkdir/scandir/glob/stat/perms/temp/streams — ~40 FN-FS rows) **→ PARTIAL (§4.9)** — Core.Fs 17 fns | +5 C +1 P | dir/create/list/walk/temp shipped; **stream handles + stat/perms + glob still GU** |
| 6 | **Unicode-correct strings** (byte `String.length`, mb_* replacement) **→ PARTIAL (§4.9, DEC-256)** — Unicode tier addresses DEF-016 | +2 C +3 P | The one *inherited* PHP defect; full mb_* long tail + normalization/grapheme still open |
| 7 | **Named arguments + variadics + spread** | GAP-planned | A-named-args/A-variadics (§3). Modern PHP idiom; also blocks the lifter on 8.0+ code |
| 8 | **Generators/`yield` + iterator protocol** | GAP-planned | marathon A2 + A-iterators (§3 M11). Lazy pipelines, large-data loops, Traversable interop |
| 9 | **Date/time breadth** (timezones, formatting, DatePeriod, parsing) | GAP-planned | N-tz-iana (M-TIME-2). Every business app touches tz |
| 10 | **CSPRNG** (random_int/random_bytes) + timing-safe compare (hash_equals) + HMAC | GAP-planned | G-crypto (§3 M8). Security-critical absences — tokens/nonces can't be generated safely today |
| 11 | **array_* long tail** (diff/intersect on lists, splice, column, combine, multisort, pad) | GAP mixed | L-list-breadth remainder. Muscle-memory blockers for line-by-line migration |
| 12 | **XML/DOM/XPath** (12 FN-XML rows) | GAP-unplanned | enterprise integration formats; no plan on record |
| 13 | **Subprocess execution** (exec/proc_open family) | GAP-unplanned | CLI tooling written in PHP shells out constantly; no Phorj process API |
| 14 | **Regex breadth** (preg_replace_callback, preg_quote, full modifier surface) | GAP-unplanned | callback-replace is the common non-trivial regex use |
| 15 | **Compression/archives** (zlib/zip; phar planned) | GAP-unplanned (P-phar GP) | deploy/packaging + data interchange |
| 16 | **User-defined attributes + reflection of them** | GAP mixed | only `#[Route]` exists; PHP frameworks are attribute-driven (8.0+) |
| 17 | **`__toString`/Stringable + `__invoke`** | GAP-planned | A-magic-stringable/A-magic-invoke (§3). Pervasive interop idioms in APIs being lifted |
| 18 | **Structured logging** (error_log/syslog → G-log/Q-corelog) | GAP-planned | §3 M11/M6; production apps need a log seam before serve is production-usable |
| 19 | **intl formatters** (NumberFormatter/IntlDateFormatter/MessageFormatter) | GAP-planned (tier-3 defer) | any localized app; deliberate ICU deferral needs an explicit extension story |
| 20 | **Math long tail + BigInt** (atan2/hypot/log2, base_convert; GMP/BCMath arbitrary precision) | GAP-planned | G-math-breadth (M11), N-bigint (M-NUM-2) |

Watch-item outside the ranking: **transitive dependencies** (RT-008, documented deferral) becomes
a top-10 blocker the moment a second-party package ecosystem appears.

---

## SUMMARY

```
PASS 1 (824 rows: 173 SYN + 631 FN + 20 RT)
  COVERED          220   (SYN 93 [56 better / 37 equal] · FN 118 · RT 9)
  PARTIAL           76   (SYN 20 · FN 49 · RT 7)
  GAP-planned      110   (SYN 9  · FN 100 · RT 1)
  GAP-unplanned    259   (SYN 7  · FN 251 · RT 1)
  GAP-by-design     49   (SYN 24 · FN 24 · RT 1)
  N/A              110   (SYN 20 · FN 89 · RT 1)

Coverage:  language 79.8% · stdlib 27.5% row-weighted / 32.5% usage-weighted · runtime 69.4%

PHP-parity %  ≈ 58   (ccb2403 full-pass; domain-weighted; raw row-parity floor 38.8%)
Vision %      ≈ 60   (ccb2403 full-pass; 70% parity + 30% roadmap-programme at 64.4%)

  ⟶ CURRENT at HEAD 9a5deff6 (2026-07-19, §4.11):  PHP-parity ≈ 68%  ·  Vision ≈ 69%  ·  floor ≈ 53%
     (§4.11 = backed enums DEC-302 (PHP 8.1) + targeted phantom-gap credit (Core.Path, crypto); full re-tally still owed)
  ⟶ PREV at HEAD 580c6041 (2026-07-19, §4.10):  PHP-parity ≈ 66%  ·  Vision ≈ 67%  ·  floor ≈ 51%
     chain: 58% (ccb2403) → 59% (§11.2) → 60% (§4.6, af3aad3) → 61% (§4.7, bea7f61) →
     62% (§4.8, DB+Mail) → 64% (§4.9, Web/Runtime spine: HTTP client #2 + FS #5 + Uri +
     Unicode #6 + sessions #3) → 66% (§4.10, overnight Wave-B: named args + variadics +
     Math tail + List set-ops + String tail + Deque + PriorityQueue). ⚠ PHANTOM-GAP finding
     (§4.10): several §1.2 "gaps" are already built (Core.Path/FS-broad/crypto) → true parity is
     higher, pending the owed full §1.2 per-row re-pass.

PASS 2: 35 beyond-PHP capabilities (no PHP counterpart)

PASS 3 (45 DEF): 39 FIXED · 4 PARTIALLY-FIXED (DEF-016 string-bytes [inherited sub-flaw,
  verified], DEF-024 value/handle asymmetry, DEF-030 URL parser absent-not-fixed,
  DEF-037 trait state duplication) · 2 N/A · 0 fully INHERITED

Top-5 gaps: database access · HTTP client · sessions/auth · sprintf family · filesystem breadth
```

### 4.8 Recompute at HEAD (2026-07-16 — the DEC-208 Core.Db execution wave + Core.Mail, fable overnight run)

**Scope of the span (bea7f61 → HEAD, ~40 commits, 2026-07-13→16):** the ENTIRE `Core.Db` execution
layer (DEC-208 slices S1/S2/B–K: prepare/bind/bindNamed/bindList/query/exec/executeMany/
execReturningId/lastInsertId, typed Row accessors incl. decimal/enum/JSON/arrays, queryInto/
queryOneInto/queryScalar/queryMap hydration + naming strategies + turbofish, lazy streamInto,
manual + closure transactions + savepoints + retry, typed DbError taxonomy, timeout, onQuery,
Secret credentials, W-SQL-INJECTION lint, THREE drivers: bundled SQLite + Postgres + MySQL/MariaDB,
`db` a DEFAULT feature) · `Core.Mail` (DEC-223: injection-safe composition, 4 transports, DKIM,
typed taxonomy) · `Core.Log` · DEC-221/222 throwing ctors + closures · DEC-214 collections ·
E-TRANSPILE-DB/MAIL ladder gates · 2 macro benches.

**Row flips [Verified: tests/database.rs 79 rows-of-coverage + tests/mail.rs + shipped commits]:**

| # | Row(s) | Was | Now | Δ | Evidence |
|---|---|---|---|--|---|
| 1 | FN-DB (10 rows — "entire database surface absent", TOP-20 #1) | 10 GAP-planned | 9 COVERED + 1 P | T2 +9.5 | PDO/mysqli execution parity and beyond: prepared/named binds, typed fetch, transactions incl. savepoints+retry (PDO has no retry), lastInsertId/RETURNING, bulk executeMany, IN-list binds (PDO cannot), 3 drivers, typed error taxonomy (PDO: string codes). The 1 P: isolation-level surface + fetch-mode variety corners deferred [Verified: tests/database.rs on both backends; tests/database_postgres.rs/db_mysql.rs live legs] |
| 2 | FN-NET `mail()` (1 row) | GAP-unplanned | COVERED | T2 +1 | Core.Mail exceeds mail(): SMTP auth+TLS+attachments+DKIM vs none of those; DEF-031 header injection = structurally impossible (typed Address) vs PHP's mitigable [Verified: tests/mail.rs both backends] |
| 3 | FN-NET `syslog` (1 row) | GAP-planned | COVERED | T2 +1 | Core.Log shipped in-span (3-sink) [Verified: tests/log.rs] |
| 4 | DEF-031 scorecard | N/A ("no mail facility") | **BETTER-THAN-PHP** | — | the injection class is unrepresentable, not merely mitigated |

**Recomputed arithmetic (additive delta on §4.7):**
- T2 score 18.5 → 30 / den 140; Δweighted = 2×11.5 / 1264 = +1.8pp → FN usage-weighted 35.2 → **37.0%**
- SYN unchanged **79.8%** (turbofish/throwing-ctors are phorj-side ergonomics, not PHP-parity syntax
  rows — the DEC-202 precedent) · RT unchanged **75.0%**
- **PHP-parity = 0.35×79.8 + 0.40×37.0 + 0.25×75.0 = 27.9 + 14.8 + 18.75 ≈ 62%** (was ≈61)
- Raw floor: (103 + 166 + 13.5)/665 ≈ **42%**

**Vision %** — programme deltas: **GA-M12 62→78** (the DB story is now BOTH halves — execution +
3 drivers + typed hydration + streaming — AND the mailer battery; not higher: HTTP client, sessions,
FS remain). **M-perf holds 70** (21 micros hold ≥1.0; the two NEW macro benches land as honest
flagged losses — jsonround 0.25×, dbwork 0.63× — with recorded anatomy+levers; crediting perf while
adding known losses would be dishonest). Mean = (1092 + 16)/16 = 1108/16 = **69.3%**.
**Vision = 0.70×62.0 + 0.30×69.3 = 43.4 + 20.8 ≈ 64%** (was ≈63).

**Grade:** row flips **[Verified]** (per-row evidence above); headline **[Inferred]** (additive
delta on the ratified model, quoted with the 35/40/25 weights); programme scores **[Speculative]**
(judgment, as always).

**The same finding, again sharpened:** the single largest migration blocker (TOP-20 #1, all 10
FN-DB rows) fell this span and the headline moved +1 (61→62) — stdlib breadth remains the drag
because the NEXT blockers are untouched: HTTP client (#2), sessions (#3), FS/streams (#5). Those
are exactly the run's Web/Runtime pillar packs; §11.3's ≈65–66% projection needs all three.

### 4.9 Recompute at HEAD `da3fc0c2` (2026-07-18 — the Web/Runtime spine catch-up + language mega-arc)

**Scope of the span (§4.8 HEAD → `da3fc0c2`, ~108 commits, 2026-07-16→18):** the recompute §4.8
CONSERVATIVELY deferred (it credited only DB/Mail/Log). Now folded in — all VERIFIED shipped +
surface-checked by grep this pass (Rule-11 discipline; three "gaps" turned out already-built —
Core.Regex/Core.Decimal/`match` — so every credit below was surface-confirmed, not memory-trusted):
**Core.HttpClient** (GET/POST/PUT/DELETE/HEAD/PATCH + headers + Bearer/Basic auth + cookies + timeout,
DEC-273 wave 3) · **Core.Fs** breadth (17 fns: readText/writeText/appendText/copy/move/delete/size/
exists/isFile/isDir/createDir/removeDir/removeDirAll/listDir/walk/tempDir) · **Core.Uri** (RFC-3986:
parse/scheme/host/path/query/fragment/port/userInfo + encodeForm, DEC-240) · **Unicode string tier**
(DEC-256 — addresses the inherited DEF-016 byte-length defect) · **Core.Session + class Cookie**
(DEC-242) · **Core.Iterator + generic interfaces** (DEC-257, foreach-over-implementor) · **String
distance** (levenshtein/similarText, DEC-243) · **List breadth** (flatMap/takeWhile/dropWhile/groupBy/
zip/partition, DEC-214/288/289) · **Date.parse/Instant.parse** (DEC-290) · **pipe `|>`** (PHP-8.5
precedence slot, DEC-239) · **asymmetric visibility** `private(set)` (PHP 8.4, DEC-241) · plus the
extension architecture (DEC-273), unified loader (DEC-282), Core.Input stdin (DEC-281), tuples
(DEC-288), lazy-Json perf (DEC-294 — perf, no parity row).

**Row flips [Verified: registry + ext/native surface grep this pass + shipped commits]:**

| # | Group | Tier | Was (§4.8) | Now | Δ score | Evidence |
|---|---|---|---|---|--|---|
| 1 | FN-CURL (13, "no HTTP client", TOP-20 #2) | T2 | 13 GP | 9 C + 3 P | +10.5 | Core.HttpClient verbs+auth+cookies+timeout [Verified: `src/ext/http_client/`]; 1 GU (curl multi/low-level opts) |
| 2 | FN-FS (55) | T1 | 8 C / 2 P | +5 C +1 P | +5.5 | Core.Fs createDir/removeDir/removeDirAll/listDir/walk/tempDir/isFile/isDir → mkdir/rmdir/scandir/opendir/tempnam; streams/stat/glob still GU |
| 3 | FN-URL (10) | T1 | 3 C / 3 GP | +3 C | +3.0 | Core.Uri parse_url + http_build_query + 8.5 Uri objects — DEF-030 leapfrogged [Verified: `src/ext/uri/`] |
| 4 | FN-MB (22) | T2 | 0 C / 2 P | +2 C +3 P | +3.5 | DEC-256 Unicode tier: codepoint-correct length/case; full mb_* family still partial |
| 5 | FN-SESS (10) | T2 | 10 GP | +2 C +3 P | +3.5 | Core.Session + class Cookie [Verified: `src/cli/preludes.rs`]; CSRF/full session-config partial |
| 6 | FN-SPL (39) | T2 | 2 C / 2 P | +1 C +1 P | +1.5 | DEC-257 Core.Iterator/Traversable protocol + generic interfaces; heaps/PQ/SplObjectStorage still GU |
| 7 | FN-STR (93) | T1 | 30 C | +2 C | +2.0 | levenshtein + similarText (DEC-243) from GU |
| 8 | FN-ARR (74) | T1 | 26 C / 2 GP | +1 C | +1.0 | zip lands (DEC-288); flatMap/groupBy/takeWhile/dropWhile are beyond-PHP enrichment (PASS-2) |
| 9 | FN-DATE (27) | T1 | 5 C / 5 P | +1 C +1 P | +1.5 | Date.parse/Instant.parse (ISO) — date_parse from GU (DEC-290) |
| 10 | SYN (pipe `\|>`, asymmetric visibility) | — | 103 C | +1.5 | SYN +1.5 | PHP-8.5 `\|>` + PHP-8.4 `private(set)` are parity SYN, now COVERED (DEC-239/241) |

Explicitly checked-and-ruled-out (no NEW flip): **Core.Regex** — already COVERED in the §1.2 baseline
(FN-PCRE 4 C / 2 P), NOT an uncounted mover; **`match` expression** — already built + mature
(`Expr::Match`, guards, exhaustive), a §1.2 SYN row, not new; **Core.Decimal** — baseline (FN-MATH P);
**extension architecture + unified loader (DEC-273/282)** — real ecosystem infra but map to RT-008
(deferred watch-item) / autoload (§1.2 N/A "superior model"), conservatively UNCREDITED like DI/attrs;
**tuples/lazy-Json/typed-foreach** — beyond-PHP or perf, no PHP-parity row.

**Recomputed arithmetic (additive delta on §4.8 — T1 135.5/303, T2 30/140, T3 0/75; SYN 103/129; RT 13.5/18):**
- T1 score 135.5 → 135.5 + (5.5+3+2+1+1.5) = **148.5 / 303**
- T2 score 30 → 30 + (10.5+3.5+3.5+1.5) = **49 / 140**
- FN usage-weighted = (3×148.5 + 2×49 + 1×0) / 1264 = (445.5 + 98) / 1264 = 543.5/1264 = **43.0%** (was 37.0%)
- SYN: 103 → 104.5 / 129 = **81.0%** (was 79.8% — pipe + `private(set)`)
- RT: unchanged **75.0%** (loader/extensions map to deferred RT-008 / N/A autoload)
- **PHP-parity = 0.35×81.0 + 0.40×43.0 + 0.25×75.0 = 28.35 + 17.2 + 18.75 ≈ 64%** (was ≈62)
- Raw row-parity floor: FN raw 166 (§4.8) + 26 C + (12 P ×0.5 = 6) = 198; (104.5 + 198 + 13.5)/665 = 316/665 ≈ **47%** (was ≈42 — the floor catching up to the weighted headline, gap 20pp→17pp, is exactly what high-row-count FN breadth should do)

**Vision %** — programme deltas on §4.8 (mean 1108/16 = 69.3): **GA-M12 78→82** (Web/Runtime spine
substantially in: HTTP client + Uri + sessions/cookies + Fs), **M-text 40→55** (DEC-256 Unicode tier —
the one inherited PHP defect, DEF-016, finally addressed), **M-Batteries 50→62** (Fs breadth + Uri +
the extension architecture + string distance), **M11+M4 70→75** (List/date breadth). **M-perf holds 70**
(lazy-Json/dbwork are perf, honestly flagged, not milestone movers). Itemized bumps +4+15+12+5 = +36 →
new mean = (1108 + 36)/16 = 1144/16 = 71.5. **Vision = 0.70×64.3 + 0.30×71.5 = 45.0 + 21.45 ≈ 66%** (was ≈64).

**Grade:** row flips **[Verified]** (each surface grep-confirmed this pass); the per-group C/P split
**[Inferred]** (conservative estimate against the §1.2 group counts, not a fresh per-row re-tally of all
631 FN rows); headline **[Inferred]** (additive delta on the ratified 35/40/25 model); milestone-programme
scores **[Speculative]** (judgment). Quote with the weights: ±10 stdlib-weight pts moves the headline ±~5.

**The finding that matters:** this is the first span since the E-baseline where the **stdlib-breadth drag
itself moved materially** (+6pp on the FN leg) — because the span shipped the actual TOP-20 blockers, not
perf/polish: **#2 HTTP client** and **#5 FS breadth** fell, **#6 Unicode** and **#3 sessions** went
partial. Parity +2 (62→64), Vision +2 (64→66). The remaining FN drag is now XML (#12), streams (FN-STREAM
15 GU), intl (#19), SPL heaps/PQ, and the mb_* long tail — which is exactly what the confirmed programme's
breadth slices (#9 collections, #10 TOP-20, #13 packs+XML+icu4x) target next.

### 4.10 Recompute at HEAD `580c6041` (2026-07-19 — overnight autonomous Wave-B: language ergonomics + collections + stdlib tails)

**Scope of the span (§4.9 HEAD `da3fc0c2` → `580c6041`, overnight autonomous run):** seven VERIFIED
shipped features, each surface-grep-confirmed this pass (Rule-11), plus a FRONTIER-MAP finding that
several §1.2 "gaps" are PHANTOM (already built) — flagged below, NOT credited here (that needs the owed
full §1.2 re-pass). Credited: **named arguments** `f(name: value)` FULL SCOPE (free fns + ctors +
methods, DEC-297) · **variadics** `f(int ...xs)` (DEC-298) · **Core.Math tail** (asin/acos/atan/atan2/
sinh/cosh/tanh/log1p/expm1/degToRad/radToDeg/log2/hypot — 13 fns) · **List.difference/intersection**
(typed-strict set ops, FN-ARR) · **String.capitalizeWords/translate** (ucwords/strtr, FN-STR) ·
**Core.Deque\<T\>** (DEC-300) · **Core.PriorityQueue\<T\>** (DEC-301).

**Row flips [Verified: registry/prelude surface grep + shipped commits this pass]:**

| # | Group | Tier | Was (§4.9) | Now | Δ score | Evidence |
|---|---|---|---|---|--|---|
| 1 | SYN (named args + variadics) | — | 104.5 C | +2.0 | SYN +2.0 | PHP-8.0 named args (colon spelling, transpiles 1:1) + PHP variadic `...` — both parity SYN, now COVERED (DEC-297/298) [Verified: `src/parser/`, differential] |
| 2 | FN-MATH tail (inverse-trig/hyperbolic/angle) | T3 | 0 C (T3 empty) | +13 C | +13 (T3) | asin/acos/atan/atan2/sinh/cosh/tanh/log1p/expm1/degToRad/radToDeg/log2/hypot; libm-backed, `log2`=ln/ln to match PHP `log(x,2)` [Verified: `src/native/math.rs`] — scientific math = T3 (rare in web code) |
| 3 | FN-STR (93) | T1 | 32 C (§4.9) | +2 C | +2.0 (T1) | capitalizeWords (ucwords) + translate (strtr) from GU [Verified: `src/native/text.rs`] |
| 4 | FN-ARR (74) | T1 | 27 C (§4.9) | +2 C | +2.0 (T1) | difference + intersection (typed-strict, filter semantics — NOT PHP array_diff loose) [Verified: `src/native/list_registry.rs`] |
| 5 | FN-SPL (39) | T2 | 3 C (§4.9) | +2 C | +2.0 (T2) | Core.Deque covers SplDoublyLinkedList/SplStack/SplQueue behaviours; Core.PriorityQueue covers SplPriorityQueue — both pure-Phorj, T?-on-empty (DEC-300/301) [Verified: `src/cli/preludes.rs`] |

**PHANTOM-GAP FINDING (flagged, NOT credited — needs the owed full §1.2 re-pass):** grep-verification
this run found several §1.2/§4.x "gaps" ALREADY built and shipped — **Core.Path** full (baseName/
directoryName/extension/fileStem/join), **Core.FileSystem** far broader than §4.9's "17 fns" credit
(same 17 + appendText + more), **Core.Random/Core.Hash** CSPRNG/HMAC/HKDF/PBKDF2 (crypto — the §4.9
recompute never credited a FN-HASH/FN-CRYPT flip), plus **match**/**Process**/**levenshtein** (some
already in §4.9). The true FN parity is therefore HIGHER than even §4.10 credits; a fresh per-row §1.2
re-tally (the standing process debt) is owed to bank it. Not credited here to avoid unverified inflation.

**Recomputed arithmetic (additive delta on §4.9 — T1 148.5/303, T2 49/140, T3 0/75; SYN 104.5/129; RT 13.5/18):**
- T1 score 148.5 → 148.5 + (2+2) = **152.5 / 303**
- T2 score 49 → 49 + 2 = **51 / 140**
- T3 score 0 → 0 + 13 = **13 / 75**
- FN usage-weighted = (3×152.5 + 2×51 + 1×13)/1264 = (457.5 + 102 + 13)/1264 = 572.5/1264 = **45.3%** (was 43.0%)
- SYN: 104.5 → 106.5 / 129 = **82.6%** (was 81.0% — named args + variadics)
- RT: unchanged **75.0%**
- **PHP-parity = 0.35×82.6 + 0.40×45.3 + 0.25×75.0 = 28.9 + 18.1 + 18.75 ≈ 66%** (was ≈64)
- Raw row-parity floor: FN raw 198 (§4.9) + 19 (4+2+13) = 217; (106.5 + 217 + 13.5)/665 = 337/665 ≈ **51%** (was ≈47 — the floor keeps closing on the weighted headline: gap 17pp→15pp, driven by the high-row-count Math tail)

**Vision %** — programme deltas on §4.9 (mean 1144/16 = 71.5): **M-Batteries 62→66** (collections
Deque/PQ + Math scientific tail), **GA-M12 82→84** (language ergonomics: named args + variadics — a core
call-site quality-of-life that PHP has). Itemized +4+2 = +6 → new mean = (1144 + 6)/16 = 1150/16 = 71.9.
**Vision = 0.70×65.75 + 0.30×71.9 = 46.0 + 21.6 ≈ 67%** (was ≈66).

**Grade:** row flips **[Verified]** (each surface grep-confirmed this pass + differential-tested);
C/P split and tier assignment **[Inferred]** (conservative — Math tail placed in T3 as rare scientific
math, not T1/T2, to avoid weighting inflation); headline **[Inferred]** (additive delta on the ratified
35/40/25 model); Vision milestone bumps **[Speculative]** (judgment). The phantom-gap undercount is
**[Verified]** as existing (grep-confirmed the modules ship) but its parity credit is **[Unverified]**
pending the per-row re-pass.

**The number the developer asked for:** **PHP-parity ≈ 66% · row-parity floor ≈ 51% · Vision ≈ 67%**
(from §4.9's 64/47/66). Modest +2 headline — the overnight span was language ergonomics + stdlib
long-tail, not another TOP-20 blocker fall — but the floor moved +4 (high-row Math tail) and the
PHANTOM-GAP finding means the *true* parity is higher still, pending the owed re-pass.

### 4.11 Recompute at HEAD `9a5deff6` (2026-07-19 — backed enums (DEC-302) + a TARGETED phantom-gap credit)

**Scope (§4.10 → `9a5deff6`):** the flagship this span is **backed enums (DEC-302, PHP 8.1)** — fully
built + gate-verified (2309 tests --all-features, byte-identical, example glob-gated): `enum Suit:
string {…}` / `enum Priority: int {…}` + `.value`, `Enum.cases()`, `Enum.from()`, `Enum.tryFrom()`.
This is a genuine PHP-8.1 LANGUAGE feature, not long-tail. Plus a TARGETED crediting of grep-VERIFIED
phantom gaps (§4.10's finding): **Core.Path** (baseName/directoryName/extension/fileStem/join) and
**crypto** (FN-HASH/RAND: hmac/equals/hkdf + CSPRNG secureBytes/secureInt), both shipped but never
credited by §4.9/4.10. **This is a TARGETED recompute, NOT the full 631-row re-tally (still owed).**

**Row flips [Verified: shipped + gate-green this span; phantom credits Inferred-uncredited]:**

| # | Group | Tier | Δ | Evidence |
|---|---|---|--|---|
| 1 | SYN — backed-enum decl (`: type` + `= value`) | — | SYN +1 | PHP-8.1 backed enum syntax, dev-ruled repr B [Verified: DEC-302, `parses_backed_enum_decl`] |
| 2 | FN-ENUM/SPL — `.value`/`cases()`/`from()`/`tryFrom()` | T2 | +4 C | The backed-enum method surface, byte-identical 3-leg [Verified: differential + example] |
| 3 | FN-PATH — Core.Path (5) | T1 | +5 C | baseName/directoryName/extension/fileStem/join — shipped, uncredited by §4.9/4.10 [Inferred] |
| 4 | FN-HASH/RAND — crypto (5) | T2 | +5 C | hmac/equals/hkdf + secureBytes/secureInt — shipped, uncredited [Inferred] |
| 5 | FN — tonight's stdlib (chunk/containsValue/product/none/sortDescending) | T1 | +5 C | DEC-303–308 [Verified: differentials] |

**Arithmetic (additive on §4.10 — T1 152.5/303, T2 51/140, T3 13/75; SYN 106.5/129; RT 13.5/18):**
- T1: 152.5 + 5 (Path) + 5 (stdlib) = **162.5**
- T2: 51 + 4 (enum methods) + 5 (crypto) = **60**
- T3: **13** (unchanged)
- FN weighted = (3×162.5 + 2×60 + 1×13)/1264 = (487.5 + 120 + 13)/1264 = 620.5/1264 = **49.1%** (was 45.3)
- SYN: 106.5 → 107.5/129 = **83.3%** (backed-enum syntax)
- RT: **75.0%**
- **PHP-parity = 0.35×83.3 + 0.40×49.1 + 0.25×75.0 = 29.2 + 19.6 + 18.75 ≈ 68%** (was ≈66)
- Raw floor: FN raw 217 + 4 + 5 + 5 = 231; (107.5 + 231 + 13.5)/665 = 352/665 ≈ **53%** (was ≈51)

**Vision** — backed enums is a language-completeness programme win: **GA-M12 84→86**. Mean (1150+2)/16 =
72.0. **Vision = 0.70×68 + 0.30×72 = 47.6 + 21.6 ≈ 69%** (was ≈67).

**Grade:** backed-enum + tonight's-stdlib credits **[Verified]** (shipped + gate-green + differential);
Path/crypto phantom credits **[Inferred]** (built confirmed by grep; uncredited-before inferred from
§4.9/4.10 not listing them); headline **[Inferred]** (additive on the 35/40/25 model). Still a TARGETED
recompute — a full per-row §1.2 re-tally would likely credit MORE (other phantom gaps) and remains owed.

**The number:** **PHP-parity ≈ 68% · floor ≈ 53% · Vision ≈ 69%** (from §4.10's 66/51/67). The +2 is a
real PHP-8.1 language feature (backed enums) + banking verified-already-built modules — not long-tail padding.

### 4.12 FULL §1.2 per-row re-tally at HEAD `d2f95509` (2026-07-19 — the owed 631-row re-pass)

**Scope:** the FULL §1.2 FN re-pass §4.9/4.10/4.11 all flagged as still owed (they did only targeted
credits). Every GU/GP the frozen §1.2 Notes named was grep-VERIFIED against the current codebase
(fresh-context subagent + main-session independent spot-check of the biggest + softest credits: Math
tail 11/11, String chunk/similarText/levenshtein 5/5, DB DBAL query/prepare/transaction/commit/rollback/
lastInsertId 6/6, and the discipline catches asinh/acosh/atanh/var_export all confirmed genuinely absent
— not inflated). Formula is the doc's actual §1.2 method **`(C + 0.5·P) / (total − NA − GD)`** (verified
against the frozen `(118 + 24.5)/518 = 27.5%`).

**Result — §1.2 SIMPLE-model FN coverage: 27.5% → ≈ 44.1%** `(202 + 0.5·53)/518 = 228.5/518` (honest
range ≈43–45%; ±1-per-soft-module → 43.6%). **81 phantom-gap rows** reclassified (63 GP→C, 18 GU→C) +
3 P→C. New FN tally (each group re-verified to sum to its rowcount; Σ = 631 ✓):

| | C | P | GP | GU | GD | NA |
|---|--|--|--|--|--|--|
| old (frozen) | 118 | 49 | 100 | 251 | 24 | 89 |
| **new** | **202** | **53** | **30** | **233** | 24 | 89 |

**Per-group flips (the 17 changed groups; durable audit trail — the subagent worktree auto-cleans).**
Format `C/P/GP/GU/GD/NA` old → new; every C-credit grep-cited to a `src/…` symbol:

| Group | old → new | Evidence |
|---|---|---|
| FN-STR (93) | 30/6/9/35/3/10 → 39/6/5/30/3/10 | +4 sprintf family→`text_registry "format"`; +5 chunk/capitalizeWords/translate/similarText/levenshtein |
| FN-ARR (74) | 26/12/2/22/2/10 → 30/10/0/22/2/10 | +2 zip/flatMap; +2 P→C difference/intersection (list-level) |
| FN-MATH (37) | 17/3/11/4/0/2 → 27/3/3/2/0/2 | +8 asin/acos/atan/atan2/sinh/cosh/tanh/hypot/log2/log1p/expm1; +2 CSPRNG secureInt/secureBytes |
| FN-PCRE (11) | 4/2/0/4/0/1 → 6/2/0/2/0/1 | +2 replaceCallback/quoteMeta |
| FN-DATE (27) | 5/5/2/8/2/5 → 6/5/2/7/2/5 | +1 date_parse→Instant.parse/Date.parse |
| FN-FS (55) | 8/2/7/34/0/4 → 13/2/3/33/0/4 | +4 mkdir/rmdir/scandir/opendir; +1 tempnam→tempDir |
| FN-HASH (8) | 0/1/4/3/0/0 → 4/1/0/3/0/0 | +4 hmac/equals/hkdf/pbkdf2 |
| FN-DB (10) | 0/0/10/0/0/0 → 8/2/0/0/0/0 | +8 DBAL `src/ext/database/` (2 P: PDO::quote/ATTR — softest) |
| FN-CURL (13) | 0/0/13/0/0/0 → 10/3/0/0/0/0 | +10 HttpClient `src/ext/http_client/` (3 P: curl_multi/setopt — softest) |
| FN-MB (22) | 0/2/12/4/0/4 → 6/2/6/4/0/4 | +6 codepointLength/grapheme/codepoints/unicodeUpper/unicodeLower |
| FN-SPL (39) | 2/2/4/26/0/5 → 10/2/1/21/0/5 | +3 Iterator protocol; +5 PriorityQueue/heaps/Deque preludes |
| FN-SESS (10) | 0/0/10/0/0/0 → 8/2/0/0/0/0 | +8 Session `src/ext/session/` (2 P: save_path/gc — softest) |
| FN-INTL (18) | 0/0/6/10/2/0 → 2/0/4/10/2/0 | +2 grapheme_strlen/str* |
| FN-RAND (4) | 1/1/0/0/0/2 → 2/0/0/0/0/2 | +1 P→C CSPRNG engine (secureBytes/secureInt) |
| FN-URL (10) | 3/0/3/3/0/1 → 7/0/0/2/0/1 | +3 parse_url/http_build_query/Uri-objects; +1 parse_str→decodeForm |
| FN-NET (9) | 0/0/1/8/0/0 → 1/0/1/7/0/0 | +1 mail()→`src/ext/mail/` |
| FN-MISC (18) | 2/1/2/3/3/7 → 3/1/1/3/3/7 | +1 error_log→Core.Log |

(Unchanged, verified: JSON, CRYPT, ICONV, CTYPE, FILTER, STREAM, SOCK, XML, FINFO, ZLIB, ZIP, PHAR, GD,
REFL, PROC, OB, VAR, FUNC — genuine gaps held, not inflated.)

Biggest banks (all grep-cited to a `src/…` symbol): typed **DBAL** `src/ext/database/` (FN-DB +8), **HttpClient**
`src/ext/http_client/` (FN-CURL +10), **Session** `src/ext/session/` (+8), **Math tail** asin/acos/atan/
atan2/sinh/cosh/tanh/hypot/log2/log1p/expm1 (FN-MATH +8), **HMAC/HKDF/PBKDF2/timing-safe** (FN-HASH +4),
**CSPRNG** secureBytes/secureInt (FN-MATH/RAND), **Uri** parse/build/objects (FN-URL +4), **mail()**
`src/ext/mail/` (FN-NET +1), **Iterator/PriorityQueue/Deque** preludes (FN-SPL +8), **mb/grapheme** unicode
(FN-MB +6), **String** chunk/capitalizeWords/translate/similarText/levenshtein (FN-STR +9), **error_log→
Core.Log** (FN-MISC +1). Per-group evidence detail was produced in a subagent worktree (ephemeral); the
reclassifications above are the durable record.

**⚠ RECONCILIATION WITH §4.11 — do NOT stack (the integration trap).** This is the SIMPLE row-weighted
§1.2 model; §4.11's ≈49.1% FN is the USAGE-WEIGHTED (T1/T2/T3) model. **They are the same reality counted
two ways — parallel views, not summands.** The +81 rows are NOT added onto §4.11's tiered numbers.
Consistency check passes: simple 44% < weighted 49% (expected — weighting favours the high-frequency
covered core). **The headline PHP-parity ≈68% is the WEIGHTED-model figure and is UNCHANGED by this
re-tally** (a different model). ⚠ **Critically — the weighted model is NOT built on the frozen §1.2; it
has been progressively recomputed and ALREADY contains most of these 81 rows** [Verified against the
§4.8/§4.9 flip tables]: **§4.8** folded in DB (+9.5 T2) + mail + syslog/Log; **§4.9** folded in HTTP
client (+10.5) + FS (+5.5) + Uri (+3) + mb/Unicode (+3.5) + sessions (+3.5) + SPL-iterator (+1.5) + STR
levenshtein/similarText (+2) + ARR zip + DATE — i.e. **~40 of the 81 rows are already in the weighted
49.1%**, plus §4.11's ~19 (Path/crypto/enum/stdlib). So **~60 of 81 are already weighted-credited**; the
genuine remaining re-tier upside is only **~20 rows** (Math tail 8, HASH 4, PCRE 2, INTL 2, a few SPL
heaps/URL) → a modest **~1–2pp** to the weighted FN, NOT the ~15pp the raw 81-count would suggest. **Net:
≈68% is a WELL-EVIDENCED floor with modest headroom — do NOT chase large phantom weighted upside; the
81-row simple-model gain is mostly already banked in the weighted headline.** [Grade: simple-model 44.1%
**[Verified]** (grep-cited + spot-checked); "~60/81 already weighted-credited, ~20 rows / ~1–2pp headroom"
**[Verified]** against the §4.8/§4.9/§4.11 flip tables.]

**Caveats:** (a) softest credits are module→N-rows [Inferred] — DB (8C from one DBAL), CURL (10C from one
HttpClient), SESS (8C) map a higher-level typed API onto N PHP rows; the matrix's COVERED-BETTER rule
supports it but the exact split is judgment (sensitivity trims to 43.6%). (b) ~26 credited rows (DB/CURL/
mail) sit behind NON-DEFAULT Cargo features — "covered when the feature is compiled in," consistent with
how PHP's own DB/curl are opt-in extensions and how the baseline already credited gated `regex`.
(c) reverse-check for over-credit/regression in old-C: none found (spot-checked Hash/Csv/Encoding/sort/
Option/Result still ship).

**GENUINE remaining gaps (the actionable future-work targets, by weight):** FN-FS fopen stream-handle
family + stat/chmod/symlink/realpath (33 GU — largest pool) · FN-SPL SplObjectStorage/ArrayAccess/
decorator-zoo (21) · FN-STREAM wrappers/filters (13) · FN-XML/DOM/XPath (12 — largest untouched family) ·
FN-SOCK raw sockets (10) · FN-INTL calendars/Normalizer/break-iterators (10) · FN-GD (7) · FN-ZLIB
compression (7) · **FN-CTYPE isLower/isUpper/isSpace/isPunct/isCntrl/isGraph/isPrint (7 — CHEAP, 5 more
validators next to the existing 5)** · FN-CRYPT sodium/openssl (5) · FN-STR wordwrap/strtok/strpbrk/strspn/
soundex/metaphone/strip_tags · **FN-MATH asinh/acosh/atanh (cheap add) + BigInt/GMP** · FN-FILTER
email/URL validators (Uri.parse exists → cheap wire) · FN-VAR var_export · FN-DATE DatePeriod/getdate ·
FN-PROC exec/system/proc_open subprocess.

### 4.13 ALIGNMENT MATRIX — feature × {transpile, lift, LSP} (2026-07-20 audit; the lift/transpile/LSP-align pass)

**Purpose:** the bidirectional-alignment audit the 2026-07-20 pass opened with — for every native/feature,
does it TRANSPILE (Phorj→PHP), LIFT (PHP→Phorj), and surface in the LSP? All figures grep-cited, cross-checked.

**Registry size (corrects the stale "286"):** **492 natives all-features / 465 default** (Core 333 + ext 159;
pure 374 / impure 118; **34 HigherOrder** re-entrant). "286" was a raw-`grep NativeFn {` undercount (missed
macro/helper rows). Source: `native::registry()` / `build()` `src/native/mod.rs:339-453`.

**TRANSPILE leg — 96 natives do NOT transpile (of 492):**
| Gap | Count | Code | Disposition (DEC-313) |
|---|---|---|---|
| Core.Native.Database | 40 | E-TRANSPILE-DB | PERMANENT (live I/O ≠ byte-identical) |
| Core.Native.Mail | 21 | E-TRANSPILE-MAIL | PERMANENT (SMTP/TLS, injection) |
| Core.Native.FileSystem | 18 | E-TRANSPILE-FS | **BUILDABLE → build it** (msg text out-of-contract) |
| Core.Native.Session | 7 | E-TRANSPILE-SESSION | **PERMANENT** (entropy sids + wall-clock TTL + persistent store) |
| Core.Native.HttpClient | 6 | E-TRANSPILE-HTTPCLIENT | PERMANENT (live network I/O) |
| Core.String Unicode tier | 4 | E-TRANSPILE-UNICODE | PERMANENT-per-call (mbstring/intl forbidden; codepoint tier transpiles) |
Plus non-native gates: `#[UncheckedOverflow]` fns (E-TRANSPILE-UNCHECKED), `spawn`/channels (E-CONCURRENCY-NO-PHP).
Everything else transpiles via the `NativeFn.php` emitter (`src/native/mod.rs:66`). Transpile leg is ~healthy.

**LIFT leg — the biggest gap: NO inverse native table at all.** `src/lift/lifter/*` has zero Core mapping
(only `echo`→`Core.Output`, `decls.rs:75`); a PHP `strlen($s)` lifts to an unresolved `strlen` call. Of 631 PHP
FN builtins: **~124 already have a forward Core equivalent** in transpile emitters (directly invertible — the
build seed), ~507 have no Core equivalent, 99 emitters use `__phorj_*` shims (need an idiom recognizer).
High-fan-in invertible builtins: `count`(19 emitters), `preg_match`(16), `array_values`(12), `implode`/`array_map`/
`array_merge`(9/7/7), `strlen`(6). Tier-2 loud-refusals (`src/lift/lifter/exprs.rs`+`decls.rs`): assign/`++`/`--`
as sub-expr, elvis, mixed keyed/positional array, enum-with-methods, default params, untyped fn/field, `array`
type needing List/Map/Set inference. **Fix = DEC-312** (`lift_from` facet on `NativeFn`; lifter derives its table
from the registry — one bidirectional SSOT).

**LSP leg — completion is skeletal + consumes zero registries.** `completion()` (`src/lsp/mod.rs:261`) returns
only current-buffer top-levels + locals + keywords; it's parse-dependent (returns `[]` on incomplete input like
`Output.` mid-type — verified live over stdio). NO member completion, NO import-path completion, NO Core-module/
native completion, NO project scan. The `.` trigger is advertised (`mod.rs:436`) but unfulfilled. Registries: 
`native::registry()`+`ext::EXTENSIONS` already `pub`; `CORE_MODULES` is `pub(super)` (`preludes.rs:869`), loader
`index_packages`/`peek_package`/`discover_roots` private (`src/loader/mod.rs`); `views/` not a search root. DEC-252
diagnostics ARE shared (`front_end_diagnostics`, drift-guarded). Fix = the LSP build slice (one enumeration API +
member/import/project completion + parse-tolerant cursor). Editors: vscode = thin client (surfaces server); phpstorm
= README stub, LSP4IJ path (server speaks correct LSP over stdio — verified).

**PERF leg (Invariant 18):** 40/465 natives benched (~8.6%, not 40/286). 30 wins / 16 losses. Loss classes:
#2b-winnable (mapmerge, stringcontains, the reduces) · physics-blocked linear-vs-C (listcontains/listfilter/
sumBy/minBy/maxBy — need representation work) · front-end-blocked (mapkeys/mapvalues — `List<Map>` not JIT-eligible,
`analyze.rs:1502`). isEmail/isUrl NOT benched. #2b (dispatch-overhead) = DEC-314 fresh-context slice.

**Craftsmanship flags:** see KNOWN_ISSUES §CRAFT-2026-07-20 (90 files >500-cap; 83 scattered `uses_*` flags;
1 at-risk dead-gate `interop.rs:144`; LSP advertises-but-doesn't-fulfill; stale-286).
