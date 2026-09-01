# Json-ADT JIT slice — build plan (DEC-333 (a))

> **PROMOTED OUT OF `SLICE-STATE.md` on 2026-09-02** by the post-Slice-3 consolidation. This is a LIVE
> design for the NEXT perf slice (DEC-333 Phase B, item 1), not history — it was sitting inside a
> 2026-07-24 cursor block that the consolidation archived, and archiving Phase B's own build plan
> alongside spent session notes would have buried the design for upcoming work. Nothing else changed:
> the text below is verbatim as it stood.
>
> Status when promoted: **v7 plan COMPLETE, no gate rounds owed; the 6C MAXIMAL panel is owed AFTER
> the build.** Re-verify the current state before resuming — the baselines quoted here were measured
> in a container that no longer exists, and the perf-claim rule requires a fresh docker
> `php:8.5-cli`+JIT baseline with both sides core-pinned and interleaved.

# Json-ADT JIT slice — build plan v4 (DEC-333 (a); targets jsonround 0.30x / deepjson 0.90x)
# v5 = round-4 findings folded ([R4-*]: RoundingMode injected-collision + NullMark-return miscompiles,
# GetEnumField-Owned husk, materialize no-panic). v1-v4 folds retained ([R1..R3-*]).
# v4 = round-3 findings folded ([R3-*]); v2/v3 folds retained ([R1-*]/[R2-*]).

## Goal
Extend the JIT unboxed subset so the two bench bodies (and any Json-shaped hot code) compile to
native code. Byte-identity untouched: every gate fails closed to code-5 VM redo. The two
wrong-bytes-with-code-0 paths round 2 identified (call-result tag threading; un-tag-gated pair
release) are designed out explicitly below.

## Baseline (measured): jsonround 507.0M vs 149.6M (0.30x), deepjson 841.1M vs 754.8M (0.90x)
(container, php-8.5.8+opcache local). Dev-box canonical: 0.31x / 0.95x.

## Perf model (unchanged from v2, defended): the VM decomposition buckets are mostly dispatch
(validate_json = 29.2M of the 277.4M "parse loop"); JIT'd jsonround estimate ~750-900ns/iter vs
php dev-box 1195ns/iter → flip plausible; deepjson lazy top-level+data[0] vs php whole-doc
json_decode (7548ns/iter) → clear headroom. Container is INDICATIVE; dev-box canonical. ABORT
CRITERION: if the measured native-work floor exceeds the php budget at dev-box scale → HARD
FLAG + anatomy per DEC-269; parse-memoization of the const doc stays FORBIDDEN (bench measures
repeated parse; php pays it every iteration).

