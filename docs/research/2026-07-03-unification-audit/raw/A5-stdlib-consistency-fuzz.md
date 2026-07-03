# A5 — Stdlib API Consistency + Fuzz Audit (2026-07-03)

Auditor: A5 (stdlib/native-function consistency, naming, inconsistent features, fuzz/DX).
State audited: HEAD `0691228`, clean tree. Binary: `target/release/phg` 0.5.1-alpha.1.
PHP oracle used for parity legs: `/stack/tools/phpbrew/php/php-8.5.7/bin/php -n`.

Method: full registry extraction from `src/native/*.rs` (module/name/params/ret of every
`NativeFn`, including the `entry()` helper-registered ones in `hash.rs` and `validate.rs`),
plus **~44 distinct throwaway `.phg` probe programs actually executed** (52 executions total,
counting 7 transpile+PHP legs). Prior research (`docs/research/full-audit/raw/E-phorj-surface.md`,
`D-php-surface.md`, `F-cross-language.md`) used as leads only; every claim below re-verified
against current `src/`.

Evidence grades: **[Verified-run]** = executed and observed; **[Verified-read]** = read the
current registry/impl source; [Inferred] where noted.

---

## 0. Census

**236 natives across 28 `Core.*` modules** [Verified-read: extracted from `src/native/*.rs`
registry literals + `entry()` helpers; counts cross-checked against `grep -c 'module: "'`]:

| Module | # | Module | # | Module | # | Module | # |
|---|---|---|---|---|---|---|---|
| Bytes | 6 | File | 8 | Math | 31 | Runtime | 4 |
| Conversion | 20 | Hash | 8 | Output | 2 | Set | 11 |
| Cryptography | 2 | Html | 8 | Path | 5 | String | 31 |
| Csv | 2 | Ini | 1 | Process | 1 | Test | 8 |
| Decimal | 3 | Json | 5 | Random | 6 | Time | 3 |
| Encoding | 4 | List | 30 | Reflection | 7 | Url | 4 |
| Environment | 2 | Map | 12 | Regex | 7 | Validation | 5 |

Note for other auditors: naive `grep 'name:'` extraction MISSES 9 natives — `Core.Hash`
crc32/md5/sha1/sha256 and all 5 of `Core.Validation` (isInt/isNumber/isAlpha/isAlnum/isHex)
are registered through local `entry()` closures (`hash.rs:551`, `validate.rs:92`). The oft-quoted
"~227" count is this undercount.

---

## 1. P0 — Byte-identity (Invariant 1) violations found by fuzzing

These are not naming nits; they are `phg run` ≠ transpiled-PHP divergences, confirmed by
running BOTH legs on the same program. None is disclosed anywhere (grep of KNOWN_ISSUES.md,
INVARIANTS.md found no mention). The differential corpus never feeds non-ASCII whitespace /
multibyte strings / empty separators into these natives, which is how all three slid through.

### 1.1 `String.trim` / `trimStart` / `trimEnd` — Unicode-vs-ASCII whitespace set
- Rust side: `s.trim()` (`src/native/text.rs:25`) — strips **all Unicode whitespace** incl. U+00A0.
- PHP side: emits bare `trim({})` / `ltrim` / `rtrim` (`text.rs:459,405,414`) — strips only `" \t\n\r\0\x0B"`.
- **[Verified-run]** probe `"\u{00A0}x\u{00A0}"` → `phg run`: `5 1` (length before/after);
  transpiled PHP: `5 5`. Divergent output, same program.
- This is precisely the "byte-id trap: match PHP `trim()` set exactly" lesson already learned in
  `Core.Ini` (`ini.rs` hand-rolls the PHP set) — `Core.String` never got the same fix.
- Fix direction: restrict the Rust impl to the PHP charset (interpreter is the oracle, but here the
  *intended* semantic per the PHP-familiarity philosophy is PHP's set — developer call; either leg
  can be made to match the other, but they must match).

