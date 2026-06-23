# checker.rs decomposition map (raw research)

**File:** `src/checker.rs` — **9786 lines** (verified `wc -l`).
**Goal:** behavior-preserving split into `checker/` module dir (precedent: `bundle.rs` → `bundle/` with
`mod.rs` + 6 sibling files, each `impl`-bearing or pure-fn).
**Constraint honored:** NO OOP/SOLID/visitor recs. Rust exhaustiveness is the safety net; keep every
exhaustive `match` whole; split by file, not by trait abstraction.

---

## 0. Top-level shape (verified)

| Region | Lines | Size | Content |
|---|---|---|---|
| Header `use` + struct defs | 1–176 | 176 | `FnSig`/`EnumInfo`/`ClassInfo`/`HookInfo`/`InterfaceInfo` (private), `pub struct Checker` (24 fields) |
| **`impl Checker { … }`** | **177–5324** | **~5147** | THE WHALE — 1 monolithic impl, ~110 methods |
| Free functions | 5328–5680 | ~352 | `lambda_uses_this`, `check`/`run_checker`/`check_resolutions` entry points, case helpers, `apply_subst`, `ty_has_param`, totality helpers, `is_builtin_type_name` |
| `pub fn resolve_html` | 5681–5998 | 317 | self-contained AST rewrite (nested fns only) |
| `pub fn erase_generics` | 5999–6477 | 478 | self-contained AST rewrite (nested fns only) |
| `pub fn expand_aliases` | 6478–6767 | 289 | self-contained AST rewrite (nested fns only) |
| `#[cfg(test)] mod tests` | 6768–9786 | **~3018** | ~190 unit tests + helpers (`prog`, `errors_of`, …) |

**External API surface is tiny** (verified `grep checker:: src/*.rs` minus self): only
`check`, `check_resolutions`, `resolve_html`, `erase_generics`, `expand_aliases`. `pub struct Checker`
is `pub` but never referenced outside the module. → A `checker/` dir can re-export exactly these 5 fns
from `mod.rs`; nothing else needs to cross the module boundary.

---

## 1. Proposed cohesion clusters

The whale `impl Checker` is already a **thin-dispatcher design**: `check_expr_inner` (3055) and
`check_stmt` (2724) are giant `match`es that delegate to per-construct `check_*` methods. This is the
single most important finding — it means the impl splits cleanly into multiple `impl Checker { … }`
blocks across sibling files **without touching the dispatch matches' shape**, because each arm is
already a one-line `self.check_xxx(...)` call. Rust allows multiple `impl Checker` blocks in different
files of the same module; all private fields stay visible to every file in the `checker/` dir.

### Recommended file layout

