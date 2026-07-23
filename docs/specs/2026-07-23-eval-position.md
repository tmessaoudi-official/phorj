# SPEC — `eval`: position paper (DEC-331 D10c, the "spec tomorrow" hold)

> Status: **RULED (dev, 2026-07-23): rejection + substitutes ACCEPTED; `Core.Sandbox` BUILDS IN V1** (scope change from the frozen recommendation — see §5). D10c ruling being elaborated: full `eval`
> stays REJECTED (breaks the closed-language / no-RCE / soundness guarantee); the only open
> avenue is a sandboxed typed sub-interpreter, gated on a concrete use case.

## 1. Why full `eval` stays out (the guarantee it would break)

phorj's core promises are static: every program that checks is typed, every fault string is
canonical, `run`/`check`/`transpile` never execute unvetted text, and the transpiled PHP is
byte-identical. `eval(string)` at full power destroys all four at once: untyped code enters at
runtime (soundness), user input becomes executable (RCE-by-design — the #1 PHP CVE class),
and the PHP leg would need `eval()` emission (the exact construct the transpile floor bans).
No partial mitigation preserves the guarantee — this is a rejection, not a deferral.

## 2. What PHP uses `eval` for — and the phorj-native substitute for each

| PHP eval use case | phorj substitute (shipped unless noted) |
|---|---|
| config-driven values / feature flags | `#[Config]` providers (typed, DEC-318) |
| plugin / extension loading | extension SPI + userland `.phg` packages |
| dynamic dispatch by name | `#[Invoke]` + overloading; `Core.Reflection` (read-only) |
| templating / codegen | compile-time expansion (`cli::check_and_expand` sugar family) |
| REPL / notebook evaluation | the `phg` compiler AS a library (see §3) |
| user-supplied formulas (spreadsheet class) | the §3 sandboxed sub-interpreter, IF ruled in |

## 3. The one open avenue: a sandboxed TYPED sub-interpreter (`Core.Sandbox`, sketch)

NOT `eval`. A capability-scoped evaluator over a DECLARED interface:

```phg
Sandbox sb = Sandbox.expressionsOnly();          // no IO, no natives beyond Math/String pure set
sb.bind("price", 12.5);
sb.bind("qty", 3);
Result<float, string> r = sb.evalFloat("price * qty * 1.2");   // typed result or error
```

Properties that keep the guarantee intact: (1) the input is CHECKED by the real checker
against a closed, caller-declared binding environment — type errors are values, not injections;
(2) capability floor = pure expressions only (no calls except a whitelisted pure-native set,
no assignment, no loops in v1); (3) runs on the tree-walker (the oracle — no JIT of untrusted
input, ever); (4) **transpile: Ladder tier 2** — `E-TRANSPILE-SANDBOX` (PHP has no safe
equivalent; a `__phorj_*` reimplementation of the checker in PHP is out of scope), so programs
using it are native-only, loudly. (5) fuel/step limit + depth cap (no DoS via `while(true)` if
loops ever land).

**RULED (dev, 2026-07-23): `Core.Sandbox` BUILDS IN V1** with exactly this scope — pure
expressions only, caller-bound environment, tree-walker-only execution, `E-TRANSPILE-SANDBOX`
(native-only, loud), fuel/step caps. The dev accepted the four compromises explicitly
(native-only programs; slowest engine by design; a frozen public API over checker internals;
API-shape risk without a live consumer). Target consumers: user formula fields, pricing
rules, report expressions, config arithmetic.

## 4. Final position (ruled)

§1 = the permanent rejection rationale (FEATURES/KNOWN_ISSUES pointer), §2 = the published
"instead of eval" doc section, §3 = a V1 BUILD ITEM (not frozen).

## 5. RULED (dev, 2026-07-23)

- **P1 → accepted**: full-eval rejection + substitutes doc locked.
- **P2 → Core.Sandbox v1 BUILDS** (dev overrode the freeze recommendation after the
  tradeoff re-ask): pure-expression scope, tree-walker-only, `E-TRANSPILE-SANDBOX`.