### 1.2 `String.reverse` — char-wise vs byte-wise reversal
- Rust: `s.chars().rev().collect()` (`text.rs:40`). PHP: `strrev({})` (`text.rs:477`) — byte reversal.
- **[Verified-run]** `String.reverse("noël")` → phg: `lëon` (valid UTF-8); PHP: `l??on`
  (mangled bytes, ë's two bytes reversed). Same byte length, different bytes.
- Any multibyte string diverges. Needs either a char-wise PHP helper or a byte-wise Rust impl.

### 1.3 `String.split` / `splitOnce` with empty separator — total vs fatal
- Rust: `s.split("")` is total → `["", "a", "b", ""]` for `"ab"`. PHP: `explode('', …)` throws
  `ValueError: Argument #1 ($separator) must not be empty` (PHP 8).
- **[Verified-run]** `String.split("ab", "")` → phg: prints `4` (exit 0); PHP: Fatal error, exit 255.
  Invariant 1 requires identical **failure behaviour** too — this violates it in both directions
  (phg succeeds with a surprising `["","a","b",""]`; PHP dies).
- `splitOnce` emits `explode({sep}, {s}, 2)` (`text.rs:535`) — same trap.
- Cleanest fix consistent with the existing style: fault on empty separator on BOTH legs (there is
  precedent — `String.count` already faults on empty needle, see §4).

### 1.4 (minor, forward-compat) `Math.pow(0.0, negative)` — PHP deprecation diagnostic
- **[Verified-run]** stdout identical (`NaN inf` both legs — the `__phorj_float` helper normalizes
  NAN/INF casing correctly), but PHP 8.5 prints `Deprecated: Power of base 0 and negative exponent…`
  on stderr. Today: benign (stdout-parity holds). When PHP promotes it to an error, leg dies.
  Worth a KNOWN_ISSUES line.

Consistent-under-fuzz (checked the same way, NO divergence): `String.length` (byte count, 7 for
"straße", = strlen), `uppercase`/`lowercase`/`capitalize` (ASCII-only both legs; `STRAßE`, `émile`
unchanged), `padLeft` (byte-width both), `substring` (byte/`substr` semantics both), `lines`
(`\n`-split both, `\r` retained), `indexOf("", …)` → 0 both, `Math.round` half-away-from-zero,
`Math.numberFormat(x, -2)` → decimals clamped to 0 both legs. [Verified-run]

---

## 2. Cross-module naming inconsistencies (the "same concept, different name" table)

All **[Verified-read]** against the extracted registry; leads from the prior audit re-confirmed
still present at HEAD.

| # | Concept | Current names | Inconsistency |
|---|---------|--------------|----------------|
| 2.1 | find-index-of-value | `String.indexOf(s,s)->int?`, `List.indexOf(l,t)->int?`, **`Bytes.find(b,b)->int?`** | `Bytes.find` is the exact same operation/return convention as the two `indexOf`s. Lead confirmed → rename `Bytes.find→indexOf` (the already-approved P2). |
| 2.2 | "find" itself, 3 meanings | `Bytes.find`→index?, `List.find(l,pred)`→element?, `Regex.find(re,s)`→matched substring? | One verb, three different return conventions across modules. After 2.1, `List.find`(predicate→element) vs `Regex.find`(pattern→text) remains a tolerable split, but should be documented deliberately. |
| 2.3 | membership test | `List.contains`, `Set.contains`, `String.contains`, **`Map.has`** | `has` is the lone outlier. Lead confirmed → `Map.has→containsKey` (approved P2). |
| 2.4 | cardinality | `List.length`, `String.length`, `Bytes.length` vs `Map.size`, `Set.size` | Two names for "how many". Either is defensible; the split is not. |
| 2.5 | `count`, 2 semantics | `List.count(l, pred)->int` (predicate matches) vs `String.count(s, needle)->int` (substr_count) | Same name, different kind of argument and meaning. If `Bytes.find` is worth renaming, this pair is the same class of trap. |
| 2.6 | sub-sequence extraction | `List.slice(l, offset, LENGTH)` — PHP array_slice, **negative offset AND length OK** [Verified-run: `slice([1..5],-2,2)`→`[4,5]`]; `String.substring(s, start, LENGTH)` — PHP substr, negative start OK [Verified-run: `("hello",-3,2)`→`"ll"`]; `Bytes.slice(b, start, END-EXCLUSIVE)` — clamped half-open, **negatives clamp to 0** [Verified-run: `(-1,2)`→`"he"`] | THREE conventions: two different second-arg meanings (length vs end), two different negative-index behaviours, and two names (`slice`/`substring`) for the same concept. The approved "unify slice (length+neg)" P2 covers this; the Bytes end-exclusive form is the odd one out. |
| 2.7 | add-one-element | `Set.add`, `List.append`, `Map.set` | Three verbs. `Map.set` is keyed so it earns its name; `add` vs `append` is a coin-flip pair. Minor. |
| 2.8 | serialize-to-string | `Json.stringify`/`stringifyPretty`/`stringifyLines`, `Csv.format`, `Html.render` | Three verbs for "value → string". `Html.render` is arguably a distinct concept (finalize safe HTML); `Csv.format` vs `Json.stringify` is not. `parse` is uniform everywhere (good). |
| 2.9 | ranged random | `Random.nextInt()`, `Random.nextFloat()` (no args) vs `Random.intBetween(min,max)` vs `Random.secureInt(min,max)` | The two `(int,int)->int` ranged siblings are named on different patterns (`intBetween` vs `secureInt`); a `secureIntBetween`/`intBetween` or `nextIntBetween` pairing would be self-consistent. |
| 2.10 | unit suffixes | `Time.nowMilliseconds` vs `Runtime.monotonicNanos` | Spelled-out vs abbreviated unit in the same surface (`memoryBytes`/`peakMemoryBytes` are fine). |
| 2.11 | Hash output types | digests (`crc32/md5/sha1/sha256`) → hex `string`, **`hmac` → hex `string`**, but `hkdf`/`pbkdf2` → `bytes` | Within one module, MAC returns hex text while KDFs return raw bytes; composing hmac output into hkdf/equals requires `Encoding.hexDecode`. `Hash.equals(bytes,bytes)` can't take `hmac`'s own output directly — a real API trap. [Verified-read: hash.rs:539-597] |
| 2.12 | `File.copy -> int` | siblings `write/append/delete/rename -> void` | Lone Int return (bytes copied, Rust `fs::copy` leak-through). Nothing else in Core.File reports a size on success. |
| 2.13 | crypto surface split | `Core.Cryptography` (hashPassword/verifyPassword), `Core.Hash` (digests/MAC/KDF), `Core.Random` (secureBytes/secureInt) | Three homes for cryptography. Defensible individually; as a whole the discoverability is poor (a user looking for PBKDF2 will try Cryptography first). |

### 2.14 `Core.Conversion` — three naming families in one module
[Verified-read: convert.rs registry, 20 natives]
- Family A `xToY` (typed, total or Optional): `boolToInt`, `boolToFloat`, `boolToDecimal`,
  `intToBool`, `intToDecimal`, `floatToBool`, `floatToDecimal`, `decimalTo{Bool,Float,Int,IntExact}`, `floatToIntExact`.
- Family B bare `toX`: `toFloat(int)`, `toInt(float)`, `toString(T)` — these are just
  `intToFloat`/`floatToInt` under unpredictable names; the user must guess which pairs got the
  short name. `toInt(float)` truncates (returns `int?`) while its exact sibling is `floatToIntExact` —
  but the decimal pair spells BOTH long (`decimalToInt`/`decimalToIntExact`). Same relationship,
  two naming shapes.
- Family C `asX(T)->X?`: `asBool/asFloat/asInt` — these are **runtime type assertions on unions**,
  not conversions at all (convert.rs:126 comment says so) — semantically alien to the module name.
- Also: `Conversion.round(float)->int` duplicates `Math.round(float)->int` (two natives, one
  operation, two modules), and `Conversion.truncate` has no Math sibling.

---

## 3. Asymmetric coverage (present-here-absent-there)

All **[Verified-read]**: absence checked against the full 236-row registry at HEAD.

| # | Where | Present | Missing counterpart |
|---|-------|---------|--------------------|
| 3.1 | String | `containsIgnoreCase`, `equalsIgnoreCase` | `startsWithIgnoreCase`, `endsWithIgnoreCase` — the approved P3 additions are **NOT shipped yet** (memory claims them as "planned"; confirmed absent). |
| 3.2 | String | `replace` (replaces ALL, [Verified-run: `("aaa","a","b")`→`bbb`]) | `replaceFirst` (approved P3, not shipped). Also no count-limited variant. |
| 3.3 | Set | `isSubset` | `isSuperset`, `isDisjoint`, `symmetricDifference` (approved P3, not shipped). A lone `isSubset` is the classic asymmetry smell. |
| 3.4 | Set | — | `map`/`filter`: List has both, Map has both, Set has NEITHER. Users must `Set.of(List.map(Set.toList(s), f))`. (Approved P3, not shipped.) |
| 3.5 | Math | `abs/min/max/clamp/sign` are **Int-only**; `pow` is Float-only (with `integerPower` for Int) | No Float variants of abs/min/max/clamp/sign (approved P3, not shipped). Meanwhile rounding siblings disagree on return type: `round(float)->INT` but `ceil/floor(float)->FLOAT`. |
| 3.6 | Bytes | `length` | `isEmpty` — String, List, Map, Set all have `isEmpty`; Bytes is the only sequence type without it. |
| 3.7 | Map | `keys`, `values` | `entries` (or pair iteration), `Map.of`/`fromList` — Set has `of`+`toList` round-trip; Map has NO constructor and NO list round-trip. |
| 3.8 | Map | — | **No empty-map literal exists at all**: `Map<string,int> e = [];` → `type error: cannot infer element type of empty list literal` (even with the declared type right there — the expected-type rule that admits empty `[]` for List args does not admit it for Map); `[=>]` → parse error. [Verified-run: both probes] The only way to an empty Map is `Map.remove(["k"=>v], "k")` or a filter-to-nothing. Genuine usability hole. |
| 3.9 | Csv | `parse(string)->List<string>` / `format(List<string>)->string` | Single-ROW only. No document-level parse (List<List<string>>), while Json got `parseLines`/`stringifyLines`. Undocumented scope surprise. |
| 3.10 | Decimal | `of(string)->decimal?` | No `Decimal.toString` sibling in-module (it's `Conversion.toString` generic) — minor, but `of` without a dual reads incomplete. |

---

## 4. Domain-validation philosophy is inconsistent (fuzz results)

Same class of bad input, three different outcomes — no discernible rule [all Verified-run]:

| Probe | Outcome |
|-------|---------|
| `List.chunk(xs, 0)` | **fault** `List.chunk size must be at least 1` |
| `List.fill(7, -1)` | **fault** `List.fill count must be >= 0` |
| `String.repeat("ab", -1)` | **fault** `Text.repeat count must be >= 0` |
| `String.count("aaa", "")` | **fault** `Text.count: the substring must not be empty` |
| `List.take(xs, -1)` / `drop(xs, -1)` | **silent clamp** to 0 / whole list |
| `Math.clamp(5, 10, 0)` (min > max) | **silent nonsense**: returns `10` (the min), no fault — Rust's own `clamp` would panic on this precondition |
| `String.padLeft("hello", 2, "*")` | silent no-op (width < len) — reasonable |
| `List.slice(xs, -2, 2)` | negative = PHP semantics (by design) |

`Math.clamp(min>max)` is the worst of these: it silently returns the wrong bound where every
neighbouring precondition violation faults. Either fault (consistent with chunk/fill/repeat) or
document the total behaviour.

Overflow/edge behaviour that IS consistent and good [Verified-run]: `Math.abs(INT_MIN)` →
`integer overflow in Math.abs` fault; `integerPower(2,63)` → `integer overflow`;
`integerDivide(1,0)` → `division by zero`; `String.parseInt("999…")` (overflow) → `null`;
`List.first/max([])` → `null`; `List.sum([])` → `0`; `Map.get` missing → `null`;
`Bytes.toString(invalid UTF-8)` → `null`.

---

## 5. DX / diagnostics findings

### 5.1 User-facing fault messages carry STALE module prefixes [Verified-run]
Runtime faults print the pre-rename module names:
- `runtime error at 3: Text.repeat count must be >= 0` — module is `Core.String`, not `Text`.
- `runtime error at 3: Text.count: the substring must not be empty` — same.
Grep confirms the whole family in code [Verified-read]: `text.rs` has ~31 `"Text.…"` message
literals; `convert.rs` says `"Convert.…"` (module is `Core.Conversion`); `validate.rs` says
`"Validate.…"` (module is `Core.Validation`); `bytes.rs` says `"Bytes.from_string"` /
`"Bytes.to_string"` — **snake_case names that never existed on the surface** (`fromString`/`toString`).
Most of these arg-shape strings are checker-shadowed (unreachable), but the domain-validation ones
(repeat/count/pad) are user-visible today, and ALL of them are parity-affecting fault strings by
Invariant 4 if they ever surface. One sweep would fix ~40 strings.

### 5.2 Fault-message FORMAT is inconsistent [Verified-run]
Four shapes observed in one probe session: `Text.count: <msg>` (colon), `Text.repeat <msg>`
(no colon), `integer overflow in Math.abs` (module-last), `division by zero` (no module at all).
These strings are canonical/parity-affecting (value.rs kernel discipline) — worth a single
canonical format (`Module.function: message`) before more natives land.

### 5.3 Unknown MEMBER reports unknown MODULE, no did-you-mean [Verified-run]
`String.lenght("abc")` (with `import Core.String;` present) →
`type error at 4:13: unknown identifier 'String' [E-UNKNOWN-IDENT]`.
The identifier `String` is *not* unknown — the member is. The message points the user at the
import (which is correct) instead of the typo, and offers no `did you mean length?`. Given the
project's M-DX investment in error quality, this is the weakest diagnostic found.

### 5.4 Checker errors inside interpolation inherit the W5-13 span reset [Verified-run]
`Output.printLine("{String.length(42)}")` on line 3 → `type error at 1:14` and the caret renders
under **line 1's text** (`package Main;`). Known issue W5-13 is filed as "VM reports line 1
inside `{…}`" — this shows it also mis-anchors CHECKER diagnostics (wrong line number AND wrong
quoted source line). Scope of W5-13 should be widened.

### 5.5 Diagnostic code coverage is uneven [Verified-run]
`E-UNKNOWN-IDENT` prints a code; arg-type (`String.length argument 1 expects string, found int`)
and arity (`String.indexOf expects 2 argument(s), found 1`) errors print NO `[E-…]` code.
`phg explain` presumably can't address the uncoded ones.

### 5.6 Positive findings (for balance) [Verified-run]
Outside interpolation, misuse errors are excellent: exact native name, expected/found types,
argument ordinal, caret on the call site (`d1b/d2b/d6/d7` probes). `Math.sqrt(4)` correctly
rejects int-for-float (no silent coercion) — strict but consistent with the typed-language
philosophy; whether ints should coerce to float in native args is an ADJUDICATION question
(§15), not a bug.

---

## 6. Lead re-verification summary (the three candidates handed to this audit)

| Lead | Status at HEAD `0691228` |
|------|--------------------------|
| `Bytes.find` vs `List.find`/`indexOf` | **CONFIRMED still present** — `Bytes.find` = index?-returning (i.e. `indexOf` semantics); approved rename not yet executed (§2.1). |
| `Map.has` vs `contains` | **CONFIRMED still present** (§2.3). |
| Slice uniformity | **CONFIRMED worse than the lead** — not just missing negative support on one type: three conventions, two arg meanings, two names (§2.6, all three probed live). |
| P3 additive batch (ignoreCase siblings, Set ops, Math float variants) | **CONFIRMED not shipped** — all absent from the 236-row registry (§3.1–3.5). |

## 7. Fuzz probe ledger

~44 distinct probe programs, 52 executions, ALL actually run (none reasoned-about-only):
- String: 15 programs (unicode length/case/reverse, substring negatives, repeat/count/pad/split
  edge cases, parseInt overflow, trim NBSP ×2, replace, splitOnce implicit) — plus 5 PHP legs.
- List: 5 (empty-list first/max/sum, slice negatives, take/drop negatives, chunk 0, fill −1).
- Map: 3 (get/getOrDefault/remove on missing, empty-map literal ×2).
- Bytes: 2 (slice negative clamp, find empty needle).
- Math: 8 (÷0, abs INT_MIN, integerPower overflow, round halves, clamp inverted, sqrt −1,
  pow 0^−1, numberFormat −2) — plus 2 PHP legs.
- DX misuse: 9 (wrong type, wrong arity ×2, missing import, member typo, in/out of interpolation).

Probes live in the session scratchpad (throwaway; not committed).

## 8. Suggested priority order (findings only — no fixes attempted, per brief)

1. **P0**: §1.1 trim family, §1.2 reverse, §1.3 split empty-sep — real Invariant-1 violations,
   each needs a same-commit differential case with the adversarial input baked in.
2. **P1**: §3.8 empty-Map literal hole; §5.3 unknown-member diagnostic; §2.11 Hash hmac/KDF
   type mismatch (API trap); §4 Math.clamp silent wrong answer.
3. **P2 (already approved by dev)**: Bytes.find→indexOf, Map.has→containsKey, slice unification —
   this audit adds `String.substring` naming and `List.count`/`String.count` to that discussion.
4. **P3 (already approved)**: §3.1–3.5 additive batch; plus new candidates `Bytes.isEmpty`,
   `Map.entries`/`Map.of`, Csv document-level, `secureIntBetween`-style rename (§2.9).
5. **Docs**: §1.4 pow deprecation note; byte-length semantics of `String.length` (7 for "straße")
   worth an explicit FEATURES.md line before W4-4 Unicode work re-opens it.