| File | Pulls (line ranges) | ~Lines | Purpose |
|---|---|---|---|
| `checker/mod.rs` | 1–176 (struct + fields), 5328–5466 (entry fns), re-exports | ~330 | `pub struct Checker` + fields, `new`, `err`/`err_coded`/`warn_coded`/`err_assign` diagnostic ctors (221–275), scope prims `push_scope`/`pop_scope`/`declare`/`declare_binding`/`lookup`/`lookup_binding`/`in_scope_names`/`nearest_name` (276–302, 2462–2541), `run_checker`/`check`/`check_resolutions`, module decls + re-exports |
| `checker/resolve.rs` | `resolve_type` 303–561, `no_args`/`one_arg` 562–585 | ~285 | type-syntax → `Ty` resolution (the `Type::Named` exhaustive match), arity guards |
| `checker/collect.rs` | 586–1672 | ~1086 | declaration hoist pass: `collect`, `collect_trait`/`_interface`/`_function`/`_enum`/`_class`, `check_interface_graph`, `inherit_class_members`, `merge_inherited`, iface cycle/flatten helpers, `is_subtype`, `validate_new_overload`, `validate_type_params` |
| `checker/throws.rs` | `flatten_throws`/`is_error_type`/`throws_declared`/`covered_by_try`/`free_call_throws`/`try_throws_propagate`/`validate_throws_decl` 1400–1527 | ~128 | M-faults checked-exception machinery |
| `checker/program.rs` | `check_program` 1922–2009, `check_type_body` 2010–2100, `check_function` 2425–2461, `check_body`/`check_block`/`check_return_totality`/totality engine 2542–2703 | ~390 | top-level driver: program walk, function/body checking, return-on-all-paths totality |
| `checker/casing.rs` | 2101–2424 | ~323 | `check_casing`/`check_fn_casing`/`check_stmt_casing`/`check_expr_casing`/`want_name_case`/`want_type_case` (the `Item`/`Stmt`/`Expr` casing-walk matches) |
| `checker/stmt.rs` | `stmt_span` 2704–2721, `check_stmt` 2722–3038, `check_for`/`check_while`/`check_cfor` 4957–5052 | ~440 | statement-level checking (the `Stmt` exhaustive match) + loop statements |
| `checker/expr.rs` | `check_expr`/`check_expr_inner` 3039–3174, `check_unary`/`check_binary`/`check_instanceof`/`check_str`/`check_html`/`expr_span`/`check_list`/`check_map`/`check_index`/`check_range`/`check_if_expr`/`check_lambda` 3175–3665 | ~626 | expression dispatch (the `Expr` exhaustive match) + literal/operator/collection exprs |
| `checker/calls.rs` | `check_call`…`check_member`, `class_subst`/`enum_subst` 3666–4477 | ~810 | call resolution: named/overload/generic/native/method/member calls, variant-or-class construction, arg checking, `unify`, class/enum substitution |
| `checker/assign.rs` | `check_local_reassign`/`check_index_assign`/`check_field_assign`/`check_clone_with`, Result helpers, `check_propagate`/`check_intrinsic_call`/`check_force`/`err_opt_use`/`opt_wrap` 4478–4956 | ~478 | assignment & mutation checks (M-mut), propagate/force/optional helpers |
| `checker/matches.rs` | `check_match`/`check_pattern`/`expect_prim` 5053–5327 | ~275 | match exhaustiveness + pattern checking (the `Pattern` matches) |
| `checker/casehelpers.rs` *(or fold into mod.rs)* | `is_intrinsic_name`/`levenshtein`/`leaf_ident`/`is_camel`/`is_pascal`/`case_words`/`upper_first`/`to_camel`/`to_pascal`/`is_true_lit`/`breaks_this_loop`/`match_arm_key`/`is_builtin_type_name`/`lambda_uses_this`/`apply_subst`/`ty_has_param` 5328–5680 | ~352 | stateless free helpers (no `self`) |
| `checker/rewrite_html.rs` | `resolve_html` 5681–5998 | 317 | post-check `html"…"` / propagate erasure AST rewrite |
| `checker/rewrite_generics.rs` | `erase_generics` 5999–6477 | 478 | generic-type-param erasure AST rewrite |
| `checker/rewrite_alias.rs` | `expand_aliases` 6478–6767 | 289 | `type` alias expansion AST rewrite |
| `checker/tests.rs` | 6768–9786 | ~3018 | `#[cfg(test)] mod tests` moved wholesale (single biggest line win, zero risk) |

**Biggest single win, zero risk:** moving the **3018-line test module** to `checker/tests.rs` cuts the
file by ~31% with no behavior surface at all (`#[path]` or `mod tests;` re-include). The three
self-contained `rewrite_*.rs` files (1084 lines combined) are the next-cleanest pull — pure
`Program → Program` functions with only nested `fn`s.

---

## 2. State coupling (the `Checker` struct)

24 private fields, **all touched diffusely** across the impl (verified counts):

| Field | refs | Concentration |
|---|---|---|
| `classes` | 33 | collect, calls, assign, expr (member/method), resolve |
| `active_type_params` | 19 | resolve, collect, calls (generic) |
| `interfaces` | 17 | collect, resolve, calls |
| `enums` | 14 | collect, calls, matches |
| `cur_ret` | 11 | program (save/restore), stmt (return) |
| `funcs` | 8 | collect, calls |
| `loop_depth` | 8 | stmt (loops), program |
| `scopes` | 6 | scope prims, stmt, expr |
| `cur_throws`/`cur_is_main`/`try_catch_stack`/`cur_class` | 5 each | throws, program, expr |
| `aliases`/`alias_stack`/`depth`/`cur_class_type_params` | 3–4 | resolve, expr |
| `imports`/`errors`/`warnings`/`skip_throws_discharge`/`html_resolutions` | 1–2 | scattered |
| `class_implements`/`class_supertypes` | 1 each | collect (write) → consumed by backends |

**Verdict: the fields do NOT partition.** No subset of methods owns a private subset of fields. The
hot fields (`classes`, `active_type_params`, `interfaces`, `enums`, `cur_ret`, `scopes`) thread through
nearly every cluster. **This forbids a "split the struct into sub-structs" decomposition** — and that's
fine. The correct Rust idiom (used by rustc itself) is: **one struct, many `impl` blocks in sibling
files of one module.** Within a module, child files declared via `mod` see the parent's private
fields, so each `checker/<cluster>.rs` can write `impl Checker { fn check_xxx(&mut self) {…} }` and
freely touch every field. No field needs to become `pub`. No accessor methods needed.