## Kinds (src/jit/analyze/kinds.rs)
- `Kind::Json(JRef, Own)` pair: payload vars[d], runtime tag evars[d]; tags 0..6 RELATIVE
  (prelude order Null,Bool,Int,Float,String,Array,Object), 7 = phorj null. Payload: 0/7 filler,
  1 bool, 2 i64, 3 f64-bits, 4 str handle, 5 JList, 6 JMap. Release TAG-GATED and MANDATORY
  (per-iteration values; Dyn's leak doctrine does NOT apply). Json(_, Owned) in is_owned_handle;
  Json in is_handle.
- `Kind::JMap(Own)` / `JList(Own)`: untagged handles to boxed Value::Map/List (JsonLazy inside).
- `Kind::NullMark`: Const(Value::Null) marker; Eq/Ne vs Json → icmp tag ==/!= 7. RETURN/ENTRY
  GATE [R4-corr-1: the Const(Null)→NullMark accept is GLOBAL, so a function with a top-level
  `return null` would carry NullMark as its ret kind and run_unboxed decodes the filler word 0
  as Int(0) — wrong bytes where the VM returns null]: NullMark joins the Return decline list
  (mirrors the existing IntSet :2374 / MapList :2380 return declines) AND the compile.rs
  entry-ret gate; confirm SetLocal-then-return + MakeMap-value consumers decline too
  (GetEnumField/MatchTag/Call-into-Dyn already reject it). OPERAND-TRANSIENT INVARIANT
  [R5-corr-1: the `Json? x; if(c) x=parse(s); else x=null` shape could otherwise merge a NullMark
  into a Json slot via join_kind and mis-decode the filler word]: NullMark is produced ONLY by
  Const(Null) and consumed ONLY by an immediately-following Eq/Ne-vs-Json in the SAME block;
  SetLocal(NullMark) declines, Return(NullMark) declines (above), and `join_kind(NullMark, ·) →
  None` so a NullMark can never survive to a leader/merge (the if/else-null shape declines,
  fail-closed) — unit-tested as a mandatory join arm. An OWNED Json
  operand gets tag-gated release-on-consume [R1-F6] — this release is the FIFTH evars site,
  emitted INSIDE the new Json-Eq/Ne arm placed BEFORE the generic arm_cmp dispatch
  (emit mod.rs:1060; the str-Eq precedent at :1044 releases via its own meta-mask, not
  release_kinded) [R3-comp-F2]. ANALYZE GATE [R3-comp-F3]: NullMark is admitted ONLY opposite a
  Json operand — every other pairing (NullMark,Int/Bool/Str/...) rejects in the analyze Eq/Ne
  arm, never relying on arm_cmp/checker downstream.
- MANDATORY lattice arms with unit tests pinning each [R2-B5: borrowed_copy's `other => other`
  catch-all silently yields Owned→Owned double-free — the one unsafe-by-default catch-all]:
  join_kind (V(a)⊔V(b)→Any, V⊔Any→Any, own per join_own; JMap/JList; NullMark),
  borrowed_copy (Json/JMap/JList), is_handle/is_owned_handle, join_unknown_bottom
  (analyze/mod.rs:73-110).

## Canonical-Json identification [R2-safety-F5 — replaces v2 shape-sniffing]
NEW program-level compiler fact `BytecodeProgram::canonical_json: Option<u32 /*desc base*/>`,
stamped by the compiler pre-pass ONLY for the injected `Core.Json` enum (the checker sees
`injected` + true field types; EnumDesc carries neither). The provenance PLUMBING is explicit
work [R3-safety-F3]: the `injected` flag lives on the AST `Item::Enum` — the enum_descs
pre-pass (compiler/program.rs:78-99) reads it there and stamps the fact; nothing may fall back
to shape inference. STAMP CONDITION [R4-corr-2: `injected` is TRUE for EVERY injected enum —
RoundingMode/Option/Result — so `injected` ALONE would stamp the first injected enum and
miscompile e.g. RoundingMode's MakeEnum/MatchTag as Json variants]: the fact is stamped iff
`injected && e.name == "Json" && <the 7-variant (name,arity) shape matches the prelude>` — all
three conjuncts. A user-declared look-alike `enum Json`
never sets it (no `injected`) → all arms decline (the v2 (name,arity) sniffing was a miscompile
hole; `injected`-only was the R4 collision). Unit
test pins the helper-side variant→0..6 mapping + payload representation against the prelude;
compile-setup debug_assert pins fact-order == prelude-order. json_base>0 (a preceding enum
shifts descriptor indices) gets a dedicated unit test [R2: benches have base=0 — the rel-tag
subtraction is otherwise untested].

## Entry ABI [R1-P0 deepjson + R2 hardening]
- NEW `Function::str_params` checker fact (bitmask; dyn_params twin — stamped at
  compiler/program.rs:587, ctors.rs:112/197, lambda.rs:130 sites + chunk field). Seeding is
  ROOT-FUNCTION-ONLY [R2-B6: seeding internal callees would clobber call-site-proven Owned args
  → leak; internal callees get Str kinds from call_sigs already]. Root params seed
  Str(Own::Borrowed). GATING MECHANISM [R3-comp-F1: param_kinds has no entry knowledge today]:
  `entry_idx` is stored in UbGraphInfo (resolve_unboxed_graph has it, collect_unboxed.rs:302)
  and the seed applies iff func_idx == info.entry_idx — dyn_params-style unconditional
  application would be the exact R2-B6 leak.
- run_unboxed reorder [R2-safety-F2]: build/reset the ctx FIRST (compile.rs:398-413), THEN
  marshal Value::Str args into fresh untagged handles via cached.as_deref_mut() and splice into
  `ia` (today `ia` is frozen at :385-395 before the ctx exists). Arg handles land past n_pinned
  — no const collision (verified: reset truncates to n_pinned; alloc returns past it).
- COMPILE-TIME entry gates (release builds — the debug_asserts are backstops, not the mechanism
  [R2-B4]): parallel to the Inst-return decline at compile.rs:153 — decline entry ret ==
  Json(..); decline any entry param kind ∉ {Int, Float, Bool, Str(Borrowed)} BEFORE make_fn_sig
  (kills the `_ => 0` silent-zero forever). debug_assert at the transmute: entry sig returns == 2.
- DOCUMENTED HONESTLY [R2-safety-F2]: untagged-Borrowed release protection is compile-time-only
  (release() on untagged words is unconditional — no runtime owned-bit); the Kind discipline is
  the entire wall. Verify during build that no inline str fast path assumes SLOT bits on an
  entry-marshalled (untagged) str param — helpers use str_bytes (untagged-safe).

## Internal 3-return ABI (Json-returning callees) [R2-B1 is the P0 fix]
- make_fn_sig gains the callee's ret kind (info.ret_of available at both call sites :246/:261)
  → 3rd return for Json. Heterogeneous per-function arities are Cranelift-safe (verified).
- emit_call_to: thread `evars` in (new param); a dedicated ret==Json branch reads payload=r[0],
  tag=r[1], code=r[2], branches the fault dispatch on r[2], `def_var(evars[kinds.len()], tag)`
  BEFORE the ub_push (ub_push writes one word only) [R2-B1 — without this the tag is misread as
  the fault code AND the stored tag goes stale: wrong bytes on both benches]. debug_assert
  results.len()==3 at the fork. Wired for Op::Call + Op::CallValue + Op::CallMethod
  (mod.rs:1358-1380); HOF sites defensively decline Json rets.
- fault_exit + Return arm key off info.ret_of(fi) at BODY SETUP (not ret_kind_out mid-loop):
  Json-ret frames emit 3-word terminators ((0,0,code) / (payload,tag,code)); the fault-exit
  BLOCK PARAM arity is unchanged (code word only — payload/tag are constants in the tail).
  Return arm's Json branch reads evars for the tag; Borrowed Str/Json returns clone at the
  boundary (tag-gated for pairs).
- Throwing × Json-ret decline [R3-corr-NEW-1 SUPERSEDES R2-B8's op-keyed form]: decline at body
  setup when `info.ret_of(fi) == Json && info.thrown_class.is_some()` — the THROWING-GRAPH flag,
  not "frame contains Op::Throw": a Json-ret function that merely CALLS into a throwing graph
  emits the throwing dispatch whose no-pad arm returns 2 words in a 3-return frame (malformed
  IR; today saved only by the Cranelift verifier's implicit fallback). This gate also moots the
  code-6 decode ambiguity for Json-ret callees. Benches don't throw; recorded deferred.

## Flow-sensitive variant refinement (verified necessary + correctly scoped [R2-A2])
Peephole at JumpIfFalse when same-block prefix is exactly `GetLocal(s); MatchTag(t)` and cell s
is Kind::Json: propagate the UNREFINED kinds to the branch target (clone taken before refining)
and a REFINED clone (cell s → V(rel t)) to the fall-through leader — both edges already go
through `propagate` (analyze/mod.rs:2298-2302), so the split is local. DEFENSIVE INTERFERENCE
CHECK (mirror accumulator_site): refine only if no reachable SetLocal(s) inside the refined
region. GetEnumField on Any declines. Non-Json JumpIfFalse behavior byte-identical to today.

## Op-arm extensions (analyze + emit, bodies in the new json.rs files)
- MakeEnum(idx ∈ canonical range, arity ≤1): pop payload of the variant's kind (Borrowed
  payloads W9-clone first), push Json(V(rel), Owned).
- MatchTag on Json: icmp tag vs rel(idx) → Bool (tag 7 false everywhere — VM-identical; Fault
  backstop → code 5). Wildcard/default arms emit no test — no new ops [R2: json-api's intOf
  shape is a new real JIT target; test added].
- GetEnumField(0) on Json(V(t)): payload with variant kind; Borrowed→Borrowed. Owned pair →
  DECLINE [R4-comp-1: the Owned→TRANSFER arm had no husk-neutralization — a transferred payload
  whose husk cell is still live at the match-collapse SetLocal(m_slot) would double-free; and
  the arm is effectively DEAD because GetEnumField is emitted ONLY by the match desugar, which
  always extracts from a BORROWED GetLocal(m_slot) copy (register_bindings). Fail-closed decline
  removes the double-free risk with zero coverage loss].
- Eq/Ne (Json, NullMark) either order (VM equivalence verified: eq_val Enum-vs-Null = false,
  Null-vs-Null = true → icmp tag==7 exact [R2-A1]).
- GetLocal/SetLocal of Json: copy/store BOTH words. SetLocal ORDER: (1) clone popped Borrowed
  Str/Json FIRST, (2) tag-gated release of the overwritten Owned cell, (3) store. The existing
  borrowed-handle-store DENY at mod.rs:1135-1147 (+ analyze mirror :1769-1782) is RELAXED for
  Str/Json (clone-first); other kinds keep the deny [R2-B3]. Old=Borrowed → no release
  (firstRecord scrutinee shape, [R2-A4]). RETAG [R3-corr-C1]: after clone-first the stored kind
  is the OWNED variant — BOTH the emit arm (mod.rs:1169) and the analyze arm (mod.rs:1790)
  retag kinds[slot] to Owned (a Borrowed retag would leak every clone → cap → spurious redos).
- Pop of Json: tag-gated release (evars threaded into arm_pop [R2-B2]).
- Op::Index (Int subscript) on JList → rt_u_json_list_get → Owned Json pair [R2-comp-F1: was
  missing — firstRecord's xs[0]]; Core.List.length JList branch in arm_list_len [same].
  LOCKSTEP [R3-comp-Fold1]: the JList emit arm goes BEFORE the `Op::Index => arm_index_str_list`
  catch-all at emit mod.rs:520, which silently ASSUMES StrList (a miss there = wrong bytes).
- MakeMap (Str keys, Json values) → jmap scratch-list build + seal via canonical build_map.
- Call plumbing [R2-B7/comp-F2]: a DEDICATED pk==Json branch in pop_call_args as a TOP-LEVEL
  branch mirroring the Dyn block's structure (`rev.push(vec![payload, tag]); continue` — NOT an
  arm inside the one-word `match k`, whose arms all `rev.push(vec![v])` and would drop the tag
  [R3-comp-F5]); two words, NO CLONE — callee is read-only-borrowed (explicit rule). The analyze
  Call AND CallMethod arms record Json args explicitly (not via fallthrough); make_fn_sig +1
  word for Json params (compile.rs:225-229); the callee-side entry/arg decode loop
  (emit_unboxed/mod.rs:197-213, keyed at :203) learns Json pairs.
- Release plumbing [R2-B2 + comp-F-release]: emit_release signature UNCHANGED (~20 callers
  untouched); NEW `emit_release_pair(payload, tag)` gates on tag∈{4,5,6} then delegates.
  `release_kinded` gains an `Option<ClValue>` TAG param (distinct from the existing `exclude`)
  with an explicit Kind::Json arm → emit_release_pair (the current non-Inst catch-all at
  objects.rs:304 would free a scalar payload). evars threaded into arm_pop,
  emit_unwind_releases, SetLocal-release, and emit_call_to (throw-routing sites pass the tag).

## Natives + helpers (rt_u_json_*, new src/jit/handles/json_ext.rs; feature `json` bodies +
cfg(not(json)) STUBS sharing ONE signature declaration + a type-anchor const so drift is a
compile error [R2-safety-F5c]; stubs are runtime-dead — Core.Json imports are E-EXTENSION-
DISABLED without the feature, and canonical_json is never stamped)
Two-i64-return (payload, tag), tag<0 = fault → code 5. NO-PANIC discipline (extern "C"):
bounds-check before slicing; Rc::get_mut → -1; whole-doc validate_json BEFORE minting lazy
children (keeps materialize_lazy's .expect unreachable); no OnceCell re-entrancy. EXTENDED
[R4-safety-1]: the discipline covers EVERY materialize_lazy call site — not just parse but the
map_get / GetEnumField-on-container helpers that force one level — since materialize_lazy
carries `.expect("re-parse cannot fail")` (lazy.rs:308); a future skip/materialize drift
panicking there would abort across the extern "C" boundary (vs a clean VM unwind). Each site is
wrapped (or the invariant re-asserted locally), never inherited transitively.
- rt_u_json_parse(ctx, s, free): validate; invalid → tag 7; valid → eager ONE-level materialize
  (children lazy — materialize_one semantics). PARSE-ARG FREE CONTRACT [R3-safety-F1]: free =
  compile-time-ownership (Owned ⇒ 1 — `Json.parse(a + b)` reaches here with an Owned Concat
  result); the helper builds/clones the `src` PhStr for ALL THREE input representations (boxed
  Value::Str → Rc-bump; arena SLOT → byte copy; ACC → byte copy) BEFORE issuing the free —
  free-before-build on a boxed Owned input is a use-after-free. Test: `Json.parse(a + b)`.
  Recursion note [R3-safety-F2]: validate/skip_value recursion depth is unguarded — shared
  verbatim with the VM's parse (same fn), so parity-preserving; documented, not new.
- rt_u_json_map_get(ctx, map_h, key_h, free_mask): linear HKey::Str scan (VM map_get mirror);
  miss → tag 7; hit → materialize_if_lazy → Value→pair; EVERY non-filler payload (strings AND
  containers) wraps in a FRESH handle — never alias the map's interior [R2-A5 residual].
  free_mask strictly compile-time-ownership-driven (Borrowed/ConstBorrow ⇒ 0).
- rt_u_json_list_len / rt_u_json_list_get (OOB → tag<0 → code 5) / rt_u_json_stringify
  (pair→Value via interned names → canonical encode) / rt_u_json_clone (tag-gated).
- rt_u_jmap_push/seal: the rt_u_map_push_pair pattern EXACTLY — untagged Value::List scratch
  (fresh Rc, get_mut None→-1 defensive), seal → build_map kernel (first-position/last-value
  dedup) → mint. NO Rc::get_mut on Value::Map ever [R2-safety-F6].
- MINT CAP [R2-safety-F1 — replaces v2's shared-alloc cap, which would turn rt_u_native2 hits
  into code-0 bad handles]: a json-only `alloc_json(v)` in UbCtx capping LIVE untagged count
  (handles.len() − free.len()) at 4×UB_SLOT_CAP → -1 → code 5. Shared alloc() stays infallible.
- Debug backstops [R2-safety-F4]: debug_asserts on double-free in all THREE recycle paths
  (free vec, free_storage slot stack, acc_free record pool). Stated honestly: release-mode
  double-free protection remains compile-time ownership discipline only.
- Admissions: unboxed_native_is_json_parse/_stringify (pure); Core.Map.get = FULLY NEW
  admission (JMap receiver; StrIntMap Map.get stays undeclared); Core.List.length + Index on
  JList; String.length existing (Owned operand freed by the helper — verified clean).
- 5-site lockstep (helper_refs/declares/symbols/refs/json_ext) unconditional via the stubs.
- Any new UbCtx state joins reset_for_run (none planned; checklist).

### 5b API POINTERS (runtime scan, VERIFIED 2026-07-24 — so 5b is implementable from repo state alone, Inv 19)
Kind::Json is a REGISTER PAIR (payload word `vars[d]`, tag word `evars[d]`); rel tags Null=0 Bool=1
Int=2 Float=3 String=4 Array=5 Object=6, 7 = phorj null (the `Json?` None). Payload per tag: 0/7
filler; 1 bool(0/1); 2 i64; 3 f64-bits; 4 str handle; 5 JList handle; 6 JMap handle. JList/JMap
(`Kind::JList/JMap`) = UNTAGGED `ctx.handles` indices boxing `Value::List`/`Value::Map` (whose
elements/values are `Value::JsonLazy` children) — minted via `ctx.alloc(...)`; the register-pair
`EnumInt` vertical CANNOT represent container Json nodes (they must be boxed handles).
- **Value shapes** (`src/value/types.rs`): `Value::Enum(Rc<EnumVal>)` :155; `EnumVal{ty:Rc<str>,
  variant:Rc<str>, payload:Payload}` :364; `Payload::{Zero, One(Value), Many(Vec<Value>)}` :304
  (methods `first()->Option<&Value>`, `as_slice()`, `Index` — NEVER `[]` a `Zero`; use `first()`).
  `Value::Map(Rc<Vec<(HKey,Value)>>)` :147 (insertion-ordered, NOT a hashmap); `HKey::{Int,Bool,
  Str(PhStr)}` :377. `Value::List(Rc<Vec<Value>>)` :141. `Value::JsonLazy(Rc<LazyJson>)` :162
  (cfg json); `LazyJson{src:PhStr,start:usize,cached:OnceCell<Value>}` :104.
  A Json Object node = `Enum{variant:"Object", payload:One(Value::Map(..))}`; Array =
  `Enum{variant:"Array", payload:One(Value::List(..))}`.
- **Callable-from-`src/jit/` entry points**: `crate::ext::json::...::json_parse_str(s:&str,
  out:&mut String)->Result<Value,String>` is **pub(crate)** (`ext/json/natives.rs:185`) — returns
  `Ok(JsonLazy)` on valid / `Ok(Value::Null)` on malformed; USE THIS for rt_u_json_parse (do NOT
  reach for `validate_json`, which is `pub(in crate::ext::json)` — NOT visible here).
  `materialize_if_lazy(Value)->Value` (`ext/json/natives.rs:191`, pub) forces one level;
  `materialize_lazy(&LazyJson)->Value` (`ext/json/parser/lazy.rs:298`, pub) — its `.expect` is
  `lazy.rs:308`, reachable only on an internal validate/build divergence (never on user-malformed
  input → that's `Null` at parse). `crate::value::build_map(Vec<(Value,Value)>)->Result<Vec<(HKey,
  Value)>,String>` (`value/collections.rs:40`, pub; dedup = first-position/last-value). Json
  variant NAME→order SSOT: `JSON_VARIANTS` `ext/json/natives.rs:31` (Null..Object).
- **Helper patterns to mirror** (`src/jit/handles/mod.rs`): `rt_u_map_push_pair` :1100 (scratch
  `Value::List` append via `Rc::get_mut→ -1`); `rt_u_map_seal` :1143 (`build_map` then
  `ctx.alloc(Value::Map(Rc::new(..)))`); `rt_u_map_get` :1198 with `#[repr(C)] UbMapGetRet{value,
  code}` :1188 (2×i64 return; `code:5`=redo-VM). Every helper's 1st line `let ctx=unsafe{&mut
  *ctx};`; defensive→ `-1`/`code:5`; reads via `ctx.handles.get(h as usize)` / `ctx.str_bytes(h)`;
  `ctx.alloc(v)` :354; `ctx.release(h)`; `seal_flat_entries` `maps_ext.rs:90` (pub(in crate::jit)).
- **5-site wiring** (representative `map_push_pair`): `handles/helper_refs.rs` UbHelperIds :19 +
  UbHelperRefs :61 · `src/jit/declares.rs` :49 (sig helpers :28-40) · `handles/symbols.rs` :16 ·
  `emit_unboxed/refs.rs` :21 · impl in `handles/mod.rs`. New two-i64 helper needs its own
  `#[repr(C)]` ret struct + a bespoke sig pushing a 2nd `AbiParam::new(I64)` onto `.returns`.
- **UB tags** (`handles/mod.rs`, all pub(super)): SLOT `1<<62` :48, FLAT `1<<61` :50, OWNED
  `1<<60` :52, ACC `1<<59`, IDX_MASK `(1<<40)-1` :128, SLOT_SIZE 64 :144, SLOT_CAP 4096 :147.
- **alloc_json mint cap** [R2-safety-F1]: a json-only `alloc_json(v)` on UbCtx capping LIVE
  untagged count (`handles.len()-free.len()`) at `4*UB_SLOT_CAP` → `-1`→code 5; shared `alloc()`
  stays infallible (turning rt_u_native2 hits into bad handles was the v2 hazard this replaces).

## collect_unboxed gates
Accept: Const(Value::Null) → NullMark ONLY WHEN THE VERY NEXT OP IS Eq/Ne [R6-corr-1 NARROWED:
a GLOBAL Const(Null) accept admits NullMark into contexts the operand-transient invariant
declares impossible — list/tuple DESTRUCTURE (compiler/stmt/core.rs:278,:321) emits Const(Null)
as binder-slot PLACEHOLDERS that stay live across leaders, never adjacent to Eq/Ne. The
peephole accept `Const(Null)` iff `code[ip+1]` is Op::Eq|Op::Ne makes the invariant true BY
CONSTRUCTION: every other Const(Null) (destructure placeholders, any non-comparison null) keeps
today's VM fallback (unchanged behavior, fail-closed). This is a scope NARROWING — strictly
safer than v6]. Belt-and-suspenders: SetLocal/GetLocal/Call/CallMethod-arg arms still explicitly
decline a NullMark operand, and the join_kind NullMark→None arm is ordered BEFORE the `a==b`
short-circuit at kinds.rs:230 [R6-safety-1: else join_kind(NullMark,NullMark) returns
Some(NullMark) via the fast-path and the mandated →None unit test fails].
CallNative json ids (uses_handles), MakeEnum canonical range arity ≤1, Index/List.length
already op-accepted (kind-gated in analyze).

## Files + size-gate reality [R2-comp-F3]
NEW: src/jit/analyze/json.rs, src/jit/emit_unboxed/json.rs, src/jit/handles/json_ext.rs,
src/jit/tests/json_adt.rs, PLUS an M-Decomp split of src/jit/compile.rs (at 498 of HARD 500;
the entry/run_unboxed/make_fn_sig work lands there → split the run/entry half into
src/jit/compile/ mod + run.rs BEFORE feature work, split-as-you-go per Inv 13).
AT-BASELINE grandfathered files that WILL grow and must net-out or bump-with-disclosure:
analyze/mod.rs (2435), emit_unboxed/mod.rs (1641), handles/mod.rs (1982), compiler/program.rs
(697, +1 str_params stamp), vm/tests.rs (563, +7 Function-literal inits). kinds.rs 291/300soft.
Plan: net-out via comment consolidation where honest, else baseline bumps disclosed in the
commit + register row (slice-1 precedent, dev may revert). kinds.rs WILL cross the 300 soft cap
(advisory WARN, gate-verified non-failing) — disclosed here [R3-comp-F6]. BUILD-TIME VERIFY
STEP [R3-corr-C2]: before shipping the str_params widening, audit every inline str fast path
for SLOT-bit assumptions against an untagged entry-marshalled param (emit_arg_clone's
band==SLOT check is correct-by-fallthrough; each other site must be checked, not assumed).

## Tests (json_adt.rs; assert_jit_hits = hits>0 + tree-walk parity + redos==0)
1. jsonround-shaped mini + handle-table stability (live-count returns to base per iteration).
2. deepjson-shaped mini WITH `bench(string doc, int iters)` — the str-param entry marshal IS
   the test; an internal-const variant would false-green.
3. Construction+stringify; parse+match; missing-key coalesce → Json.Null arm; malformed doc →
   if-var else; `parse(x) == null` direct (Owned-operand Eq release); `Json.parse(a + b)`
   (Owned parse arg — the free-after-build contract [R3-safety-F1]).
4. In-bounds xs[0] HIT (hits>0 — the firstRecord shape) AND OOB → REDO_ON_VM + VM parity
   [R2: fault-only coverage would miss F1]; Bool/Float payload variants; default-arm match
   (the json-api intOf shape — wildcard lowering).
5. Cross-function: Json param callee with hits>0 asserted ON THE CALLER→CALLEE fast path
   [R2: fallback-only would false-green]; Json RETURN (3-return, firstRecord shape); synthetic
   Json-returning METHOD (CallMethod decode; corpus has none — defensive); borrowed-payload-str
   store (topString clone-before-release regression).
6. Fallback soundness: nested syntactic pattern declines + parity; user look-alike `enum Json`
   → canonical_json None → EnumInt/VM paths (the R2 miscompile hole pinned as a test); throwing
   Json-ret function declines; existing enum/Dyn/accumulator regressions.
7. kinds unit tests (borrowed_copy/join_kind/join_unknown_bottom arms); json_base>0 rel-tag
   test; helper variant-mapping pin.
Full differential + conformance (json-api.phg's intOf/summarize now real JIT targets) + full
quality gate (--all-features, --no-default-features, fmt, size-gate, release build).

## Perf verification (Inv 11, WIN-OR-FLAG): microbench.sh jsonround deepjson before/after;
regression sweep mapget/listcontains/sumby/mapmerge/stringlen; dev-box canonical; abort
criterion above.

## Out of scope (recorded): Http.jsonParse vertical (queryparse — ✅ BUILT via DEC-338, near-parity), Json class fields,
