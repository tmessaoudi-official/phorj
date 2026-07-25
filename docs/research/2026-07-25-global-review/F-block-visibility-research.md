# F — Developer question #8: "can we add visibility/access in blocks inside a function?"

> *"and can we add visibility/access in blocks inside a function ? this need research and brainstorming
> give me what you understand and a recommendation"*

The phrasing is genuinely ambiguous — it admits **five** distinct language features. Per **Invariant 15**
I do not pick; below is what each one means, whether it exists today (probe-verified), what peer languages
do, and a recommendation for each. **The first question to the developer is therefore a disambiguation.**

## Ground truth — what a function body supports TODAY [all Verified with `target/release/phg`]

| Construct | Status | Evidence |
|---|---|---|
| Bare nested block `{ … }` creating a real scope | ✅ **SUPPORTED** | `{ int b = 2; }` type-checks; reading `b` after the block → `[E-UNKNOWN-IDENT]`, proving the scope is real, not cosmetic |
| Shadowing an outer local in a nested block | ✅ accepted by all frontends | but **P0 byte-identity break on the PHP leg** — see `P0-block-shadow-byte-identity.md` |
| Locals immutable by default + `mutable` opt-in | ✅ SUPPORTED | `int k = 0; k = k + 1;` → `[E-ASSIGN-IMMUTABLE]` with hint *"declare it `mutable`"*. A genuine better-than-PHP property |
| Visibility modifier on a local (`private int a = 1;`) | ❌ **parse error** | `expected an expression, found Private` |
| Named local/nested **function** declaration | ❌ **parse error** | `function helper(): int {…}` inside a body → `expected '(' after 'function', found Ident("helper")` |
| Local **class**/type declaration | ❌ **parse error** | `class Tmp {…}` inside a body → `expected an expression, found Class` |
| Closures / lambdas as expressions | ✅ SUPPORTED | `function(int v) => v > threshold`, arrow `v => v * 2`, pipe `\|>`; `ast::Expr::Lambda` (`exprs.rs:226`) |
| Closure capture semantics | ✅ **CORRECT, all three legs agree** | captures by value at creation; emitted as PHP arrow `fn($v) => …`. Probe: mutate the captured var after creation → `kept=2\|kept2=0` identical on vm/tw/php. **Positive attestation — do not re-litigate** |

## The five readings, each assessed

### F-i — "Bare blocks that scope their locals" → **ALREADY EXISTS**
If this is what was meant, the answer is *yes, we have it* — and the real news is that it is **broken on
the PHP leg** (the P0). Recommendation: rule the P0 fix; no new feature needed.

### F-ii — "Access modifiers on local variables" (`private int a = 1;`)
**Recommend AGAINST.** A local is already maximally restricted — it is visible only within its block, which
is *narrower* than anything `private` could express. `private` on a local is a no-op keyword, so it would be
pure noise that implies a distinction it cannot make. No mainstream language offers it (C++/Java/C#/Rust/
Kotlin/Swift/Go/TS all reject it). Adding it would violate the project's own naming/clarity doctrine
(CLAUDE.md: "crystal clear and intuitive"). [Grade: Inferred — from the scoping semantics verified above]

### F-iii — "Named nested functions inside a body" (with or without visibility)
**A real, defensible ergonomic gap.** Today the only in-body callable is a lambda *bound to a variable*,
which cannot self-recurse naturally and reads worse for a multi-statement helper. Peer languages
(Invariant 16 scan): Rust ✅ `fn` items in bodies (no visibility — always body-private); Python ✅ `def`;
JS/TS ✅ hoisted function declarations; Kotlin ✅ local `fun`; Swift ✅ nested `func`; Go ✅ only closures
(`f := func(){}`); C# ✅ local functions (since 7.0, deliberately **no** access modifiers); Java ❌ (only
local classes); **PHP ❌ — no nested function scoping at all** (a nested `function` declares a *global*
function on first execution, a notorious footgun).
**Key point on visibility:** every language that has local functions makes them *implicitly* body-private
and **forbids access modifiers on them** (C# rejects `private` on a local function explicitly). So "local
functions" and "visibility in blocks" are separate asks, and the industry answer to the second is *no*.
**Transpile note (Invariant 14 LADDER):** PHP's lack of nested scoping means the faithful lowering is a
closure assigned to a variable (`$helper = function(…){…};`) — which *does* map, so this is ladder rung (1)
transpile, not native-only. Recursion needs `use (&$helper)`, a known idiom.
**Recommendation:** worth a spec + ruling as its own small slice, WITHOUT visibility modifiers.

### F-iv — "Local class/type declarations in a body"
**Recommend AGAINST for now.** Java has them (local classes), C#/Kotlin/Swift/Rust allow local types too —
but the phorj payoff is low (a local class cannot be named in a signature, so its use is confined), while
the cost is high: the transpiler would have to hoist it to a top-level PHP class with a mangled name, and
that interacts with the package/PSR-4 model, reflection tables (`ClassTables`), and the decomposition
machinery. Poor cost/benefit versus F-iii. Revisit only if a concrete need appears.

### F-v — "Explicit capture lists — controlling what an inner block/closure may ACCESS"
This is arguably the most literal reading of *"visibility/access in blocks"*: not who can see the block,
but **what the block can see**. Peer scan: C++ `[x, &y]` mandatory; Swift `[weak self]`; **PHP
`function() use ($x)` mandatory** (arrow `fn()` auto-captures); Rust `move`; JS/Python/Kotlin implicit.
Phorj today captures **implicitly by value** and — verified above — does so *correctly and byte-identically*.
**Recommendation: do NOT add mandatory capture lists.** They would be pure ceremony on top of a
capture model that is already sound and already matches the PHP arrow-`fn` lowering. The one *optional*
refinement worth considering later is an explicit `mutable`-capture form if by-reference capture is ever
wanted — but nothing needs it today, and by-ref capture would immediately re-open a PHP-parity question.

## Recommendation summary (developer rules — Invariant 15)

1. **Disambiguate first** — ask which of F-i…F-v was meant. My reading of the phrasing, given it sits in a
   list of concrete gaps, is **F-iii (named nested functions)** with F-i as the likely trigger for the thought.
2. **Regardless of the answer, the P0 (block-shadow byte-identity) must be ruled and fixed** — it is the
   thing that is actually broken about blocks today.
3. **Recommend NO to F-ii, F-iv, F-v**; recommend a **spec+ruling for F-iii without visibility modifiers**,
   because every peer language that has the feature deliberately omits access control on it.
4. Standing principle worth recording: **"visibility" is a top-level/member-axis concept; inside a function
   body the axis is *lifetime/scope*, not access.** Conflating them is what makes F-ii feel appealing and is
   exactly the same conflation the visibility spec already caught once (G3: *"The dev's 'we need it' is
   really a request for the SUBTREE level — two DIFFERENT axes; must not conflate"*).