This is exactly the `bundle/` precedent's mechanism applied to a stateful struct instead of free
functions.

---

## 3. Exhaustive `match` locations (the safety-net couplings — MUST stay whole)

Each of these matches the full set of an AST/`Ty` enum; adding a variant forces the compiler to flag
the arm. **A match cannot be split across files** — but every one of these lives inside a single method,
so each lands intact in exactly one cluster file:

| Line | Method | Enum matched | Lands in |
|---|---|---|---|
| 459 | `resolve_type` | `Type::Named { name }` (built-in type names) | `resolve.rs` |
| 590 | `collect` | `Item` | `collect.rs` |
| 1990 | `check_program`/`check_type_body` | `Item` | `program.rs` |
| 2104 | `check_casing` | `Item` | `casing.rs` |
| 2197 | `check_stmt_casing` | `Stmt` | `casing.rs` |
| 2297 | `check_expr_casing` | `Expr` | `casing.rs` |
| 2724 | `check_stmt` | `Stmt` (full statement dispatch) | `stmt.rs` |
| 3055 | `check_expr_inner` | `Expr` (full expression dispatch) | `expr.rs` |
| 3062/3095 | nested in expr | `Option<Ty>` / `cur_class` | `expr.rs` |
| 3955 | `unify` | `(Ty::Param, _)` | `calls.rs` |
| 5339/5371/5606 | `in_expr`/`in_stmts`/`breaks_this_loop` | `StrPart`/`Stmt` | helpers |
| 5690/5758 | `resolve_html::rexpr` | `Expr::Html`/`Expr::Propagate` | `rewrite_html.rs` |
| 5946 | `resolve_html` top | `Item` | `rewrite_html.rs` |
| 6338 | `erase_generics` top | `Item` | `rewrite_generics.rs` |
| 6713 | `expand_aliases` top | `Item` | `rewrite_alias.rs` |

**Critical clustering rule surfaced:** the **three `Item` walks** (collect@590, check_program@1990,
casing@2104) and the **three rewrite `Item` walks** (5946/6338/6713) are *independent* exhaustive
matches over `Item` in *six* different methods. Splitting them into different files is SAFE (they're
already separate methods) and actually *good* — it co-locates each `Item`-walk with its pass. The
`Expr`/`Stmt` exhaustive walks appear three times each (casing, the main check, and inside rewrites);
each stays whole in its own method. No match is interleaved with another.

---

## 4. Cross-cutting helpers → `mod.rs` (or a thin `common.rs`)

These are called from many clusters and should be centrally visible:

- **Diagnostic ctors** (`&mut self`): `err` (221), `err_coded` (229), `warn_coded` (244),
  `err_assign` (259), `err_opt_use` (4940). Used everywhere. → `mod.rs`.
- **Scope primitives** (`&mut self`/`&self`): `push_scope`/`pop_scope`/`declare`/`declare_binding`/
  `lookup`/`lookup_binding` (2462–2541), `in_scope_names`/`nearest_name` (276–302). → `mod.rs`.
- **Subtyping/assignability oracle:** `ty_assignable` (1391) wraps `Ty::assignable_with` (which lives
  in `src/types.rs`, NOT here — verified). `is_subtype` (1343) + `sig_conforms` (1331) +
  `iface_*`/`merge_inherited` are collect-pass-local → keep in `collect.rs`. The real subtype *engine*
  is `types.rs`; the checker only feeds it `class_implements`/`class_supertypes`. So there is **no
  "subtyping cluster" to extract** — it already lives in another file.
- **Stateless free helpers** (no `self`): `apply_subst`, `ty_has_param`, `levenshtein`, the 7 case
  fns (`is_camel`/`is_pascal`/`case_words`/`upper_first`/`to_camel`/`to_pascal`/`leaf_ident`),
  `is_intrinsic_name`, `is_builtin_type_name`, `is_true_lit`, `breaks_this_loop`, `match_arm_key`,
  `lambda_uses_this`. → `checker/common.rs` (pub(crate) within the module) or fold into `mod.rs`.
  `case_words`/`to_camel`/`to_pascal` are reused by `casing.rs`; `apply_subst`/`ty_has_param`/`unify`'s
  helpers by `calls.rs`. Keeping them in one `common.rs` avoids duplicate `use` churn.

**No `expand_aliases`/`erase_generics`/`resolve_html` belong in common** — they are top-level pipeline
passes (own files), not helpers.

---

## 5. Risk notes

