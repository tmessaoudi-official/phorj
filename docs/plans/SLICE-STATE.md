# SLICE-STATE (live cursor — updated as work progresses; read FIRST after any compaction)

## ▶ CURRENT CURSOR (2026-08-29) — **S3.5 SHIPPED. DEC-331 SLICE 3 IS CLOSED.**

**DEC-331 D7 built: `phg serve` terminates TLS**, feature-gated `http-server-tls`. HTTPS enables iff
BOTH `cert` and `key` are set on the registered `ServeConfig` — no `--tls` flag, no switch;
`tlsMinVersion` (`"1.2"` default, `"1.3"`) is the floor. **All five of Slice 3 are now built**
(D1/D4/D5/D6/D7).

**It added NO crate.** rustls 0.23 has no client/server feature split, so the outbound HTTP client's
existing feature set already compiled `ServerConfig`/`ServerConnection`/`StreamOwned`. PEM decoding is
hand-rolled (`src/serve/pem.rs`, ~85 lines) rather than admitting a fifteenth dependency. If someone
later proposes a PEM crate: the decoder is tested to yield FEWER blocks on malformed input, never a
wrong one, which is the only property `tls::build` relies on.

**Six things that will bite whoever touches this next:**
- **A lone `cert` is an ERROR, not plain HTTP.** D7's surface says "iff BOTH are set", and the literal
  reading is a silent downgrade to clear text on a port the operator believes is encrypted. The
  refusal is the ruling (`E-SERVE-TLS-INCOMPLETE`); do not "fix" it back toward the spec text.
- **`TlsServer` is an UNINHABITED enum without the feature** — not a struct that is never built. That
  is what makes "a non-TLS build cannot serve a TLS-configured program in the clear" a fact about the
  types; `Conn::accept` discharges the branch with `match *never {}`. Turning it into a struct would
  silently delete the guarantee while every test still passed.
- **Config errors outrank build errors**, pinned by a test: a lone `cert` on a feature-off build
  reports `-INCOMPLETE`, not `-DISABLED`, because the config is wrong however phg was compiled.
- **The stream is wrapped only AFTER blocking mode + timeouts are set on the raw `TcpStream`.** rustls
  fails outright on a non-blocking socket, and running the handshake through those same timeouts is
  what bounds a TLS-level slowloris. Both accept paths already did this; do not hoist the wrap.
- **The handshake happens in the WORKER, never the accept loop.** `StreamOwned` drives it on first
  read, so a stalled client cannot serialize `accept()` and starve the pool.
- **TLS is read directly from the config, NOT through `settings::resolve`.** That function is the
  flag-vs-config PRECEDENCE rule and D7 rules there is no TLS flag — one source, no precedence. It
  would also give `ServeSettings` (which derives `PartialEq, Eq`) a field whose type has neither.

`src/serve/transport.rs` fell 635 → 455 lines and **left `scripts/size-baseline.txt`** (the ratchet
tightens): the wire framing moved to `src/serve/framing.rs`, which is where it belonged — every
function there is generic over `Read` or pure over `&[u8]`.

**Deferred BY RULING, not oversight** (KNOWN_ISSUES §SERVE-TLS): HTTP→HTTPS redirect, HSTS,
certificate hot-reload, mTLS. Cert paths resolve against the process cwd, not the site-mode app root;
passphrase-protected keys are not supported.

### ⊳ CURSOR 2026-09-02 (evening) — panel round 3 in-repo, the readiness wave is the next body of work

**Read first:** `docs/plans/2026-09-02-php-parity-readiness.plan.md` (the delta against three real PHP
apps + cross-language + PHP ecosystem, the ruled ORDER, the questions queue) and
`docs/plans/2026-08-31-post-slice3-consolidation.plan.md` § "Panel round 3" (35 findings transcribed
in-repo, disposition per finding) + its Decisions Log (the 2026-09-02 rulings).

