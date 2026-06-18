# PHP Language Constructs — Exhaustive Inventory & Phorge Mapping

> **Scope.** Every *language* construct (syntactic/semantic feature) of PHP from the PHP 3/4 era through 8.6 (in-dev), **excluding** anything deprecated or removed as of 8.5/8.6 (those are noted in §0 as excluded). Library functions are out of scope except where they were once language-level (e.g. `create_function`, `each`).
> **Target.** Phorge: statically-typed, immutable-by-default, VM-compiled + PHP-transpiled. The "Phorge mapping" column states the equivalent Phorge surface (today or roadmapped).
>
> **Buckets:** ✅ Phorge has ≥ (equal or richer) · 🔶 partial · 🔲 roadmapped:`<milestone>` · ❌ reject-by-design (dynamic/unsafe, no idiomatic safe analogue).
> **Verdict:** BETTER (Phorge safer/richer) · SAME · SAME+syntax (same semantics, different/nicer syntax) · WORSE→reject (PHP feature has no safe place in Phorge).
>
> **Sources verified** (see §10): php.net manual (type declarations, generators, goto, control structures, migration84/85.deprecated), wiki.php.net RFCs (pipe-operator-v3, throw_expression, new_in_initializers, first_class_callable, deprecate-backtick-operator-v2), php.watch version pages.

---

## §0. EXCLUDED — deprecated or removed as of 8.5/8.6 (NOT mapped)

These are noted for completeness and explicitly **excluded** from the mapping tables below.

| Construct | Status | Note |
|---|---|---|
| Backtick execution operator `` `cmd` `` | **Deprecated 8.5**, removal 9.0 | Alias of `shell_exec()`. Was already ❌ reject-by-design (dynamic shell exec); now also dead. |
| `(boolean)` `(integer)` `(double)` `(binary)` cast names | **Deprecated 8.5** | Non-canonical aliases of `(bool)/(int)/(float)/(string)`. |
| `case 1;` (semicolon-terminated case) | **Deprecated 8.5** | Must use `case 1:`. |
| `create_function()` | **Removed 8.0** (dep. 7.2) | String-eval closure factory. Was ❌ (eval-based). |
| `each()` | **Removed 8.0** (dep. 7.2) | Internal-pointer iterator. |
| Implicitly-nullable params `f(T $a = null)` | **Deprecated 8.4** | Must write `?T`/`T\|null`. Phorge requires explicit `T?` already. |
| `0 ** -negative` / `pow(0,-n)` | **Deprecated 8.4** | Division-by-zero; use `fpow`. |
| class named exactly `_` | **Deprecated 8.4** | Reserved for future use. |
| `trigger_error(…, E_USER_ERROR)` | **Deprecated 8.4** | Library, listed for completeness. |
| `E_STRICT` constant | **Deprecated 8.4** | Error level already removed. |
| `${x}` / `"${ expr }"` string interpolation | **Deprecated 8.2** | Variable-variable-style interpolation removed; `"{$x}"` survives. *Tracked in §4 as excluded variant.* |
| `(real)` cast, `(unset)` cast | Removed (7.x / 8.0) | — |

---

## §1. Control flow