1. **Private-field access is a non-issue *if* you split within one module.** The whole point of a
   `checker/` dir (vs. separate top-level modules) is that child `mod` files share the parent's private
   namespace. If a cluster were instead promoted to a *separate crate module* (`crate::checker_calls`),
   every field would need `pub(crate)` — DON'T do that. Keep everything under `mod checker { … }`.

2. **Multiple `impl Checker` blocks are legal and idiomatic** but the methods must keep their exact
   signatures and `self`-mutability. A mechanical cut (move the method text verbatim into a new
   `impl Checker { }` block in the sibling file) is behavior-preserving. Risk is purely
   transcription/`use`-import drift, caught instantly by `cargo build`.

3. **Order dependency between passes is real but lives in the *callers*, not the split.** `check`
   (5439) drives: `collect` → `check_interface_graph` → `inherit_class_members` → `check_casing` →
   `check_program`, then the pipeline calls `expand_aliases` (before backends) and `erase_generics`
   and `resolve_html` (post-check). This sequencing is encoded in `run_checker`/`check` and in
   `cli::check_and_expand` — splitting the method *bodies* into files does not change call order. Keep
   `run_checker`/`check` in `mod.rs` so the pass orchestration stays in one obvious place.

4. **`save/restore` of `cur_ret`/`cur_throws`/`cur_is_main`/`cur_class`/`active_type_params`/
   `cur_class_type_params` is interleaved** across `check_function`/`check_program`/`check_type_body`/
   `check_lambda`. These set-and-restore dances must NOT be broken apart mid-method. They're already
   self-contained per method, so file placement is safe — but a reviewer must confirm no helper that
   reads `cur_ret` is moved to a file where the set/restore site is unclear. Co-locate
   `check_function`/`check_program`/`check_type_body`/totality in `program.rs` (they share the
   `cur_ret` discipline).

5. **`html_resolutions` write-then-`resolve_html`-read split.** The checker *populates*
   `self.html_resolutions` (in `check_html`@3352 and throws-mode `?` erasure) and the free fn
   `resolve_html` *consumes* a `HashMap` passed in by `check_resolutions` (5453). The field and the
   consumer are already decoupled via the entry-point hand-off — `check_html` lands in `expr.rs`,
   `resolve_html` in `rewrite_html.rs`, and the bridge is `check_resolutions` in `mod.rs`. No risk,
   but note the producer/consumer live in different files post-split (intentional, mirrors current
   logical separation).

6. **Tests reference private items.** `mod tests` calls `prog`/`errors_of` and exercises behavior via
   the public `check`/`erase_generics`/`expand_aliases`. It does NOT poke private fields directly
   (verified: tests build programs and assert on `Diagnostic`s). Moving it to `checker/tests.rs` with
   `mod tests;` keeps it a child of `checker`, so any private access it *does* have is preserved.
   Lowest-risk extraction; do it first to shrink the file before touching the impl.

7. **`FnSig`/`EnumInfo`/`ClassInfo`/`HookInfo`/`InterfaceInfo`** (13–93) are private structs used by
   `collect.rs` (writers) and `calls.rs`/`expr.rs` (readers). Keep them in `mod.rs` so every sibling
   sees them without a `use super::` storm, OR put them in `common.rs`. They have no methods, so no
   coupling beyond field reads.

---

## Recommended execution order (lowest-risk-first, each step `cargo test` + PHP-oracle gate)

1. Extract `mod tests` → `checker/tests.rs` (−3018 lines, zero behavior).
2. Extract the 3 self-contained rewrites → `rewrite_html.rs`/`rewrite_generics.rs`/`rewrite_alias.rs`
   (−1084 lines, pure fns).
3. Extract stateless helpers → `common.rs` (−352).
4. Split the `impl Checker` whale into `resolve.rs`/`collect.rs`/`throws.rs`/`program.rs`/`casing.rs`/
   `stmt.rs`/`expr.rs`/`calls.rs`/`assign.rs`/`matches.rs`, leaving struct + entry fns + diagnostic/
   scope primitives in `mod.rs`.

After step 1+2+3, `checker.rs` (becoming `mod.rs`) is already down from 9786 → ~4900 with near-zero
risk. Step 4 is the cohesion payoff for navigation.

## Open design question to flag for the milestone

`calls.rs` (~810) is the largest impl cluster. It bundles named/overload/generic/native/method/member
resolution. It *could* sub-split into `calls.rs` (free/overload/generic/native) + `members.rs`
(method/member/construction/subst), ~400 each. Both touch `classes`/`enums`/`active_type_params`
identically, so it's a pure navigation call, not a coupling one. Recommend deferring that decision to
the by-construct-vs-by-phase question the milestone must answer.