**Gate: CLOSED (DEC-490, 2026-09-03) on the round-4 fixes** — the readiness wave is the active body of
work; OWED at this close: the parity-% recompute (M-gap-matrix §4). History: panel round 3 ran on frozen `6a18f71a` over `0c982019..6a18f71a`: correctness 12
(1×P0 `default_fills` collision reproduced on 3 legs), completeness 16 (3×P1), safety 7. DEC-481
(fix-then-verify) ruled the closing rule: every P0/P1 fixed, freeze, ONE round. **Round 4 ran on
`bfafdd23`** (correctness 8, safety 8, completeness 13 — see the consolidation plan § "Panel round
4"): its P1s are fixed in the commits after it; whether they close the gate or a round 5 is owed is
the developer's call, asked at the end of step 1.

**ORDER (ruled 17:05):** (1) harness trust — panel disposition (C6 `default_fills` P0 FIXED `53df9ef1`+`1e62f74a`; the three ungated PHP-emit paths C7/C8/F1 FIXED `06e9e975`; the LSP/check test-mode path DONE (DEC-486); the differential floor K4 DONE; DEC-459 prelude isolation BUILT; C10/F4 malformed Content-Length FIXED; the regex cluster C1–C5/C11 BUILT with REGEX-B (DEC-461, C4 deferred); the docs pass DONE (K2/K3/K5/K6/K9/K10/K14/K15, MASTER-PLAN §0.07 mirror rows); next: FREEZE and run the ONE panel round of DEC-481 over `0c982019..HEAD`); (2) readiness wave in
leverage order — **charset BUILT 2026-09-04** (DEC-494/495 — both legs hand-rolled from one table, no crate, no ini extension); `String.foldAccents` BUILT too (step 6b, DEC-496), so DEC-468 is fully closed; `Time.sleep` BUILT (DEC-487, ladder DEC-497) and `Runtime.onShutdown` + `isShuttingDown` BUILT (DEC-204/498, step 9); next = time zones (DEC-466 + DEC-499 — the PHP leg is RULED: emit the pinned table, `Zone.of` takes a literal; the crate-vs-generate half is still open, measurements in the readiness plan's Needs research); (3) DEC-333 perf. **Register rows: DEC-460 … DEC-499** (`C-decisions.md`, the
2026-09-02 readiness rulings — 486 LSP/check test mode, 487 `Time.sleep`, 488 the Q22 split, 489 the small stdlib rows); MASTER-PLAN's §0 cursor is stale relative to this block until step 1's
docs pass — this block + the two plan files' Decisions Logs are the record meanwhile (Invariant 19
pointer, not a fork).

**The "next" block at the bottom of the 2026-09-02 cursor below is SUPERSEDED by this one** where
they differ (its `phg test` line predates the CD-31 fix; see panel K3).

### ⊳ ADDENDUM 2026-09-02 — the milestone panel RAN, and the gate is still OPEN

The 3-lens milestone panel that S3.5's plan §5 declared due was run 2026-08-31 against the FROZEN
commit `cf6875db` `feat(serve): HTTPS that refuses rather than falls back — inbound TLS (S3.5, DEC-331
D7)`, over the range `0c982019..cf6875db`. That discharges the obligation to RUN it. It does not close
the milestone.

- **security + safety-promises: CLEAN** — every promise checked against diff, source and EXECUTED
  tests (10/10 default-feature TLS refusals; 21/21 with `http-server-tls`, a real handshake included;
  no committed secrets; the `unsafe` island untouched).
- **completeness + blast-radius: FINDINGS — 8** (2×P1, 2×P2, 4×P3).
- **correctness + regression: FINDINGS — 3** (1×P1 — a live Invariant-1 spine break — and 2×P2).

**Gate status: OPEN, clean counter at zero.** DEC-268 wants two consecutive fully-clean rounds; this
was round one, with findings. Slice 3 must NOT be reported as panel-certified until the developer's
chosen closing procedure completes — and that choice is theirs, because DEC-268's two-clean rule and
the 2026-08-19 economize ruling (one panel per milestone; a second is the waste the rule exists to
prevent) genuinely conflict here. Do not resolve that alone.