| Construct | First ver | Phorge mapping | Bucket | Verdict |
|---|---|---|---|---|
| `if` / `elseif` / `else` | PHP 3 | `if`/`else if`/`else` statement | ✅ | SAME |
| `switch` (+ fall-through) | PHP 3 | `match` (exhaustive, no fall-through) | 🔶 | BETTER — exhaustive, no implicit fall-through |
| `match` expression (arms, no fall-through, strict `===`) | 8.0 | `match` over enums/`T?` exhaustive | ✅ | SAME+syntax — Phorge `match` predates value-level union breadth |
| `while` | PHP 3 | `while` loop | ✅ | SAME |
| `do`…`while` | PHP 3 | (none) — `while` only | 🔲 roadmapped:M3 | SAME (gap: post-test loop not yet) |
| `for (init; cond; step)` | PHP 3 | `for (int i in a..b)` range form | 🔶 | BETTER — range loop is bounds-safe, no manual step bugs |
| `foreach ($a as $v)` | PHP 4 | `for (x in xs)` for-in over `List<T>` | ✅ | SAME+syntax |
| `foreach ($a as $k => $v)` | PHP 4 | (none) — needs Map/index pairs | 🔲 roadmapped:M3 | SAME (Map + keyed iteration roadmapped) |
| `foreach ($a as &$v)` (by-ref mutate) | PHP 5 | (none) — immutable, no aliasing | ❌ reject-by-design | WORSE→reject — references + in-place mutation |
| `break` / `break N` | PHP 3 / 4 | `break` (single level) | 🔶 | BETTER — multi-level `break N` rejected as error-prone |
| `continue` / `continue N` | PHP 3 / 4 | `continue` (single level) | 🔶 | BETTER — `continue N` rejected (same reasoning) |
| `goto label;` + `label:` | 5.3 | (none) | ❌ reject-by-design | WORSE→reject — arbitrary jumps defeat static reasoning |
| `return` | PHP 3 | `return` | ✅ | SAME |
| `declare(strict_types=1)` | 7.0 | implicit — always strict-typed | ✅ | BETTER — strictness is the only mode, no opt-in |
| `declare(ticks=N)` | 4.3 | (none) | ❌ reject-by-design | WORSE→reject — runtime tick callbacks, dynamic/global |
| `declare(encoding=…)` | 5.3 | (none) — UTF-8 source assumed | ❌ reject-by-design | WORSE→reject — source re-encoding directive |
| Alternative syntax `if:`…`endif;`, `for:`…`endfor;`, `foreach:`…`endforeach;`, `while:`…`endwhile;`, `switch:`…`endswitch;` | PHP 4 | (none) — brace syntax only | ❌ reject-by-design | WORSE→reject — template-interleaving syntax, no use w/o inline HTML |
| `try`/`catch`/`finally` + `throw` (statement) | 5.0 / 5.5 | (none) | 🔲 roadmapped:M3 | SAME (exceptions roadmapped M3) |

---

## §2. Operators (ALL)

| Construct | First ver | Phorge mapping | Bucket | Verdict |
|---|---|---|---|---|
| Arithmetic `+ - * / %` | PHP 3 | same, **checked** overflow | ✅ | BETTER — checked arithmetic, faults not silent wrap/inf |
| Exponentiation `**` | 5.6 | (none yet) | 🔲 roadmapped:M3 | SAME (use repeated mul / stdlib `math.pow`) |
| String concat `.` | PHP 3 | `+` on string / interpolation | 🔶 | SAME+syntax — interpolation preferred, `.` not used |
| `=` assignment | PHP 3 | `var x = …` binding (immutable) | 🔶 | BETTER — binding, no rebinding (no aliasing footguns) |
| Compound assign `+= -= *= /= %= **= .=` | PHP 3 / 5.6 | (none) — no mutation | 🔲 roadmapped:M3 | SAME (needs reassignment, M3) |
| `??=` null-coalesce assign | 7.4 | (none) — needs mutation | 🔲 roadmapped:M3 | SAME (have `??` expr) |
| Bitwise assign `&= \|= ^= <<= >>=` | PHP 3 | (none) — no mutation | 🔲 roadmapped:M3 | SAME |
| `== === != !== <> <=>` | PHP 3 / 4 / 7.0 | `==` `!=` (typed, no juggling) | 🔶 | BETTER — one equality, no `==` juggling; `<=>` via `match`/compare |
| `< <= > >=` | PHP 3 | same (typed) | ✅ | SAME |
| Ternary `c ? a : b` | PHP 3 | expression-`if (c) { a } else { b }` | ✅ | BETTER — mandatory else, single-expr arms, typed |
| Short ternary `a ?: b` | 5.3 | `a ?? b` (for null) | 🔶 | BETTER — `?:` relied on falsy-juggling; `??` is null-precise |
| Null-coalesce `??` | 7.0 | `??` | ✅ | SAME |
| Logical `&& \|\| !` | PHP 3 | `&& \|\| !` (short-circuit) | ✅ | SAME |
| Logical `and or xor` (low-prec keywords) | PHP 3 | (none) | ❌ reject-by-design | WORSE→reject — duplicate ops w/ surprising precedence |
| Bitwise `& \| ^ ~ << >>` | PHP 3 | (none yet) | 🔲 roadmapped:M3 | SAME (int ops roadmapped) |
| `instanceof` | 5.0 | `match` over enum / `is` (none yet) | 🔲 roadmapped:M3 | SAME (RTTI via match; runtime type test roadmapped) |
| `clone` (+ `clone with` 8.5) | 5.0 / 8.5 | (none) — values are immutable, copy is free | ✅ | BETTER — value semantics; clone is a no-op concept |
| Casts `(int)(float)(string)(bool)` | PHP 3 | explicit conversion fns (stdlib) | 🔶 | BETTER — no silent lossy juggling; conversions explicit |
| `(array)` `(object)` casts | 4 / 5 | (none) | ❌ reject-by-design | WORSE→reject — dynamic shape coercion |
| Error-suppression `@expr` | PHP 3 | (none) | ❌ reject-by-design | WORSE→reject — swallows errors silently |
| Execution `` `cmd` `` (backticks) | PHP 3 | (none) — **deprecated 8.5 too** | ❌ reject-by-design | WORSE→reject — dynamic shell exec |
| Nullsafe `?->` | 8.0 | `?.` safe access | ✅ | SAME+syntax |
| Pipe `\|>` | 8.5 | (none yet) | 🔲 roadmapped:Track A/S3 | SAME (planned alongside lambdas) |
| `instanceof` short-circuit / spaceship in sort | — | covered above | — | — |

