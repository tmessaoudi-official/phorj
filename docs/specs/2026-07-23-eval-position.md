# SPEC — `eval`: position paper (DEC-331 D10c, the "spec tomorrow" hold)

> Status: **POSITION SPEC, awaiting dev ruling.** D10c ruling being elaborated: full `eval`
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

**Gate (per the D10c ruling): this ships ONLY against a concrete use case.** The register
records none today. Candidates worth waiting for: user formula fields (pricing/report tools),
safe config expressions. Until one is real, `Core.Sandbox` stays a frozen design.

## 4. Recommendation

Lock §1 as the permanent rejection rationale (FEATURES/KNOWN_ISSUES pointer), publish §2 as
the "instead of eval" doc section, keep §3 frozen-not-queued. Revisit only on a named use
case (ADJUDICATION as always).

## 5. PENDING for dev

- **P1**: accept the recommendation (reject + substitutes doc + frozen sandbox design)?
- **P2**: if `Core.Sandbox` is ever activated — v1 scope confirm: pure expressions only,
  tree-walker-only, `E-TRANSPILE-SANDBOX` (all three recommended above).
