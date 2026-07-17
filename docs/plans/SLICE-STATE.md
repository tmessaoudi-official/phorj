# SLICE-STATE (live cursor — updated as work progresses; read FIRST after any compaction)

> Location developer-ruled 2026-07-16: lives IN THE REPO (tracked), committed alongside each
> slice commit. High-churn detail stays here so MASTER-PLAN §0.2 stays clean.

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
  - ✅ SLICE 2 CORE BUILT + PROBED (all uncommitted): for_iter_lowerings HashSet field
    (mod.rs/plumbing.rs; check_resolutions tuple 7→8, both pipeline.rs destructures fixed);
    iterator_elem helper + check_for arm (flow.rs — throws rule = covered_by_try OR
    throws_declared union w/ targeted E-CALL-UNHANDLED message; NOTE discharge_call_throw alone
    was WRONG: bare-call discharge is try-only in Phorj's model); rewrite_foreach.rs (stmt
    walker + span-keyed For→Block{VarDecl __for_it_<start>; While(hasNext){VarDecl x=next();
    body}} lowering; lambda block bodies via rewrite_pipe::walk::visit_exprs_mut; idempotent);
    wired OUTERMOST in check_and_expand_reified. PROBES ALL THREE-LEG-IDENTICAL: basic foreach
    3-2-1 · interface-typed param (total(Iterator<int>)) · nested iterator-in-iterator+list ·
    throwing iterator declared/caught (declared=3 caught=3) · undeclared = clean loop-site
    error. Bare `Iterator<int>` type annotation needs `import Core.Iterator.Iterator;`
    (E-INJECTED-TYPE-BARE — the X.X shape DEC-278 addresses).
  - ✅ SLICE 2 FINISHERS DONE: 3 cli tests pass (foreach_over_* — implementor+nested+
    interface-typed / throwing declare-or-catch / non-iterator error); throws.rs destructure
    8-tuple fixed; guide example examples/guide/iterators.phg THREE-LEG-IDENTICAL (incl. the
    Iterator<string?> nullable-element proof + manual pulls); docs done (CHANGELOG slice-2,
    FEATURES row, examples/README row, MASTER-PLAN 16b, UNIFIED-SPEC stdlib block).
  - ✅ SLICE 2 COMMITTED `a9e9f693` (+ naming rulings docs `59ce8bb3`).
  - ✅ SLICE 3 BUILT (uncommitted, gate running): RowStream/DbStream implement Iterator —
    lookahead `mutable Row? ahead` in RowStream.hasNext (pull+cache, carries throws), next =
    cache or `panic("iterator exhausted")` (needs `import Core.Abort.panic;` in DB_PRELUDE);
    DbStream.hasNext delegates (NO hydration — laziness exact), next = rows.next()? + hydrate.
    ⚠ GOTCHAS hit: (a) REGISTRY ROW ORDER — Core.Iterator's row must sit AFTER Core.Db's (the
    injection fold resolves transitive prelude imports in row order; comment at the row);
    (b) `x != null` is NOT phorj (cross-type comparison error) — use `if (var v = opt)`;
    (c) bare throwing calls inside throwing prelude methods need `?` AS WHOLE BINDING INIT
    (`bool has = this.hasNext()?;` — never in if-condition position);
    (d) `panic` diverges for totality ✓ but needs `import Core.Abort.panic;`.
    MIGRATED: 4 tests/db.rs bodies → foreach/direct-next + NEW exhausted-fault pin test
    (80/80 db tests pass); examples/db/streaming.phg → foreach (both backends identical);
    docs (CHANGELOG slice-3, examples/README row, UNIFIED-SPEC stream line, MASTER-PLAN
    "DEC-257 COMPLETE").
  - ✅ SLICE 3 COMMITTED `05f224a7` — **DEC-257 COMPLETE**; release binary rebuilt.
- **NAMING MEGA-SLICE (DEC-276…279 renames)** — ✅ agent done (112 files; its gate 2284/2284 +
  clippys + fmt + release in the worktree), diff cherry-picked onto master (1 conflict:
  FEATURES.md, resolved — kept DEC-280 foreach row + renamed Iterator row). Dev RATIFIED
  E-IMPORT-NATIVE-MEMBER (whole-module-only raw natives) + REJECTED old→new hint table
  ("do nothing — all migrated"); register amended, CHANGELOG entries written. Agent follow-ups
  recorded: HcResult/MailResult renames · enforce_injected 3-segment-import edge · editors
  docs/snippets unchecked · UriModule.Uri.parse double-chain (already ruled follow-up).
  ⚠ agent snapshot commit `1234bdac` lives on branch worktree-agent-a3b9403d94752528a (worktree
  removal is permission-blocked — clean up manually later; second stale worktree
  agent-af41f1445fc1c9498 likewise). ✅ COMMITTED `8bae400f` (117 files, gate 2286/2286).
- **DEC-275 E-ERROR-NAME (inline, uncommitted, gate running):** rule at collect (transitive
  class_implements ⇒ name must end Error|Exception), explain entry, 2 checker tests (incl.
  subclass-of-error-base), stdlib sweep codemod = 25 renames (Mail: AuthFailed/ConnectionFailed/
  InvalidAddress/MailIo/MailTimeout/MessageBuildFailed/RecipientRejected; Http: BlockedAddress/
  HttpConnectionFailed/HttpTimeout/InvalidUrl; Db: ConstraintViolation/SerializationFailure/
  Timeout/UniqueViolation; Uri: UriMalformed + UriBad* family + UriBaseNotAbsolute/
  UriPortOutOfRange — all stem+Error; sentinels <<X>> renamed in lockstep, 30 files). The rule
  self-verifies the corpus on every suite run. On green: commit + release rebuild → NEXT = DEC-191
  #[Entry] (gaps ruled; codemod-driven breaking migration).