---

## §3. Declarations

| Construct | First ver | Phorge mapping | Bucket | Verdict |
|---|---|---|---|---|
| `function f(...)` | PHP 3 | `fn f(...)` declaration | ✅ | SAME+syntax |
| Default params `f($x = 1)` | PHP 3 | (none yet) | 🔲 roadmapped:M3 | SAME |
| Param type decls / return types | 5.0 / 7.0 / 7.1 | mandatory typed params + return | ✅ | BETTER — types are mandatory, not optional |
| By-ref return `&f()` | 5.0 | (none) | ❌ reject-by-design | WORSE→reject — reference aliasing |
| By-ref params `f(&$x)` | PHP 4 | (none) — immutable | ❌ reject-by-design | WORSE→reject — out-params via aliasing |
| Variadics `f(...$xs)` | 5.6 | (none yet) | 🔲 roadmapped:M3 | SAME |
| Named-argument call `f(x: 1)` | 8.0 | (none yet) | 🔲 roadmapped:M3 | SAME |
| `const NAME = …` (compile-time) | 5.0 | top-level immutable `var`/const | ✅ | SAME |
| `define('NAME', …)` (runtime const) | PHP 4 | (none) | ❌ reject-by-design | WORSE→reject — runtime/dynamic global define |
| `class` | PHP 4/5 | `class` + ctor promotion + methods | 🔶 | SAME+syntax (no inheritance yet) |
| `abstract class` | 5.0 | (none yet) | 🔲 roadmapped:M3 S5 | SAME |
| `final class` / `final method` | 5.0 | (none) — classes are final-by-default (no inheritance) | ✅ | BETTER — closed by default |
| `interface` | 5.0 | (none yet) | 🔲 roadmapped:M3 S5 | SAME |
| `trait` (+ `insteadof`, `as`) | 5.4 | (none) | 🔲 roadmapped:M3 S5 (as mixins) | SAME — locked as traits/mixins, NOT multiple inheritance |
| `enum` (pure + backed) | 8.1 | enums **with payloads** | ✅ | BETTER — algebraic payloads, richer than PHP backed enums |
| `namespace` + `use` | 5.3 | `package` + `import` (mandatory) | ✅ | BETTER — mandatory packaging, "nothing in the wind", folder=path |
| Group `use {A, B}` | 7.0 | (none) — per-leaf import | 🔶 | SAME+syntax |
| `use … as Alias` | 5.3 | `import a.b as c` | ✅ | SAME |
| `global $x` | PHP 3 | (none) | ❌ reject-by-design | WORSE→reject — mutable global scope import |
| Function-local `static $x` | PHP 3 | (none) — no per-call mutable state | ❌ reject-by-design | WORSE→reject — hidden persistent mutable state |
| Class `static` member/method | 4 / 5 | (none yet) | 🔲 roadmapped:M3 S5 | SAME |