The P1 spine break is FIXED (2026-09-02): `Core.Native.Http.registerServe` bypassed
`E-TRANSPILE-SERVE` and emitted PHP that fatalled at runtime. The doc findings are fixed too. Range
disclosure: the panel read only `0c982019..cf6875db`, so S3.2 Parts A/B were reviewed at HEAD state
but their diffs were never panel-read.

**G-8 remains OWED** and was re-confirmed unmeasurable on 2026-09-02: the pre-push microbench-gate
found 1-min load 10.59, waited 90s, still 4.36 against its 2.5 ceiling, and SKIPPED rather than
reporting a number. That is NO-HIDDEN-LOSS (DEC-365) working — never `--emit` a re-baseline to green
it. It needs a quiet box, gated on per-core `mpstat` idle rather than load-avg.

Full plan, ordering and the queued adjudications:
`docs/plans/2026-08-31-post-slice3-consolidation.plan.md`.

### ⊳ CURSOR UPDATE 2026-09-02 — post-consolidation state

**Slice 3's milestone gate is still OPEN.** A 3-lens panel ran against frozen `d182cd45` and returned
**24 findings** across the three lenses (1×P0, 5×P1, rest P2/P3). The P0 — a `Core.Native.Http` ladder
bypass reachable with NO import, via a leaked prelude alias — is FIXED and verified end-to-end. The
false claims it exposed are corrected in place. **The remaining findings are NOT yet worked**, and the
gate must not be reported as closed until they are.

**CLOSED since the last cursor:** KNOWN_ISSUES §TEST-RAW-CHECKER is **FIXED** — `phg test` shares the
front end (injected-prelude types resolve), the item-level desugars descend into `Item::Test` (CD-31),
and since DEC-486 `phg check` / `check --json` / the LSP check a document that declares a `test` item in
test mode, so `check ≡ LSP ≡ test` holds; `run`/`transpile`/`build` stay strict.

**RULED 2026-09-02 — build state per item** (Invariant 19: a ruled-but-unbuilt item belongs in the cursor,
not only the register):
- **DEC-457** — generic `#[Config]` providers key on the REIFIED type (`Map<string,string>` vs
  `Map<string,int>` become distinct injection keys).
- **DEC-458** — the `Core.Database` PHP twin is a `__phorj_db_stmt` wrapper
  `[PDOStatement, sql, params[], nextIndex]`. Unblocks case-1 step 2; step 3 (the `decimal` mapping)
  still needs its own ruling.
- **DEC-459** — BUILT 2026-09-02 (alias isolation at injection; F6 closed with it). Was: its own slice;
  adjacent to §span-collision. It is also the structural cure for the P0 above, whose current fix is a
  containment arm.

### ⊳ CURSOR UPDATE 2026-09-02 (later) — the panel's `Item::Test` finding was the tip of CD-31

Working the panel's *"`resolve_variant_imports`/`desugar_router` skip `Item::Test`"* finding widened
into a class. DEC-356 made `Expr`/`Stmt`/`Pattern` walks exhaustive and its ratchet lists the six
extracted `*_walk.rs` files — so the identical defect survived one level up, in the ITEM walks of
their parent files. **Five live defects, all verified end-to-end against the release binary with a
class control proving the asymmetry** (`CD-31` + addendum carry the table):

| shape | before |
|---|---|
| `html"…"` in a trait method | check clean → both backends `unreachable!`, exit 101 |
| `html"…"` in a **field initializer** | check clean → both backends `unreachable!`, exit 101 |
| UFCS in a trait method | check clean → backend `unknown field` |
| `inject<T>()` in a trait method | check clean → `unreachable!("inject() not expanded")`, exit 101 |
| generic method in a trait | **INVARIANT 1 BROKEN** — natives print `7`, PHP dies `TypeError: must be of type U`, exit 255 |