- **LIFT CATCH-UP + DEC-280 (inline, uncommitted, gate running):** DEC-280 RULED+BUILT
  (untyped/mixed foreach k=>v; developer challenged→confirmed; lift marker inline comment form).
  Landed: parser bare/mixed bindings (parse_foreach — dropped both mandatory-type errors);
  **materialize_for_binds** (rewrite_foreach.rs; Invariant-7: inferred foreach binding types →
  AST post-check, BOTH forms — single-binding had the same latent CTy gap; wired BEFORE
  lower_foreach_iter; check_resolutions tuple 8→9, pipeline+throws.rs updated;
  rewrite_pipe::materialize now pub(in checker) for ty_to_ast_type); format printer two-binding
  arm (foreach spelling when any binding Infer; fully-typed keeps `for (K k, V v in m)`); lift:
  PhpMember::Prop.set_vis + (set)-group parsing + DEC-241 modifier mapping + lift printer
  PrivateSet/ProtectedSet ORDER entries (was silently dropping!) + k=>v Tier-1 with inline
  marker + two-binding print arm (was silently dropping val!). Tests: foreach_untyped_* cli
  test (v+0 arithmetic proves materialization), lifts_key_foreach_with_inferred_marker,
  lifts_asymmetric_visibility_properties (flipped refuses_key_foreach). Example:
  examples/guide/foreach.phg extended (v*2 differential pin, format-fixpoint, 3-leg identical).
  Docs: CHANGELOG (DEC-280+lift), FEATURES foreach row (new), C-decisions DEC-280 ruled+BUILT.
  NOW: full gate in bg → on green commit → review naming agent when it returns.
    ORIGINAL slice-2 analysis below kept for reference:
    (a) Checker field `for_iter_lowerings: HashMap<usize, ()>` (keyed Stmt::For span.start) +
        thread through check_resolutions return tuple (grows 7→8: update BOTH pipeline.rs
        destructures + checker/tests/throws.rs).
    (b) Helper `iterator_elem(&self, name, cargs) -> Option<(Ty, Vec<Ty>)>` (elem + the union
        of concrete hasNext/next throws): name=="Iterator" → (cargs[0], vec![]) (interface
        throws = empty by existing deferral); else classes[name].iface_args.get("Iterator") →
        elem = apply_subst(args[0], class_subst(name, cargs)); throws from
        ci.methods["hasNext"/"next"][0].throws.
    (c) check_for single-binding match: add `Ty::Named(..)` guard arm BEFORE `other =>` when
        iterator_elem hits: record span in for_iter_lowerings; for each throw type E call
        `self.discharge_call_throw("next", &E, *span)` (KEY SIMPLIFICATION [Verified: read
        throws.rs 43-80]: `?` is a CHECKER-ONLY marker — runtime unwind identical — so the
        REWRITE EMITS BARE CALLS, no Propagate wrapping; discharge_call_throw gives exact ruled
        semantics: caught-by-enclosing-try OR fn-declares OR clean error).
    (d) NEW rewrite_foreach.rs: recursive stmt walker (model: rewrite_pipe/walk.rs vstmt —
        must cover fn bodies, class members incl. ctor, lambda block bodies, all nested stmts).
        `Stmt::For{span in map}` → `Stmt::Block([ VarDecl{ty: Infer, name: "__for_it_{start}",
        init: iter}, While{cond: Call(__for_it.hasNext()), body: [VarDecl{ty: for's ty, name,
        init: Call(__for_it.next())}, ...body]} ])` — unique var per loop start = nested-loop
        safe. Recurse INTO the moved body (nested foreach-over-iterator).
    (e) Wire into cli/pipeline.rs BOTH check_and_expand AND check_and_expand_reified
        (invariant 6) — order: after apply_default_fills/other expr rewrites? Foreach lowering
        is stmt-level + independent of expr rewrites; run it LAST (after materialize_pipe_params
        order concerns don't apply — but its generated calls must survive: rewrite_ufcs etc.
        already ran, and our generated hasNext/next calls are plain method calls needing NO
        further rewriting on any backend).
    (f) Docs: exhausted-next() fault contract note; examples/guide/iterators.phg (Countdown +
        foreach + null-element note); checker tests (foreach over implementor; throws
        undeclared = error; declared = clean; inside try/catch = clean; foreach over
        Iterator<E>-typed value; non-implementor still errors); CHANGELOG/FEATURES/
        examples-README/MASTER-PLAN/UNIFIED-SPEC.
    Then SLICE 3: Db streams reshape (hasNext/next + implements Iterator<Row>/<T>, lookahead
    buffer; migrate desugar_db sites, examples/db/*, tests/db.rs; RowStream throws move to
    hasNext — it pulls).
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
0a. **NAMING MEGA-SLICE (DEC-275…279, all RULED 2026-07-16 — register has full detail):**
   error suffix Error|Exception + E-ERROR-NAME (stdlib sweep keeps stems) · earned-shortcut
   renames (Fs→FileSystem, Db→Database+family, Reflect→Reflection, DI→DependencyInjection,
   HcHandle→HttpClientHandle, --addr/--proto flags) · *Sys → Core.Native.* nesting ·
   7 namesake modules → *Module suffix (incl. IteratorModule; double-chained static = follow-up)
   · Core.Url merges into Uri. ONE codemod + differential sweep + docs/examples/editors.
   SEQUENCED right after DEC-257 (files overlap slices 2-3 → not truly independent; also avoids
   double-renaming the Db streams). Dev-kept-earned list in DEC-276 (Math, dd, lsp, acronyms).
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