---

## §4. Expressions / literals

| Construct | First ver | Phorge mapping | Bucket | Verdict |
|---|---|---|---|---|
| Closures `function() use($x){}` | 5.3 | (none yet) | 🔲 roadmapped:Track A/S3 | SAME |
| `use (&$x)` by-ref capture | 5.3 | (none) | ❌ reject-by-design | WORSE→reject — closure-over-reference mutation |
| `static function()` closures | 5.4 | (none yet — Phorge closures will be `this`-free by default) | 🔲 roadmapped:Track A/S3 | SAME |
| Arrow fn `fn($x) => $x+1` | 7.4 | (none yet) | 🔲 roadmapped:Track A/S3 | SAME |
| Generators `yield`, `yield k=>v`, `yield from` | 5.5 / 7.0 | (none) | 🔲 roadmapped:M3+ | SAME (lazy seqs; later milestone) |
| `list($a,$b)=…` / `[$a,$b]=…` destructuring (+ keyed, nested) | 5.0 / 7.1 | (none yet) | 🔲 roadmapped:M3 | SAME (with tuples/Map) |
| Anonymous class `new class {…}` | 7.0 | (none) | ❌ reject-by-design | WORSE→reject — unnamed nominal type defeats nominal typing |
| `new C(...)` | PHP 4/5 | `new C(...)` | ✅ | SAME |
| `new C()->method()` (no parens, 8.4) | 8.4 | method-chain on `new` already works | ✅ | SAME+syntax |
| `new` in initializers (default param, const) | 8.1 | (none yet — no default params) | 🔲 roadmapped:M3 | SAME |
| `throw` as expression | 8.0 | (none yet — no exceptions) | 🔲 roadmapped:M3 | SAME |
| First-class callable `f(...)` | 8.1 | (none yet) | 🔲 roadmapped:Track A/S3 | SAME |
| Heredoc `<<<EOT` | PHP 4/5.3 | string interpolation `"…"` | 🔶 | SAME+syntax — interpolation covers it; multiline literal later |
| Nowdoc `<<<'EOT'` | 5.3 | raw string literal | 🔶 | SAME+syntax |
| Interp `"$x"` (simple) | PHP 3 | `"{x}"` interpolation | ✅ | SAME+syntax |
| Interp `"{$x->y}"` (complex) | PHP 4 | `"{x.y}"` interpolation | ✅ | SAME+syntax |
| Interp `"${x}"` | PHP 3 | — **deprecated 8.2**, excluded | ❌ (excluded) | WORSE→reject — variable-variable interpolation |
| Numeric literals `_` sep, `0x`,`0o`,`0b`, floats `1.2e3` | varies (8.1 `0o`, 7.4 `_`) | int/float literals incl. `0x/0o/0b/_` | ✅ | SAME |
| `array(…)` long form | PHP 4 | (none) — `[…]` only | 🔶 | SAME — `array()` is just old syntax |
| `[…]` short array | 5.4 | `List<T>` literal `[…]` | 🔶 | BETTER — typed homogeneous list, not heterogeneous hashmap |
| Spread `...$xs` in call | 5.6 | (none yet) | 🔲 roadmapped:M3 | SAME |
| Spread `...$xs` in array literal | 7.4 | (none yet) | 🔲 roadmapped:M3 | SAME |
| String-keyed spread in array (8.1) | 8.1 | (none — needs Map) | 🔲 roadmapped:M3 | SAME |
| Range literal — *not native PHP* (PHP uses `range()`) | — | `a..b` / `a..=b` integer ranges | ✅ | BETTER — Phorge has native ranges, PHP only `range()` fn |
| Bytes literal — *not native PHP* (`string` is bytes) | — | `b"…"` (`\xHH`) bytes primitive | ✅ | BETTER — distinct bytes type vs PHP byte-string conflation |

