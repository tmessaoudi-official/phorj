# Track V — Competitive analysis (cross-cutting)

This track is deliberately **not** a feature list — the feature-level competitive borrows (refinement
types, match guards, let-else/guard-let, Gleam-style `use`, source-generation, opaque newtypes) are
already owned by the PHP-parity-and-beyond spec and tracks A/B/C, and the tooling-as-adoption-lever items
(formatter, WASM playground, LSP, doctests) are owned by track F. Track V's distinct contribution is the
**strategic / positioning layer** that mines *adoption outcomes* — what Hack, TypeScript, Kotlin, Swift,
Rust, Go, Gleam, Roc and Elixir got *right or wrong* as products, and the specific decisions that map onto
Phorge — plus a small set of genuinely competitive *features* the parity pass missed. The single most
important lesson is Hack's: a typed PHP that **required wholesale adoption died**, while TypeScript — which
let you adopt one file at a time — won; Phorge's entire identity (transpile-to-PHP bridge, byte-identical
spine, PHP→Phorge importer) is structurally the TypeScript answer, so the gaps here are about *protecting
and sharpening that incremental-adoption position*, a stability/editions story so the language can evolve
without an HHVM-4.0-style breaking rug-pull, a typed standard library as a coherent product (Hack's HSL was
its best-loved asset), and a differentiation imperative now that PHP 8.x itself has caught up on types and
JIT (the exact erosion that killed Hack's external value prop). A few of these are `reject`/`defer` on
purpose — async/await and full effect-platforms fail the "earn your surprise budget" and determinism tests.

## Gap table

| id | title | kind | fit | rec | milestone | effort |
|---|---|---|---|---|---|---|
| V-incremental-adoption | Incremental/file-at-a-time adoption story (the TypeScript-beat-Hack lesson) made explicit | new | strong | adopt | M8 | M |
| V-editions-stability | Language editions + stability policy (Rust editions / Go compat promise) — avoid the HHVM-4.0 rug-pull | new | strong | adopt | M12/GA | L |
| V-typed-stdlib-product | A coherent *typed standard library as a product* (the Hack HSL lesson), not scattered `Core.*` natives | new | strong | adopt | M11 | L |
| V-differentiation-vs-php8 | Differentiation thesis vs modern PHP 8.x (the "PHP caught up, Hack lost its value prop" lesson) | new | strong | adopt | GA | S |
| V-equality-refinement | Equality/`==`/`!=` type refinement narrowing (Hack refinements / TS discriminated narrowing) | new | strong | adopt | M-RT | M |
| V-discriminated-unions | Tagged/discriminated unions with a literal-tag field (TS/Swift) over the existing enum+union surface | map | ok | defer | M-RT/post | M |
| V-shapes-vs-records | Structural record/"shape" types (Hack `shape`, TS object types) vs Phorge's nominal classes | new | weak | reject | — | L |
| V-result-error-model | `Result<T,E>` + `?`-style propagation as the *primary* error model (Rust/Gleam/Swift), exceptions secondary | new | strong | adopt | M3 (error slice 2) | L |
| V-gleam-error-quality | Gleam-grade compiler-error quality as a *measured commitment*, not best-effort | new | strong | adopt | M12 | M |
| V-go-onboarding-tooling | Go-style "tooling is the adoption lever" bundle: one binary does fmt+test+run+build (already mostly true — make it a stated principle) | map | strong | adopt | M12 | S |
| V-kotlin-interop-posture | Kotlin/Java-style "100% interop with the host, gradually migrate" posture for the PHP↔Phorge boundary | new | ok | adopt | M8 | M |
| V-swift-guard-ergonomics | Swift `guard`/early-return ergonomics + labeled break/continue (competitive readability win) | new | ok | defer | M3 | M |
| V-elixir-pipe-into-everything | Elixir/F#-style pipe into *any* arg position (`x |> f(_, y)`) and pipe-first stdlib design | new | ok | defer | M-RT/post | M |
| V-roc-platform-effects | Roc-style platform/effect separation (pure core + effectful platform) | new | weak | reject | — | L |
| V-async-await | async/await coroutines (Hack/JS/Swift) | new | weak | reject | — | L |
| V-naming-branding | Resolve the "Phorge" name collision (Phabricator fork) as a competitive/adoption blocker | new | strong | defer | pre-GA | S |
| V-corpus-driven-priorities | Real-corpus-driven feature prioritisation (the "instrument adoption like the big langs do" discipline) | new | ok | defer | post-GA | M |

## Rationale per ADOPT item

**V-incremental-adoption** — [Verified, web: Hack "required wholesale adoption to realize benefits… few
developers learned the esoteric fork"; TypeScript's `allowJs`/per-file `// @ts-check` is the canonical
counterexample]. This is the *thesis of the whole language* and it is currently implicit. Phorge already
has the machinery (Phorge→PHP transpile, the planned PHP→Phorge importer at M8, byte-identical spine), but
nowhere is the **mixed-codebase adoption path** specified as a first-class capability: can a team transpile
one Phorge module into an existing PHP repo, call it from hand-written PHP, and migrate file-by-file? The
gap is to make that an explicit, tested, documented workflow (a `phg transpile` output that drops cleanly
into a PSR-4 tree + a guide), because *this is the exact axis on which TS beat Hack*. Strong philosophy fit:
the bridge "lowers the on-ramp" is already in VISION.md — this operationalises it. Effort M (mostly an
integration test + guide + ensuring transpiled output is consumable from real PHP, which M7's oracle
already half-proves).

**V-editions-stability** — [Inferred: Rust editions and Go's compatibility promise are the two languages
that evolved aggressively *without* an adoption-killing break; HHVM 4.0 dropping PHP support is the
cautionary tale, Verified above]. Phorge is pre-1.0 and breaking changes are free now, but a credible 1.0
needs a stated **stability contract** (what may break, what may not, how) and ideally an editions mechanism
so future surprise-removing changes (e.g. tightening a coercion) ship behind an opt-in flag rather than
breaking every program. This is the structural defence against Phorge's own future HHVM-4.0 moment. Fits
"earn complexity / Rule of Three" — don't build the edition machinery now, but *commit to the policy* at GA
and design the version header so an edition pragma can be added without a format break. Effort L because a
real editions implementation is large; the *policy + version-header headroom* is the GA-blocking sliver.

**V-typed-stdlib-product** — [Verified: Slack's "Hacklang at Slack" and hacklang.org both cite the
"well-typed standard library" (HSL) as a core, beloved asset; it is the thing migrators mention most].
Phorge's stdlib is currently a *scattered set* of `Core.*` natives (`Console`/`Math`/`Text`/`File`/`Bytes`/
`Html`/`Map`/`List`/`Set`) added feature-by-feature. The competitive lesson is that a typed stdlib only
becomes a *selling point* when it is **coherent, discoverable, and consistently typed** — namespaced,
documented as one surface, with uniform conventions (every collection op generic, every fallible op
returning `T?`/`Result`, consistent argument order — Phorge already hit the array_map/reduce arg-order trap
that motivates this). The gap is a stdlib *design charter* + a generated API reference, owned at M11 when
breadth lands, so the stdlib reads as a designed library not an accretion. Track L owns the breadth; V owns
the *product framing* (this is the HSL-was-the-killer-feature angle). Effort L (breadth is already on the
roadmap; the charter + reference generation is the added scope).

