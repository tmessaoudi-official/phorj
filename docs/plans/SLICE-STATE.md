# SLICE-STATE (live scratchpad — update as work progresses; read FIRST after any compaction)

Updated: 2026-07-16 (evening)

## In flight
- **DEC-257 Iterator slice 1 (generic interfaces)** — INLINE, uncommitted:
  - DONE: `InterfaceDecl.type_params` + `ClassDecl.implements_args` AST fields;
    parser `interface I<T>` (bounds rejected loudly) + `parse_implements_list`
    (`implements Iterator<int>`) wired into class parser.
  - DONE (compiles clean): all 11 construction sites fixed; InterfaceInfo.type_params +
    placeholder(arity) prebind; collect_interface resolves sigs w/ active_type_params (Ty::Param);
    resolve.rs generic-interface args (arity-checked E-TYPE-ARG-COUNT); conformance loop
    substitutes implements_args via theta+apply_subst before sig_conforms (also resolves args
    with the CLASS's type params active, so `DbStream<T> implements Iterator<T>` works);
    rewrite_generics gained the Item::Interface erasure arm (rparam/rty over method sigs).
  - PROBED GREEN: `interface Producer<T>` + `class Ints implements Producer<int>` checks+runs;
    wrong ret = E-IFACE-SIG; missing args = E-TYPE-ARG-COUNT w/ hint; `class Boxed<T> implements
    Producer<T>` THREE-LEG byte-identical (run/tree-walker/PHP all `42`). Scratch probes in
    session scratchpad (giface*.phg). NOTE: `new Boxed<int>(42)` turbofish-on-new NOT supported
    (parse error — construction infers args; only List/Map have new-with-args per DEC-214p1).
  - MORE DONE: ClassInfo.iface_args (HashMap<iface, Vec<Ty>>; populated in the conformance loop
    where args are already resolved w/ class tps active); ty_assignable gained the
    class→parameterized-interface invariant-args check (inherit.rs, BEFORE assignable_with;
    inherited-implements = documented fall-through to name path); class_subst falls back to
    INTERFACE type_params so interface-typed receivers substitute (`p.produce(): int` not `T`).
    PROBED: `Producer<int> good = new Ints()` + `consume(good)` clean; `Producer<string> bad =
    new Ints()` REJECTED. Fast test tier running in bg.
  - DONE: 5 checker tests in src/checker/tests/interfaces.rs (all pass); fast tier 2208/2208;
    FORMAT-FIDELITY BUG found+fixed (printer dropped `<T>` on interface + implements args —
    format/printer/items.rs: interface() generics + implements_body() helper at both class
    sites; lift printer needs nothing, PHP has no generics); guide example
    examples/guide/generic-interfaces.phg three-leg-verified (final canonicalized content);
    docs done (CHANGELOG slice-1 entry, FEATURES row, examples/README row, MASTER-PLAN item 16).
  - SLICE 1 ✅ COMMITTED `54255480` (full gate: 2274/2274, clippys 0+0, FMT-OK).
- **SLICE 2 IN FLIGHT (uncommitted):** DONE so far: ITERATOR_PRELUDE (`interface Iterator<T>
  { hasNext(): bool; next(): T; }`) + CORE_MODULES row (member_gated, bare_types ["Iterator"],
  before the Uri row) + injection fold now merges Item::Interface (was `_ => false`, silently
  dropped!) + InterfaceDecl.injected flag (mirrors EnumDecl; parser/collapse/alias/generics
  ctors updated) + DEC-202 builtin-name check EXEMPTS injected interfaces (entry.rs) + PHP-leg
  mangle `Iterator` → `Iterator_` in transpile/names.rs php_class_name (RoundingMode precedent;
  emit_interface disp now routes php_class_name; implements already routed php_type_ref).
  PROBED: Countdown implements Iterator<int> + manual hasNext/next pull = THREE-LEG-IDENTICAL
  (3 2 1). ⚠ transpiled output is NOT namespaced (my earlier namespace assumption was wrong —
  DEC-202's "cannot redeclare" empirically confirmed; hence the mangle).
  - REMAINING in slice 2: foreach-over-Iterator desugar — check_for arm (iter_ty = class
    implementing Iterator<E> via ClassInfo.iface_args + class_subst, or Ty::Named("Iterator",
    [E]) direct) recording span → new stmt-level rewrite (rewrite_foreach.rs; walker modeled on
    rewrite_pipe/walk.rs vstmt) lowering `for (T x in it)` →
    `{ var __it = it; while (__it.hasNext()) { T x = __it.next(); body } }`, with
    Expr::Propagate wrapping on hasNext/next calls when the CONCRETE methods throw (ruled
    auto-propagate; check_for must also discharge those throws against the enclosing fn);
    wire the rewrite through BOTH check_and_expand and check_and_expand_reified (invariant 6);
    exhausted-next() fault contract note in docs; examples/guide/iterators.phg; checker tests;
    CHANGELOG/FEATURES/README/MASTER-PLAN/UNIFIED-SPEC rows. Then SLICE 3: Db streams reshape
    (hasNext/next + implements Iterator<Row>/<T>, lookahead buffer; migrate desugar_db sites,
    examples/db/*, tests/db.rs; RowStream throws move to hasNext — it pulls).
  - Annotation note: `Iterator<int>` in type position survives to backends WITH args exactly like
    `Box<int>` does (backends already cope; rty keeps heads + recurses args). No new erasure
    needed for annotations.
  - Then slice 2 (Core.Iterator prelude + foreach stmt-desugar) + slice 3 (Db stream reshape).
    Full map = memory [[dec-257-iterator-build-map]].
- **Playground rework** — ✅ COMMITTED (`feat(playground): two-pane…` right after `6eb07c91`):
  agent diff reviewed + applied on master, README de-staled, node --check clean, CHANGELOG entry.
  ⚠ leftover: agent worktree `.claude/worktrees/agent-af41f1445fc1c9498` + its branch could not
  be removed (permission-denied on `git worktree remove --force`/`branch -D`) — ask dev or clean
  later; changes are fully applied+committed on master. ⚠ runtime smoke test in a real browser
  OWED (org policy blocked localhost browsing for the agent): `python3 -m http.server -d
  playground/web` + check tabs/badge; wasm pkg + php-wasm paths untested at runtime.

## Queue after DEC-257
0b. **LIFT CATCH-UP slice (Invariant-17 debt, dev asked 2026-07-16 "are they always up to date?"):**
   (a) lift PHP 8.4 `private(set)`/`protected(set)` → DEC-241 modifiers; (b) upgrade
   `foreach ($m as $k => $v)` from Tier-2-reject to Tier-1 (Phorj has k=>v since DEC-248 —
   stale comment at lift/lifter/decls.rs:355); (c) Uri Tier-2 mapping (already-recorded
   follow-up). Batch-gate candidate; transpile confirmed always-current (differential-gated).
1. **DEC-191 #[Entry]** — brought forward, gaps RULED (see MASTER-PLAN §13.1.1 update):
   static methods YES; FULLY BREAKING (no main fallback; codemod + differential sweep);
   `(): int` exit codes; web `(Request): Response` confirmed; CLI+web may coexist.
2. DEC-256 Unicode FULL · DEC-243 levenshtein+similarText · DEC-242 cookies · DEC-258 Db naming
   (batch-gate candidates; upfront-adjudicate their surface questions first).
3. DEC-273 ext migration AFTER queue. Owed: quiet-box microbench rerun pre-push; golden-corpus
   harness build; playground-agent review.

## Standing (new today)
- Speed levers authorized = memory [[speed-levers-authorized]] (worktree agents for independent
  slices OK; NEVER dynamic workflows/team agents).