---

## §5. Variable / scope semantics

| Construct | First ver | Phorge mapping | Bucket | Verdict |
|---|---|---|---|---|
| Variable variables `$$x`, `${$name}` | PHP 3/4 | (none) | ❌ reject-by-design | WORSE→reject — name computed at runtime, un-analyzable |
| References `$a = &$b` | PHP 4 | (none) — value semantics | ❌ reject-by-design | WORSE→reject — aliasing breaks immutability + static reasoning |
| `isset($x)` | PHP 3 | `if (var x = opt)` / `??` / `?.` | ✅ | BETTER — optionals make presence type-checked, not runtime |
| `empty($x)` | PHP 3 | explicit comparison | ✅ | BETTER — no falsy-juggling ambiguity |
| `unset($x)` | PHP 3 | (none) — bindings immutable, no removal | ❌ reject-by-design | WORSE→reject — runtime symbol-table mutation |
| `list()` (as lvalue) | 5.0 | see §4 destructuring | 🔲 roadmapped:M3 | SAME |
| Superglobals `$_GET $_POST $GLOBALS …` | 4.1 | (none) — explicit params / M6 Request | ❌ reject-by-design (+🔲 M6 for HTTP) | WORSE→reject — ambient mutable global state |
| Variable scope (function = isolated, no block scope) | PHP 3 | block + lexical scope, immutable bindings | ✅ | BETTER — true lexical block scope |
| Variable function call `$fn()` / `$obj->$m()` | PHP 3/4 | (none) — first-class callables later, typed | ❌ reject-by-design (string dispatch) | WORSE→reject — dynamic string-named dispatch |

---

## §6. Type-system constructs

| Construct | First ver | Phorge mapping | Bucket | Verdict |
|---|---|---|---|---|
| Scalar type decls `int float string bool` | 7.0 | mandatory scalar types | ✅ | BETTER — mandatory + strict always |
| Nullable `?T` | 7.1 | optional `T?` (`Ty::Optional`) | ✅ | BETTER — compile-time non-null guarantee on non-optional `T` |
| Union `T\|U` | 8.0 | (none — no type variable/union) | 🔲 roadmapped:M3 | SAME — enums cover tagged unions today |
| Intersection `A&B` | 8.1 | (none) | 🔲 roadmapped:M3 S5 | SAME (needs interfaces) |
| DNF `A&B\|C` | 8.2 | (none) | 🔲 roadmapped:M3 S5 | SAME |
| `void` | 7.1 | unit/no return type | ✅ | SAME |
| `never` | 8.1 | (none yet — faults are the model) | 🔲 roadmapped:M3 | SAME (bottom type for diverging fns) |
| `mixed` | 8.0 | (none) | ❌ reject-by-design | WORSE→reject — dynamic any; defeats static typing |
| `iterable` | 7.1 | `List<T>` / for-in | 🔶 | SAME — concrete instead of structural |
| `callable` | 5.4 | (none yet — typed fn types later) | 🔲 roadmapped:Track A/S3 | SAME (will be typed `(T)->U`, richer) |
| `object` (catch-all) | 7.2 | (none) | ❌ reject-by-design | WORSE→reject — untyped object |
| `self` / `parent` | 5.0 | `self`-like via class name; no `parent` (no inheritance) | 🔶 | SAME — `parent` is N/A without inheritance |
| `static` return type (late static binding) | 8.0 | (none) | 🔲 roadmapped:M3 S5 | SAME |
| `true` / `false` / `null` standalone types | 8.2 | `bool` + `T?`/`Null` | 🔶 | SAME — covered by bool + optional |
| `readonly` property (8.1) / readonly class (8.2) | 8.1 / 8.2 | **default** — all fields immutable | ✅ | BETTER — immutability is the default, not a keyword |
| Typed class constants | 8.3 | typed consts already | ✅ | SAME |
| Property hooks `get`/`set` | 8.4 | (none) — fields are plain immutable reads | 🔲 roadmapped:M3 (accessors) | SAME — partial: computed reads via methods |
| Asymmetric visibility `private(set)` | 8.4 | (none) — fields write-once at construction | ✅ | BETTER — write-once subsumes private-set |
| Generics `<T>` — *not native PHP* (docblock only) | — | (none — no type variable) | 🔲 roadmapped:M3+ | SAME (PHP lacks real generics; Phorge plans them) |

