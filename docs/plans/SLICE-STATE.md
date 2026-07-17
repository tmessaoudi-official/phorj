# SLICE-STATE (live cursor — updated as work progresses; read FIRST after any compaction)

## CURRENT (2026-07-17)
- ✅ **DEC-191 #[Entry] COMMITTED `7ffd550e`** (328 files; detail in the in-flight section below,
  now historical). Release rebuilt after.
- **NOW: DEC-243 String.levenshtein + similarText** (inline; no adjudication needed — PHP-parity
  natives: match PHP's levenshtein()/similar_text() semantics EXACTLY incl. the similar_text
  percent-by-reference twin question — surface: `String.levenshtein(a, b): int` +
  `String.similarText(a, b): int` (+ percent variant? check PHP's API and pick the honest
  mapping — similar_text returns count, percent via &$percent → phorj likely
  `similarText(a,b): int` + `similarTextPercent(a,b): float`). Native module = Core.String
  (text.rs/text_registry.rs); PHP erasure = the builtins themselves (Tier-1!); bench vs PHP
  per DEC-259. Examples + FEATURES + README + register BUILT.
- THEN (upfront-adjudication batch at DEC-243 close): DEC-256 Unicode FULL surface ·
  DEC-242 partitioned-cookies surface · DEC-258 Db naming opt-in surface — then build those
  (batch-gate) → DEC-273 ext migration → lift Uri Tier-2 → golden corpus → span-collision
  re-basing slice → quiet-box microbench (owed pre-push).

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
  self-verifies the corpus on every suite run — it caught TooManyRedirects/TooLarge (missed by
  the initial map) + test/example fixtures (Boom-class fixtures → *Error) on the first gate
  runs; final sweep = 27 stdlib renames. ✅ COMMITTED `284284e0` (44 files, gate 2288/2288).
  **ENTIRE NAMING DOCTRINE (DEC-275…280) NOW LANDED.**
- **DEC-191 #[Entry] IN FLIGHT — PROGRESS (uncommitted, compiles clean, probe green):**
  ✅ (b1) ast/class_hierarchy.rs: `is_entry_attr` + `EntryRole{Cli,Web}` + `entry_role(f)`
     (AST-shape classification; CLI=():void|int|(List<string>):void|int, WEB=(Request):Response)
     + `entry_candidates(program)` + `entry_for(program, role)`. Old name-keyed `entry_point`
     KEPT for now (8 callers still on it — flip pending).
  ✅ (c1) checker/program/walk.rs: E-MULTIPLE-MAIN block REPLACED by the DEC-191 validation
     (bare-args E-ATTRIBUTE-ARGS · instance-method E-ENTRY-TARGET · no-role E-ENTRY-SIG w/
     shape list · per-role E-MULTIPLE-ENTRY; CLI+web may coexist).
  ✅ checker/program/attributes.rs: Entry known in the fn-attr whitelist (validation lives in
     walk.rs). PROBED: `#[Entry] function main(): void` checks + runs.
  ✅ (b2) ALL 8 callers FLIPPED to `entry_for(program, EntryRole::Cli)` (transpile ×4,
     compiler, interpreter ×2, loader, serve handlers' cli check); "no entry point" error
     texts now name `#[Entry]`; `synth_empty_main` carries the attribute (Span uses len not
     end!). PROBED: attributed entry runs; un-attributed magic `main` = clean no-entry error
     (FULLY BREAKING confirmed live).
  ⏳ REMAINING: serve Web-role resolution + respond_bridge rewire off name-magic "handle"
     (serve/handlers.rs + preludes respond_bridge — currently keys off `handle` by name);
     old `entry_point`/`entry_point_count` fns now likely dead → remove after codemod;
  ✅ throws.rs main-no-throws restriction REMOVED (DEC-191 ruling supersedes Batch-1 D;
     comment records the supersession).
  ✅ wp() (src/cli/tests.rs) + typed_program (tests/db.rs) now inject `#[Entry] ` before a bare
     `function main(` (replacen 1, skipped when already attributed) — covers most inline tests.
  ✅ CODEMOD DONE: 275 example/conformance .phg files attributed (column-0 regex + the indented
     static-main case for class-main.phg; differential GREEN post-codemod); compiler::tests
     with_pkg helper injects (30/31 pass; missing_main assertion flipped to expect #[Entry]);
     23 integration .rs files + tests/db.rs textually codemodded (`function main` →
     `#[Entry] function main`, existing-attr protected); explain entries E-ENTRY-SIG/
     E-ENTRY-TARGET/E-MULTIPLE-ENTRY added. Census r1 = 776 fails; census r2 RUNNING —
     remaining expected: entry_point.rs E-MULTIPLE-MAIN flips ×2, throws
     main_may_not_declare_throws (rule removed → flip/delete), run_executes_sample (SAMPLE
     const direct call), library_file error-text assertion, format pipe test?, playground
     runvm tests (its own fixtures), dap handshake fixture, vendor fixture, serve/handle
     name-magic rewire still pending + old entry_point fns removal + exit codes + docs.
  ✅ census r6 = **2291/2291 GREEN** (776→0 convergence). CLOSE-OUT DONE: respond bridge
     rewired to the ATTRIBUTED web entry (textual callee substitution into HTTP_RESPOND_BRIDGE;
     class-static paths supported); 7 handle fixtures attributed (user-attributes.phg was a
     FALSE POSITIVE — its handle isn't a web handler, attr removed); NAMED-ENTRY generalization:
     compiler program.rs ×4 sites (static-init preludes + index resolution — was panicking
     "entry_point reported a class-static main" on a non-main-named entry!), interpreter
     call_name ×2, transpiler bootstrap callee — all key on entry_decl.name now;
     guide/entry.phg (class-static named entry + int exit) THREE-LEG green incl. php-exit=0;
     docs done (CHANGELOG w/ span-collision disclosure, FEATURES row, README row, MASTER-PLAN
     SHIPPED note). Old name-keyed entry_point/entry_point_count kept (pub, unreferenced by
     backends — removal is cleanup for a later pass). FULL GATE running → commit + release.
  ✅ census r5→r6 fixes: mtest ×6 = test_runner synthesize_main now attributes its synthetic
     entry + strips #[Entry]-attributed fns (not name-main); format stdin = assertion restored
     to plain form (fmt must NEVER insert attributes; MESSY has double-space so codemod missed
     it — correct outcome); diagnostics goldens = attribute REVERTED in conformance/diagnostics/
     (check-only corpus, entries not needed, preserves golden line numbers); loader+dap fixtures
     codemodded. Census r6 RUNNING (expect ~0). THEN: serve web-role rewire (respond_bridge
     name-magic `handle` → EntryRole::Web), guide/entry.phg example + docs (CHANGELOG/FEATURES/
     register BUILT note incl. the DEC-191-ruling-supersedes-main-no-throws note), old
     entry_point/entry_point_count removal if dead, full gate (raw-verified clippys), commit.
  ⚠⚠ RESOLVED BUG (was census r4 residue, REPRODUCED + root-caused): examples/db/transaction-closure.phg —
     interpreter leg RUNS CLEAN, VM leg = "compile error: `transaction` is not a function,
     variant, or class" (run≠runvm divergence!). transaction = the DEC-249 default-param method
     (fills machinery). Appeared between 284284e0 (green) and the DEC-191 work. Suspects, in
     order: (1) apply_default_fills interplay with the reified chain rewrap I did for
     materialize_for_binds/lower_foreach_iter (re-nested parens in pipeline.rs — check the arg
     nesting is EXACTLY materialize_pipe_params(...inner..., &pipe_params) then
     materialize_for_binds(·, &for_binds) then lower_foreach_iter(·, &for_iters)); (2) the
     example has for-loops → for_bind_resolutions non-empty → materialize_for_binds mutates
     For.ty in place — check ty_to_ast_type output for Row/entity types is benign on the
     VM kind path; (3) fills+ufcs double-rewrite resurrection ([[rewrite-clone-staleness-class]]
     — READ IT). DEBUG PLAN: minimal repro = default-param METHOD call + a for-in loop with
     inferred binding + #[Entry] main; bisect by disabling materialize_for_binds (pass empty
     map) then lower_foreach_iter. Others FIXED in r4→r5: format stdin assertion must expect
     CANONICAL own-line `#[Entry]\nfunction main` (fmt splits the line — fix the assertion);
     diagnostics goldens: conformance/diagnostics/*.phg got a +1 LINE SHIFT from the attr
     insert — either same-line the attr in those files or bump golden line numbers; loader
     tests + dap.rs fixtures codemodded ✓; lifter now EMITS #[Entry] (synth + php-main) and
     the lift printer prints fn attrs (was dropping them) ✓; lift_roundtrip + all 6 mtest ✓.
  ✅ census r3 = 125 → codemodded src/jit/tests/*.rs (4 files, ~90 tests) + ALL remaining .phg
     under tests/+src/ (tests/fixtures/sample.phg, dump_fault.phg …). Census r4 RUNNING;
     expected residue = SEMANTIC flips (~20): entry_point E-MULTIPLE-MAIN ×2 → E-MULTIPLE-ENTRY;
     throws main_may_not_declare_throws → entries-may-throw; missing-main assertion texts
     (interpreter, run_integration program_without_main, transpile main_is_invoked, cli
     library_file + run_executes_sample/SAMPLE const); loader::tests ×2 (main-file exemption
     keyed on entry presence — now attribute-keyed); diagnostics golden case (one case pins an
     old code/message); mtest ×6 (the `phg test` runner path — check how it resolves/needs
     entries); format stdin case; dap handshake fixture; db transaction-closure example;
     lift_roundtrip; differential class_static_main_exit_code test (NOTE: an exit-code test
     EXISTS — read it before implementing (): int exit codes, semantics may partially exist!).
  ✅ census r2 = 157 fails → helper patches: src/interpreter/tests.rs with_pkg (injects),
     src/interpreter/coop.rs fixtures (textual), src/vm/{coop,tests}.rs (textual). Census r3
     RUNNING → iterate on its list (pattern: RUN-path fixture = add attr / helper-inject;
     check-only tests need NOTHING; assertion texts mentioning old messages get flipped;
     entry_point.rs E-MULTIPLE-MAIN tests + throws main_may_not_declare_throws = flip to the
     new semantics). NOTE skip-list: checker tests (check-only, no entry needed), doc comments
     (dap.rs/diagnostic.rs/lift decls/cli pipeline/bundle section), src/lsp/tests.rs
     (diagnostics path). jit tests pass untouched (own runner).
  ⏳ ORIGINAL grind list (superseded by above, kept for detail): (a) examples/**/*.phg + conformance/**/*.phg — insert
     `#[Entry]\n` line above top-level `function main(` (218+ files; python codemod; then
     playground `python3 playground/gen_examples.py` regen); (b) NON-wp test fixtures: raw
     consts (cli/tests.rs SAMPLE) + per-file harnesses in tests/*.rs (http_client, fs, session,
     mail, regex_and_more?, differential fixtures embedded) — run suite --no-fail-fast and fix
     every 'no entry point' failure by adding the attribute; (c) E-MULTIPLE-MAIN tests in
     checker/tests/entry_point.rs flip to E-MULTIPLE-ENTRY/#[Entry] forms; (d) remove dead
     `entry_point`/`entry_point_count` + their "main" literals once nothing references them;
     grep '"handle"' for serve name-magic (respond_bridge) → Web role. throws.rs
     `validate_throws_decl` `is_entry_main` — DEC-191 ruling WINS over old main-no-throws
     (throwing entries legal; escaped fault = exit 1/HTTP 500) → drop/replace the restriction;
     (): int exit codes (interp+VM map returned Int → process exit 0-255; PHP emits
     exit($code)); E-MULTIPLE-MAIN test flips in checker/tests/entry_point.rs; THE CODEMOD
     (examples 218 + test inline strings ~1000+: `function main(` → `#[Entry] function main(`
     top-level only — EXCLUDE instance-method-main fixtures + comment texts; conformance/;
     playground regen; synth_empty_main in ast/decls.rs may need the attr!); explain entries
     (E-ENTRY-SIG/E-ENTRY-TARGET/E-MULTIPLE-ENTRY); guide/entry.phg example; docs rows.
  (all gaps ruled — MASTER-PLAN §13.1.1: static entries YES /
  FULLY BREAKING no-main-fallback / (): int exit codes / web (Request): Response, CLI+web may
  coexist / throwing entries legal). SETTLED DESIGN:
  (a) The ruling kills the MAGIC NAME, not the name — programs keep `function main`, just
      attributed: `#[Entry] function main(): void`. Codemod = insert `#[Entry] ` before
      top-level/static `function main(` declarations (trivial diffs). Same for serve `handle`
      → web role (respond_bridge in preludes keys off name-magic today — rewire to attribute).
  (b) Resolver: current `ast::class_hierarchy::entry_point(program, name)` (name-keyed, already
      handles static methods) → new attribute-keyed `entry_points(program)` returning
      {cli, web} classified by signature; CLI = ():void | ():int | (List<string>):void|int,
      WEB = (Request):Response. Grep ALL callers of entry_point/"main"/"handle" literals
      (interpreter run, vm run_entry, compiler, cli serve, preludes respond_bridge,
      entry-main-no-throws rule in throws.rs validate_throws_decl `is_entry_main`!).
  (c) Checker validation pass (collect/attributes.rs): #[Entry] arg-less, only on top-level fns
      + static methods; signature must match a role else E-ENTRY-SIG (hint lists shapes);
      >1 per role = E-MULTIPLE-ENTRY; entries may throw (escaped fault = exit 1 / HTTP 500).
  (d) (): int exit codes: interpreter + VM map returned Int → process exit (0-255); PHP leg
      emits exit($code) wrapper around the entry call. `no entry point` error message updated.
  (e) Codemod scope: examples/**.phg (~200, top-level main = safe blanket), tests' embedded
      programs (~1000+ inline strings — regex `function main\(` → `#[Entry] function main(`
      per file EXCEPT instance-method-main fixtures in entry_point.rs tests + explain/doc
      texts); conformance/; playground gen_examples regen; docs snippets FEATURES/README.
  (f) Docs+example (guide/entry.phg: named CLI entry w/ int exit + args; web coexist note),
      explain entries, editors: NO grammar change (#[...] exists).
  After DEC-191: DEC-256 Unicode FULL · DEC-243 levenshtein · DEC-242 cookies · DEC-258 Db
  naming (batch-gate candidates) · lift Uri Tier-2 · golden-corpus harness · quiet-box
  microbench (owed).
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