**The root cause in one line:** a trait's members are a full `Vec<ClassMember>` whose bodies EXECUTE
(they flatten into the using class), and every item walk that omitted `Item::Trait` was skipping
executable code while reading as though it were skipping a declaration.

`item_leaves!()` now joins the three macros in `src/ast/leaves.rs` — **`Import` and `TypeAlias` only**,
because `Interface` and `Enum` both carry `Expr` (`Param.default`, `Attribute.args`,
`variants[].backing_value`). Nine item walks carry explicit arms, and the DEC-356 ratchet was widened
to cover the eight parent files — it immediately caught two more `_ => {}` collection loops.

**Two behaviours left UNCHANGED on purpose, now visible as named arms** (Invariant 15 — a
user-visible change is the developer's call): a `#[Route]` static declared in a trait still does not
register, and `resolve_variant_imports` still does not collect `TypeAlias` names into its collision
set. Both are open questions, recorded in CD-31.

**What this did NOT close** (stated so it is never read as covered): no item-level pass walks param
defaults or attribute arguments — not for `Function` or `Class` either — so a rewrite needed inside
`function f(int n = <expr>)` or `#[Attr(<expr>)]` is missed uniformly. That is DEC-356 FOLLOW-UP B's
territory (one shared total visitor), not a drive-by widening.

**Still owed from the panel, NOT started:** the LSP non-test path, the playground's ungated PHP-emit
path, and the differential's missing floor assertion.

**Next: DEC-331 is done — the queue is open.** Two items are carried OWED and are the natural
candidates: the **G-8 microbench ratchet** (skipped since S3.4; needs a quiet box) and
**KNOWN_ISSUES §TEST-RAW-CHECKER** — FIXED (see the cursor above; DEC-486 closed the last gap).

---

## CURSOR (2026-08-28) — **S3.4 SHIPPED. The wrong verb now says so.**

**DEC-331 D6 built: `E-NO-ENTRY-FOR-ROLE`, symmetric.** `phg run` on a program whose only entry is
`kind: EntryKind.Web` — and `phg serve` on a `kind: EntryKind.Cli` one — no longer reports a bare
absence. It names the role that is missing, the role that IS declared, and the verb that would have
worked; on an interactive terminal it then offers to run that verb, defaulting to NO.

**`src/cli/role_mismatch.rs` is the whole rule, and it is pure.** TTY-ness and the answer are the
caller's, the way `serve::settings::resolve` takes its `cores` — so the ruling is exercised by the
suite rather than by a human at a prompt.

**Four things that will bite whoever touches this next:**
- **The guard is at each RUN VERB, not at a chokepoint, and that is deliberate.** The obvious shared
  steps — `parse_checked`, `check_and_expand{,_reified}` — are also `check`'s, `transpile`'s and
  `benchmark`'s, where a web-only program is perfectly legal. `pipeline::run_guard` is called from the
  nine run paths; `prepare_serve` carries the Web half.
- **It runs BEFORE the check.** A program that is both role-mismatched and type-broken reports the
  mismatch: the verb is wrong regardless, and the user was not trying to run this program at all.
  Pinned by `the_role_mismatch_is_reported_before_type_errors`.
- **`detect` fires only when the OTHER role is present.** A program with neither role is a library and
  keeps `no entry point` / `E-SERVE-NO-HANDLER`; a reserved kind (`Desktop`…) declares no active role
  and keeps `E-ENTRY-KIND-RESERVED`. Both pinned — the second only because `entry_declared_role` is
  `Active`-only, which a later widening could silently undo.
- **The prompt shows exactly what it runs**, so accepting goes through `cli::serve_with_defaults` →
  the SHARED `serve_preamble`. A hand-assembled preamble at the switch site would silently inherit
  `phg run`'s `Dev` profile and serve stack traces from a command typed as `serve`.
- **The serve→run direction is fixed by ORDERING, and it has to be.** `serve_preamble` disables stdin
  process-wide and `src/native/input.rs` has NO inverse, so a switch taken after it would run a CLI
  program with stdin dead. `serve_cli` therefore runs the role guard BEFORE any serve setup;
  `prepare_serve` keeps its own guard as the invariant for other callers, which is why that one never
  fires on this path. Do not "simplify" by deleting either.

`main.rs` fell from 622 to 496 lines and **left `scripts/size-baseline.txt` entirely** (the ratchet
tightens): the 140-line `serve` argv branch moved to `src/cli/serve_cli.rs`, which is where the
switch's sibling wiring belongs anyway.

**[Certified by execution both ways on a real pty]** — `n`, bare Enter, `y`→serve (which bound the
program's own `ServeConfig` port, not 8080), and `y`→run. Non-TTY never reads stdin. Sabotage-verified
twice: no-op'ing `run_guard` and deleting the `prepare_serve` guard each turn the suite red; restore
verified byte-for-byte by checksum.

**Next: S3.5** — inbound TLS via rustls, feature-gated `http-server-tls`. It is the last of DEC-331
Slice 3.

---

## CURSOR (2026-08-23) — **S3.2 Part C SHIPPED. `ServeConfig` finally binds the socket.** ⚠ SUPERSEDED by the cursor above

**DEC-455.14 — developer-ruled: the CLI flag wins, but LOUDLY.** Until this slice the registered
config was INERT: `serve_register::config()` carried `#[expect(dead_code)]` and had no caller, so
`Http.serve(new ServeConfig(port: 3000), h)` still bound 8080.

**The rule.** The config is the DEFAULT source for the four settings the loop binds — `host`+`port`,
`workers`, `timeout`. A flag that was PASSED and whose value DIFFERS wins, after one
`W-SERVE-CONFIG-OVERRIDDEN` line per field on stderr. A flag that merely RESTATES the config prints
nothing.

**Three things that will bite whoever touches this next:**
- **Ordering is load-bearing.** The config can only be read AFTER `web_*_factory` — its startup
  validation run is what executes the `Web` entry and populates the global — and that is still before
  any socket binds. Reading it earlier always sees `None`, i.e. the config silently never applies.
- **Provenance is approximated by VALUE.** A constructed object carries none, so a field is "set" iff
  it differs from D4's class default (`settings::class_defaults`, pinned against the prelude SOURCE).
  `new ServeConfig(timeout: 0)` therefore cannot express *no timeout*; `--timeout 0` can.
  KNOWN_ISSUES §SERVE-CONFIG-PROVENANCE — the real fix is a nullable D4 field set, its own Invariant
  15 question.
- **Do NOT read the config unconditionally.** D4 declares `timeout = 0` while `phg serve` defaults to
  30s, so that would have silently disabled the B4 idle-socket guard for every existing server.
  `an_all_default_config_is_indistinguishable_from_no_config` is the pin.

**Scope is those four fields and no more:** `cert`/`key`/`tlsMinVersion` await D7 (inbound TLS is
unbuilt — `rustls` is linked only by the outbound http-client), `maxBodySize` belongs to the wire
parser, `serverName` has no consumer. Wiring a field whose reader does not exist is a config that
still does nothing.

Resolution is a PURE function (`src/serve/settings.rs`, `cores` injected) — 9 tests, red-first
against a stub reproducing the ignore-the-config behaviour, sabotage-verified twice.
**[Verified end-to-end on a real socket both ways.]** `src/cli/serve_pipeline.rs` was split out
because the wiring pushed grandfathered `pipeline.rs` past its size-baseline row.

**Next: S3.4.** *(Done — 2026-08-28, see the current cursor.)*

---

---

## Older cursors

Everything before the cursors above lives in
[`docs/archive/plans/SLICE-STATE-ARCHIVE.md`](../archive/plans/SLICE-STATE-ARCHIVE.md) — about two
dozen dated cursor, session and handoff blocks, moved there 2026-09-02 so this file is the live
cursor and nothing else. Nothing was deleted. The Json-ADT JIT build plan that used to sit among them
is live design and was promoted to [`json-adt-jit.plan.md`](json-adt-jit.plan.md).