---

## §7. Attributes

| Construct | First ver | Phorge mapping | Bucket | Verdict |
|---|---|---|---|---|
| Attribute syntax `#[Attr(args)]` | 8.0 | (none) | 🔲 roadmapped:M3+ | SAME — no metadata-attribute system yet |
| Built-in `#[Attribute]` | 8.0 | (none) | 🔲 roadmapped:M3+ | SAME |
| `#[Override]` | 8.3 | (none — needs inheritance) | 🔲 roadmapped:M3 S5 | SAME |
| `#[ReturnTypeWillChange]` | 8.1 | (none) | 🔲 roadmapped:M3+ | SAME (compat shim, low priority) |
| `#[AllowDynamicProperties]` | 8.2 | (none) | ❌ reject-by-design | WORSE→reject — re-enables dynamic props, anti-Phorge |
| `#[SensitiveParameter]` | 8.2 | (none) | 🔲 roadmapped:M3+ | SAME |
| `#[Deprecated]` | 8.4 | lint channel (`W-*` warnings) | 🔶 | SAME — Phorge has a warning channel, not attributes yet |
| `#[NoDiscard]` (8.5) | 8.5 | (none) | 🔲 roadmapped:M3+ | SAME |

---

## §8. ❌ reject-by-design — consolidated list (every dynamic/unsafe construct)

These have **no safe analogue** in Phorge and are rejected as a design choice (each defeats either immutability, static reasoning, or determinism):

1. `foreach (… as &$v)` — by-reference iteration mutation.
2. `goto` / labels — arbitrary control-flow jumps.
3. `declare(ticks)` — runtime tick callbacks (global side effect).
4. `declare(encoding)` — source re-encoding directive.
5. Alternative `if:`/`endif;` (+ for/foreach/while/switch) — HTML-template-interleave syntax.
6. `and` / `or` / `xor` low-precedence logical keywords — duplicate ops with surprising precedence.
7. `(array)` / `(object)` casts — dynamic shape coercion.
8. `@expr` error suppression — silently swallows errors.
9. `` `cmd` `` backtick execution — dynamic shell exec (also deprecated 8.5).
10. `define()` — runtime/dynamic global constant.
11. `global $x` — mutable global scope import.
12. function-local `static $x` — hidden persistent mutable state.
13. by-ref return `&f()` and by-ref params `f(&$x)` — reference aliasing.
14. `use (&$x)` by-ref closure capture — closure-over-reference mutation.
15. anonymous class `new class{}` — unnamed nominal type defeats nominal typing.
16. `"${x}"` interpolation — variable-variable interpolation (also deprecated 8.2).
17. Variable variables `$$x` / `${$name}` — runtime-computed names, un-analyzable.
18. References `$a = &$b` — aliasing breaks immutability + static reasoning.
19. `unset($x)` — runtime symbol-table mutation.
20. Superglobals `$_GET`/`$GLOBALS`/… — ambient mutable global state (HTTP request modeled explicitly in M6 instead).
21. Variable function/method dispatch `$fn()` / `$obj->$m()` — dynamic string-named dispatch.
22. `mixed` type — dynamic any, defeats static typing.
23. `object` catch-all type — untyped object.
24. `#[AllowDynamicProperties]` — re-enables dynamic properties (anti-Phorge).