**V-differentiation-vs-php8** — [Verified, web: "PHP caught up by adding a JIT compiler and improved
performance, reducing the need for a custom runtime" — the single biggest cause of Hack's external
irrelevance]. Modern PHP 8.x has typed properties, enums, readonly, first-class callables, `match`, named
args, fibers, and a JIT. Hack's "we're faster + typed" pitch evaporated. Phorge's differentiation is *not*
speed (it transpiles to PHP) — it is **provable correctness** (byte-identical spine, no-panic guarantee,
immutable-by-default, compile-time null safety, erased generics that PHP can't express, the
whole-project-validation superpower). The gap is a crisp, honest **differentiation statement** that says
explicitly "here is what Phorge gives you that PHP 8.4 still does not" — and a discipline that every new
feature is interrogated against "does PHP already have this well enough?" (if PHP's form is sound, prefer
mapping over a novel surface, per philosophy tenet 2). Effort S — this is a positioning artifact
(a FEATURES/VISION section + a per-feature checklist), not code, but it is GA-blocking because it is the
answer to "why not just use PHP 8.4?".

**V-equality-refinement** — [Verified, web: "Equality refinements allow Hack to narrow inferred types using
the result of === and !== comparisons… fewer duplicative checks"; TypeScript's discriminated-union narrowing
is the same idea and is one of TS's most-loved features]. Phorge already has smart-cast narrowing via
`instanceof` (S1) and if-let null narrowing (S2), but **not** narrowing on equality: after `if (x == 0)` or
`if (tag == "circle")`, `x`/`tag` is not refined. This is the natural completion of the narrowing story and
a direct competitive borrow from the two languages Phorge most resembles. It lowers to existing branch ops
(no new `Op`), pairs naturally with the already-adopted match-guard cluster, and is exactly the kind of
surprise-removing, legibility-improving feature the philosophy favours. Strong fit. Effort M (a checker
flow-narrowing pass on `==`/`!=` against literals and enum tags; front-end only, spine-safe). Slot into
M-RT alongside the pattern cluster.

**V-result-error-model** — [Inferred: Rust `Result` + `?`, Gleam `Result` + `use`, Swift `throws`/`try?` all
chose an *explicit, value-level* error model; Phorge's "no silent failure / clean fault" philosophy and its
no-exceptions-yet posture point the same way]. Phorge's error-handling slice 2 is "catchable model: try/catch
vs Result" — *undecided*. The competitive evidence strongly favours making **`Result<T,E>` the primary,
idiomatic model** (with `?`-propagation, which the parity spec's Gleam-`use`/do-notation adoption already
anticipates) and exceptions a secondary/interop concession, because Result is what makes errors *visible in
the type* — the legibility apex. It transpiles cleanly (Result is an enum → PHP class hierarchy; `?` is a
front-end CPS lowering). This is a *direction-setting* recommendation for the open slice-2 design, not a new
feature request. Strong fit. Effort L (it's a whole error-model slice), milestone = M3 error slice 2.

**V-gleam-error-quality** — [Inferred: Gleam and Elm built their reputations substantially on
*friendly, actionable* compiler errors; it is a documented, deliberate competitive moat, not an accident].
Phorge already has sharp diagnostics (caret spans, did-you-mean, stable codes, `phg explain`) — strong
foundation. The gap is to make error-message quality a **measured, maintained commitment**: a corpus of
"common mistake → expected message" golden tests, a style guide for new diagnostics, and a bar that every
new `E-*` code clears (cause + fix + example). This is cheap relative to its adoption payoff and is the
single most-cited reason developers *enjoy* a typed language enough to stay. Strong fit (legibility is a
craftsmanship dimension). Effort M. M12/GA.

**V-go-onboarding-tooling** — [Verified: `phg` already does run/runvm/check/transpile/build/bench/disasm/
serve/vendor + per-command help from one binary]. Go's adoption was massively aided by "one tool, zero
config, gofmt ends all formatting debates." Phorge is *already* structurally Go-like here; the gap is to
(a) state this as an explicit design principle (one binary, one canonical format via the track-F `phg fmt`,
no config files to start) and (b) ensure the canonical formatter is *mandatory-by-convention* like gofmt
(no style options), which ends bikeshedding before the community exists. Mostly a `map` of existing reality
into a stated principle + the `phg fmt` opinion (no options). Strong fit, effort S. M12.

**V-kotlin-interop-posture** — [Inferred: Kotlin won on the JVM precisely by being "100% Java-interoperable,
migrate at your own pace"; it never asked you to abandon Java]. This is the *receiving* side of
V-incremental-adoption: the posture that the PHP↔Phorge boundary is a **first-class, supported, bidirectional
seam** — Phorge code is callable from PHP and (via the M8 importer) PHP is mechanically liftable into Phorge,
with the explicit promise "you never have to rewrite, only gradually migrate." The gap vs the bare M8
importer is the *guarantee and ergonomics* of the boundary (calling conventions, how a transpiled Phorge
class presents to PHP callers, namespace mapping). Adopt as the framing for M8. Effort M. Fits VISION's
"migration bridge both ways" exactly.

## Notes on the DEFER / REJECT items

- **V-discriminated-unions (defer)** — TS/Swift tagged unions are largely *already expressible* via Phorge
  enums-with-payloads + `match`; the only missing nicety is a literal *tag field* on a union of classes.
  Maps onto the existing surface; revisit after equality-refinement lands (which gives the narrowing such
  unions need).
- **V-shapes-vs-records (reject)** — Hack `shape`/TS structural object types clash with Phorge's *nominal*
  class identity and the byte-identical-PHP spine (PHP has no structural types). The parity spec already
  prefers nominal classes; opaque newtypes cover the lightweight-type need. Reject to protect coherence.
- **V-swift-guard-ergonomics (defer)** — `guard let … else { return }` is mostly the parity spec's already-
  adopted let-else; labeled break/continue is a small competitive add but belongs with the mutation/loop
  surface, not its own push. Defer to M3.
- **V-elixir-pipe-into-everything (defer)** — Phorge's `|>` is first-arg only; piping into an arbitrary
  position (`x |> f(_, y)`) is an Elixir/F# nicety but adds parser surprise; defer until the stdlib is
  pipe-first by design so the payoff is real.
- **V-roc-platform-effects (reject)** & **V-async-await (reject)** — both fail the determinism/earn-the-
  surprise tests. Roc's platform/effect split is research-grade and alien to PHP devs; async/await forces a
  coloured-function world that contradicts Phorge's chosen *uncoloured* `spawn`+channels concurrency (M6)
  and its single-threaded Rc-heap reality. Reject (M6 green-threads is the sound answer to the same need).
- **V-naming-branding (defer)** — the "Phorge" collision with the active Phabricator fork is a real
  discoverability/adoption blocker; decision already taken to defer the rename to pre-GA (keep `phg`/`.phg`).
  Listed here for completeness as a competitive concern.
- **V-corpus-driven-priorities (defer)** — TS/Rust/Go instrument real-world usage to prioritise; Phorge has
  no corpus yet. A post-GA discipline once there is adoption to measure.

## Critic pass

**Mis-listings found: 0.** Re-checked every ADOPT item against FEATURES.md / ROADMAP.md / MILESTONES.md /
the CLAUDE.md milestone log. None is already shipped: `phg fmt`/LSP are M7 (🔲), no editions/stability
policy exists, no differentiation-vs-PHP8 statement exists, equality narrowing is genuinely absent (only
`instanceof` S1 + if-let S2 narrow today — verified in FEATURES.md and the M-RT log), the Result error
model is the undecided "error slice 2", and the M8 PHP→Phorge importer is unbuilt. The first researcher
correctly avoided re-listing feature-level borrows owned by the parity spec / tracks A–C / F.

**Sanity-check against philosophy:** the ADOPT set is sound. The two REJECTs (Roc effect-platform,
coloured async/await) correctly fail "earn the surprise budget" + determinism. One nuance: `V-result-error-model`
is graded `strong` fit but should be read as *direction-setting for the open slice, not a mandate* — making
Result *primary* while keeping `try/catch` as the PHP-interop bridge is the philosophy-correct framing (the
parity spec line 123 already says "Result-first recommended, try/catch as PHP-interop bridge"), so it is
consistent. Kept as adopt.

**Newly-found gaps (competitive/strategy long tail the first pass missed):**

| id | title | kind | fit | rec | milestone | effort |
|---|---|---|---|---|---|---|
| V-killer-app-domain | A named flagship domain / "killer use case" (Elixir↔Phoenix, Rust↔CLI/systems, TS↔frontend) | new | strong | adopt | GA | M |
| V-onboarding-first-hour | "First hour" onboarding artifact: install→run→transpile→deploy-to-PHP in <10 min (Go tour / rustlings model) | new | strong | adopt | M12/GA | M |
| V-perf-honesty-vs-php | Honest perf-positioning vs PHP (transpiled output ≈ PHP; do NOT claim a speed win — the Hack trap inverted) | new | strong | adopt | GA | S |
| V-semver-deprecation-policy | Public SemVer + deprecation-window policy for the language surface AND `Core.*` stdlib API | new | strong | adopt | GA | S |
| V-llm-codegen-affinity | Optimise for LLM-assisted authoring (explicit, low-ambiguity, greppable surface) as a 2026 adoption lever | new | ok | defer | post-GA | M |
| V-community-governance | Lightweight governance/RFC process + decision-log discipline (the bus-factor / single-dev risk) | new | ok | defer | post-GA | S |
| V-benchmark-game-presence | A public, reproducible benchmark/showcase corpus (perf + correctness + transpile fidelity) for credibility | new | ok | defer | post-GA | M |
| V-error-suppression-stance | Explicit competitive stance: NO error-suppression/escape-hatch culture (`@`, `mixed`-everywhere, `any`) — the anti-TS-`any` lesson | new | strong | adopt | GA | S |

Rationale for the new ADOPT items:

- **V-killer-app-domain** — [Inferred: every language that crossed the chasm had a flagship domain — Elixir
  rode Phoenix, Rust rode CLI+systems, TS rode SPA frontends, Go rode cloud infra]. Phorge currently pitches
  "a better PHP" generically. The competitive lesson is that *generic* "better X" languages stall; a named
  beachhead domain (the obvious candidate: **typed web backends / HTTP services**, given M6 `phg serve` +
  `Core.Html` XSS-safety + whole-project validation) gives the language a story, an example corpus, and a
  community nucleus. This is a positioning artifact (VISION/README section + a flagship example app), not
  code. Strong fit — it operationalises "approachable, get-things-done" into a concrete promise. GA-relevant.

- **V-onboarding-first-hour** — [Inferred: Go's tour and Rust's rustlings/"the book" are repeatedly cited as
  adoption accelerants; a sub-10-minute success path is the single strongest retention lever]. Phorge has
  per-command help and examples, but no *guided* install→first-program→transpile→run-under-PHP path that a
  newcomer follows in one sitting. The "deploy your first Phorge module into a real PHP file in 10 minutes"
  walkthrough is the concrete embodiment of the incremental-adoption thesis (V-incremental-adoption is the
  *capability*; this is the *learnability*). Cheap, high-leverage, GA-adjacent.

- **V-perf-honesty-vs-php** — [Verified, web/own knowledge: Hack's "we're faster" pitch evaporated when PHP
  added JIT — the cautionary tale V-differentiation already cites]. The inverse risk for Phorge is *claiming*
  a speed win it cannot honestly make: transpiled Phorge runs *as PHP*, so it is at best PHP-speed (often a
  hair slower via helper shims). The competitive discipline is to **never sell speed** and to state plainly
  that the value is correctness + legibility, not throughput — pre-empting the "but PHP 8.4 JIT is faster"
  rebuttal. A one-paragraph honesty clause in the differentiation statement; distinct from
  V-differentiation-vs-php8 (that says *what we add*; this says *what we explicitly do not claim*). Strong fit
  with "no surprises" + craftsmanship honesty. Effort S.

- **V-semver-deprecation-policy** — [Inferred: every credible 1.0 (Rust, Go, TS) ships a stated SemVer +
  deprecation contract; the `Core.*` stdlib is an *API surface* that will need to evolve]. V-editions-stability
  covers the *language* compatibility contract; this is the narrower, cheaper, also-missing piece: a stated
  SemVer policy and a deprecation *window* (the `W-DEPRECATED` lint already exists per the parity spec — wire
  it to a policy) covering both the language surface and the stdlib API. Pairs with V-typed-stdlib-product
  (a "product" needs an API-stability promise). Effort S — policy text + leveraging the existing warning
  channel.

- **V-error-suppression-stance** — [Verified, own knowledge: TS's `any` and PHP's `@`/`mixed` are the
  most-cited *regret* features — escape hatches that erode the type system from within]. Phorge already
  rejects `@` and has no `any` (the parity spec rejects coercion footguns), but the *competitive stance* is
  unstated: a deliberate "Phorge has no type-system escape hatch — `mixed` exists only as the erasure target,
  never as an authored opt-out" clause. This is a differentiation *and* a precommitment that protects the
  spine for life. It is the positive framing of why generics are *erased* not *dynamic*. Strong fit. Effort S
  (positioning clause + a check that no authored-`mixed`/`any` surface leaks in).

Rationale for the new DEFER items:

- **V-llm-codegen-affinity** — [Speculative, but timely]: in 2026 a large share of new-language code is
  LLM-assisted; languages with explicit, low-ambiguity, greppable surfaces (Phorge already qualifies —
  mandatory packages, no inference surprises, stable diagnostic codes) get *generated* more correctly. Worth
  stating as a design value and measuring post-GA, but not GA-blocking and partly already-true.
- **V-community-governance** — [Inferred]: single-developer is the dominant *adoption* risk (bus factor); a
  lightweight RFC/decision-log process (the project already keeps Decisions Logs in plans) signals
  sustainability. Post-GA — premature before a community exists.
- **V-benchmark-game-presence** — [Inferred]: a public reproducible benchmark/showcase corpus (the
  Benchmarks-Game model) builds credibility, but Phorge has no audience yet and the `phg bench --vs-php`
  machinery already exists internally; promote it to a public artifact post-GA.
