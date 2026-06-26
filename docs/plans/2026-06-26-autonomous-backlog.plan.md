# Autonomous Backlog Plan (post-2026-06-26 compact)

> Ordered queue for autonomous progression. Each item ships independently **green + committable**
> (`PHORGE_PHP=…/php-8.5.7 PHORGE_REQUIRE_PHP=1 cargo test --workspace` + clippy + fmt) and follows the
> standing rules: an example + `examples/README.md` entry per feature, build the release binary after
> each, **commit green slices but NEVER `git push`** without an explicit request, genuine design forks
> still pause via `AskUserQuestion` (autonomy suppresses *confirmation* gates, never *information* gates).

## Decisions Log
- [2026-06-26] Developer pinned **`Core.Json`** as the next big chunk and authorized **autonomous
  progression** through this backlog (commit green slices; never push; pause only on genuine design
  forks). Spec-first for Core.Json (breaking/meaty design); the rest are direct slices.
- [2026-06-26] **Core.Json number model = `Int(int) + Float(float)`** (PHP-faithful; developer
  confirmed my recommendation over `Num(float)`). Mirrors PHP `json_decode` (int for `"42"`, float
  for `"42.0"`) + Phorge's own int/float split; least-surprising for a PHP dev; byte-identical either
  way (`json_encode(42.0)`→`"42"`).
- [2026-06-26] **Core.Json ships both `stringify` (compact) AND `stringifyPretty` (4-space,
  `JSON_PRETTY_PRINT`-matching)** in the first slice (developer chose "Add stringifyPretty too").
- [2026-06-26] **PHP-reserved enum-variant names are mangled in the transpiler** (append `_`:
  `Int`→`Int_`, `Bool`→`Bool_`, `Null`→`Null_`, `Float`→`Float_`) so the Json API stays clean
  (`Json.Int/Bool/Null/Float/Str/Arr/Obj`). PHP reserves int/float/bool/null as class names even
  inside a namespace (verified vs 8.5). Transpiler-only (run/runvm use the Phorge variant string →
  stdout byte-identity untouched); reusable for ANY enum. Developer chose this over a J-prefix API.
- [2026-06-26] Autonomy = **FULL AUTO**. Persistent per-project bypass SET at
  `~/.claude/projects/-stack-projects-phorge/state/autonomous-3c-bypass` (never expires; remove
  manually to stop — statusline shows `⚠⚠ AUTO-3C(proj)`). Post-compact sessions run this backlog
  hands-off. Hard stops that still apply: `git push` (never autonomous), risky/destructive actions,
  and genuine design forks (→ `AskUserQuestion`).

## Backlog (in order)

### 1. `Core.Json` — JSON parse/stringify (PRIMARY, spec-first)
- API: `Core.Json.parse(string) -> Json?` (None on malformed) + `Core.Json.stringify(Json) -> string`.
- Value model: a concrete recursive enum — `enum Json { Null(), Bool(bool), Num(float), Str(string),
  Arr(List<Json>), Obj(Map<string, Json>) }` (expressible today: generic enums + `Map` + `List` all
  shipped; **no new type-system feature needed** — this is what unblocked it).
- **Design risk (the spec's job): byte-identity with PHP `json_encode`/`json_decode`** — number
  formatting (reuse `__phorge_float`; integers-as-floats?), key ordering (insertion-ordered `Map` ✓),
  string escaping (`/`, unicode, control chars), compact vs `JSON_PRETTY_PRINT`. Std-only deterministic
  recursive-descent parser. Verify no `Core.Json` native exists yet; check the `native::registry` shape.
- Likely a higher-order/`Reflective`-free pure native pair returning the `Json` enum; transpile to
  `json_decode($s, false)` mapped into the enum / `json_encode`. Spec the enum↔PHP-array bridge carefully.
- Example: `examples/guide/json.phg` (round-trip parse→stringify) + a web JSON handler (see item 2).

### 2. Web + JSON demo
- `examples/web/` handler that parses a JSON request body and returns a JSON response — pairs Core.Json
  with the shipped M6 `phg serve`/`Request`/`Response`. Byte-identity-gated. Small, high-showcase.

### 3. `docs/MILESTONES.md` M6 staleness fix (tiny)
- The M6 section (~line 245) lists W2–W4 as "remaining", but `src/serve.rs`, `phg serve`,
  `web/router.phg`, `web/server.phg` all ship. Correct it to reflect reality + Core.Json's landing.

### 4. F-m — general PHP-reserved-word guard
- Extend `is_php_reserved_symbol_name` (today: just `var`) to the full PHP-reserved set
  (`list`/`print`/`clone`/`array`/`unset`/`empty`/`echo`/`eval`/`isset`/…) as **symbol** names —
  **per-word empirical PHP-8.5 verification** (some are semi-reserved = legal as method names; don't
  over-reject). Turns latent oracle-failures into clean `E-RESERVED-NAME`. See KNOWN_ISSUES.

### 5. (stretch) `parseInt` / `sort` stdlib natives (M4/M11)
- The deferred optional-return / ordering natives — `Core.Text.parseInt(string) -> int?`,
  `Core.List.sort` (comparator via the higher-order-native path). Each byte-identity-gated.

## Status
- [x] **1 Core.Json** — DONE (spec `docs/specs/2026-06-26-core-json-design.md`; Slice A reserved-variant
  mangling `305e331`; Slice B natives + injection + PHP helpers + `examples/guide/json.phg`). 917 lib +
  109 differential (PHP-8.5 oracle) green, clippy + fmt clean.
- [x] **2 web+JSON demo** — DONE (`examples/web/json-api.phg`: a `handle(Request) -> Response` JSON
  endpoint pairing Core.Json + Core.Bytes + the M6 value model; byte-identical run/runvm/real PHP).
- [x] **3 MILESTONES fix** — DONE (M6 section now reflects W0–W4 shipped + Core.Json layered on; deferrals listed).
- [ ] 4 F-m   [ ] 5 parseInt/sort