---

## §9. 🔲 roadmapped — consolidated list with milestones

| Construct(s) | Milestone |
|---|---|
| `do…while` loop | M3 |
| keyed `foreach` (`$k => $v`) | M3 (with Map) |
| `try`/`catch`/`finally` + `throw` (statement & expression) | M3 |
| Compound assignment `+= … .=`, `??=`, bitwise-assign | M3 (needs reassignment/mutation) |
| Exponentiation `**` (operator; have stdlib pow) | M3 |
| Bitwise `& \| ^ ~ << >>` | M3 |
| `instanceof` / runtime type test | M3 |
| Default params, named-arg calls, variadics `...$x` | M3 |
| Spread `...` in calls and array/list literals | M3 |
| `list()`/`[]` destructuring (keyed + nested) | M3 (with tuples/Map) |
| `new` in initializers | M3 (after default params) |
| Union types `T\|U` | M3 |
| `never` bottom type | M3 |
| `define`-free runtime consts — N/A | — |
| Property hooks / accessors | M3 |
| Closures, arrow fns, first-class callables `f(...)`, pipe `\|>`, `static`-closures | Track A / S3 |
| `core.list` map/filter/reduce, `core.json` | Track A/S3 (unblocked by lambdas/generics) |
| Generators (`yield`/`yield from`) | M3+ (lazy sequences) |
| `abstract`/`interface`/`trait`(as mixins)/class-`static`/late-static-binding/`#[Override]`/`self`+`parent` | M3 S5 |
| Intersection `A&B`, DNF `A&B\|C` | M3 S5 (needs interfaces) |
| Attributes `#[Attr]` + built-ins (`#[Attribute]`, `#[SensitiveParameter]`, `#[NoDiscard]`, `#[Deprecated]`→partial) | M3+ |
| Generics `<T>` (real, not docblock) | M3+ |
| Superglobals replacement (HTTP Request/Response) | M6 (web) |

---

## §10. Sources verified

- php.net — Type declarations: <https://www.php.net/manual/en/language.types.declarations.php> (scalar 7.0, `?T` 7.1, `void`/`iterable` 7.1, `object` 7.2, union/`mixed`/`static` 8.0, `never`/intersection 8.1, `null`/`false`/`true`/DNF 8.2, typed class consts 8.3).
- php.net — Generators syntax: <https://www.php.net/manual/en/language.generators.syntax.php> (generators 5.5, `yield from` 7.0; not deprecated).
- php.net — `goto`: <https://www.php.net/manual/en/control-structures.goto.php> (5.3, still supported, not deprecated).
- php.net — Migration 8.4 deprecated: <https://www.php.net/manual/en/migration84.deprecated.php> (implicit-nullable params, `0 ** -n`, class `_`, `E_STRICT`, `E_USER_ERROR`).
- php.net — Migration 8.5 deprecated: <https://www.php.net/manual/en/migration85.deprecated.php> (backtick operator, non-canonical cast names, `case ;`).
- wiki.php.net — Pipe operator v3: <https://wiki.php.net/rfc/pipe-operator-v3> (`|>` added 8.5).
- wiki.php.net — Throw expression: <https://wiki.php.net/rfc/throw_expression> (throw-as-expression 8.0).
- wiki.php.net — New in initializers: <https://wiki.php.net/rfc/new_in_initializers> (8.1).
- php.net — First-class callable syntax: <https://www.php.net/manual/en/functions.first_class_callable_syntax.php> (`f(...)` 8.1).
- wiki.php.net — Deprecate backtick operator v2: <https://wiki.php.net/rfc/deprecate-backtick-operator-v2> (8.5 deprecation).
- php.net 8.4 release (property hooks, asymmetric visibility, `new` without parens): <https://www.php.net/releases/8.4/en.php>.
- php.net 8.5 release (pipe operator, `clone with`): <https://www.php.net/releases/8.5/en.php>.
