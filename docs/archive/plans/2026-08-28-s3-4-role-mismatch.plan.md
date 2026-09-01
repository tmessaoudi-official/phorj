# PLAN — S3.4 role-mismatch UX (`E-NO-ENTRY-FOR-ROLE`), DEC-331 D6/P3

> **Status: SHIPPED (2026-08-28), DEC-455.15.** Slice S3.4 of DEC-331. Predecessors S3.1, S3.2 A/B/C and
> S3.3 a–e are all shipped; S3.5 (inbound TLS via rustls) stays after this one, per SLICE-STATE's
> own sequencing. Spec: `docs/specs/2026-07-23-entry-kinds-serve-tls.md` §D6.

## 1. What is ruled, and what is being built

Spec §D6, verbatim in substance: `phg run` on a Web-only program (or `phg serve` on a Cli-only one)
emits `E-NO-ENTRY-FOR-ROLE` naming the mismatch and the right command, THEN a TTY-guarded
interactive *"Did you mean `phg serve <file>`? [y/N]"* which runs it on `y`; non-TTY (CI/pipe) gets
the error plus the suggestion, a non-zero exit, and **never blocks on stdin**. Symmetric both
directions.

**Today** [Verified 2026-08-28, `grep -rn 'E-NO-ENTRY-FOR-ROLE' src/` → zero hits]:

| verb | program | current behaviour |
|---|---|---|
| `phg run` | Web-only | `"no entry point: running needs an #[Entry(kind: EntryKind.Cli)] function (DEC-331). A library or web file still type-checks and transpiles — use phg check / phg transpile"` — **no code, no mention of `phg serve`, and no awareness that a Web entry is right there.** Three emit sites: `src/interpreter/mod.rs:295`, `src/compiler/program.rs:136`, `src/interpreter/coop.rs:157` |
| `phg serve` | Cli-only | `"serve needs an #[Entry(kind: EntryKind.Web)] function that calls Http.serve(cfg, handler)"`, code `E-SERVE-NO-HANDLER` (`src/serve/web_handlers.rs:84`) — **no mention of `phg run`** |

Neither distinguishes *"you have no entry"* from *"you used the wrong verb"*, which is the whole
point of D6.

## 2. Architecture

New cohesive module **`src/cli/role_mismatch.rs`** (Invariant 13 — new file, well under the 300 soft
cap), holding four things and no I/O:

- `pub const E_NO_ENTRY_FOR_ROLE: &str = "E-NO-ENTRY-FOR-ROLE";` — a named const, not an inline
  literal, so `scripts/surface-ratchet.sh` can see it. An inline literal is how `E-CONCURRENCY-NO-PHP`
  stayed invisible to the ratchets for releases (`web_handlers.rs:40`'s own warning).
- `pub fn detect(program, wanted: EntryRole) -> Option<Mismatch>` — `Some` **iff** the wanted role is
  absent AND the other role is present. A genuine library (neither role) returns `None` and keeps
  today's *"no entry point"* / `E-SERVE-NO-HANDLER` message untouched.
- `pub fn message(&Mismatch) -> String` — the rendered diagnostic. Pure.
- `pub fn guard(program, wanted) -> Result<(), String>` — what the pipeline calls.

**Where the guard fires — the correctness-critical choice.** In the `cli::` layer, **not** in
`main.rs`. *(Amended during the build: this section first named three functions. The shipped guard is
at EIGHT run entry points via `pipeline::run_guard` — `cmd_treewalk`, `cmd_run`, `cmd_treewalk_exit`,
`cmd_run_exit` and the four `Unit`-based `{treewalk,run}_program{,_exit}` — plus `prepare_serve` for
the Web half. Three was too few: `cmd_run`/`cmd_treewalk` are the string-source helpers the tests
actually drive and are NOT wrappers over the `_exit` pair, so guarding only the latter would have left
the tests passing against unguarded code. There is no single chokepoint before the checker:
`parse_checked` and `check_and_expand{,_reified}` are shared with `check`, `transpile` and
`benchmark`, where a web-only program is legal and must not be refused.)* The test suite never executes `main.rs` (`src/cli/tests/*` call `cli::` functions
directly), so a guard living only there could not be sabotage-tested: deleting the wiring would leave
every test green, which fails this repo's own *prove the gate RUNS* rule. Those three functions are
exactly what `cmd_run`/`cmd_treewalk` and `serve_pipeline_tests` already drive.

`main.rs` therefore keeps only the **prompt**: it matches `E_NO_ENTRY_FOR_ROLE` in the returned
(rendered) error string — stringly, but that is how codes already travel here — and on a TTY offers
the switch.

## 3. Decisions Log

- [2026-08-28 15:10] AGREED: build S3.4 next (developer, `AskUserQuestion`) — it is what SLICE-STATE's
  cursor names, its shape is already ruled by spec §D6, and it needs no new dependency.
- [2026-08-28 15:40] AGREED: **the guard runs BEFORE `check`**, so a program that is both
  role-mismatched and type-broken reports the mismatch first. Defensible — the verb is wrong
  regardless of the type errors, and the check output is verb-independent — and inverting the order
  would mean checking twice on every `phg run`. Pinned by a test on the combination.
- [2026-08-28 15:40] AGREED: **the prompt shows exactly what it runs.** It offers `phg serve <file>`
  with no flags, so on `y` serve runs with DEFAULTS. Consequences that must be honoured rather than
  inherited: `run` sets `Profile::Dev` (`main.rs:508`) whereas a real `phg serve` defaults to
  `Release`, and the real serve branch calls `set_stdin_disabled()` (DEC-281). The switched call goes
  through ONE shared preamble used by both the real serve branch and the switch — hand-reassembling
  it at the switch site would create a second resolution path that drifts, the same hazard this repo
  documents for `toolchain.env` vs CI.
- [2026-08-28 15:40] AGREED: **the prompt is offered only for a plain `.phg` file source.**
  `phg run` cannot take a directory at all (`loader::load` → `read_file(entry)`, no `is_dir` branch
  [Verified]), while `phg serve <dir>` site-resolves to `<dir>/public/index.phg`. So a serve→run
  suggestion naming the *directory* would propose a command that cannot run. Directory, `-e` and stdin
  sources therefore get the diagnostic with NO prompt.
  *(Amended to what shipped: this line first said a directory source would get "the coded error naming
  the RESOLVED entry file". It does not — `message()` names no file on any path, because the target is
  the caller's concern and threading it into the message would have re-coupled the pure rule to the
  argv shape. The prompt suppression is the whole behaviour; the diagnostic is identical for every
  source.)*
- [2026-08-28 15:40] AGREED: a reserved-kind-only program (`kind: Desktop`) must NOT trigger the
  prompt — `entry_declared_role` is `Active`-only, so `detect` returns `None` and the program falls
  through to `E-ENTRY-KIND-RESERVED`. Pinned by a test rather than left to the type.
- [2026-08-28 15:40] AGREED: exit codes — a non-TTY mismatch and a declined (`n`) prompt exit **1**;
  exit 2 stays reserved for argv/usage failures. A `y` switch propagates the switched verb's own
  exit code.
- [2026-08-28 15:40] AGREED: the prompt writes to **stderr**, not stdout — stdout belongs to the
  program's `Output.*` (DEC-220, the same reasoning as `serve_program`'s startup notices).

## 4. Build order

1. **Red first.** `src/cli/tests/` cases for both directions (message contains the code, the wanted
   kind, the found kind, and the other verb), the library-with-no-entry non-regression, the
   reserved-kind non-trigger, and the mismatch-plus-type-errors ordering. Confirm each fails **for the
   stated reason**, not incidentally.
2. `src/cli/role_mismatch.rs` + the three pipeline call sites.
3. `main.rs` prompt wiring + the shared serve preamble.
4. `phg explain E-NO-ENTRY-FOR-ROLE` (`src/cli/explain/names_types.rs`) — **mandatory**: the explain
   ratchet fails the build for a code with no entry. Assert it in `explain_coverage.rs`.
5. `phg run --help` / `phg serve --help` lines (`src/cli/help.rs`).
6. **Sabotage check**: delete the guard call from `run_program_exit` → the suite must go RED. Restore
   and verify byte-for-byte.
7. `bash scripts/surface-ratchet.sh --emit` (`codes_total` 307→308, `codes_asserted` 252→253) and say
   so in the commit message.

## 5. Invariants this slice must satisfy

- **Invariant 1** — no example changes shape; the mismatch is unreachable for any program that has
  the entry it needs. The differential corpus is untouched.
- **Invariant 9** — a fault cannot be a runnable example; captured in `examples/web/README.md`
  instead, per the invariant's own carve-out. No new `.phg` ⇒ no `phg format` sweep exposure.
- **Invariant 17** — `phg explain` entry same-change. The diagnostic is CLI-runtime, NOT a `phg check`
  diagnostic, so it has no LSP surface — to be VERIFIED (that the LSP pipeline cannot produce it),
  not assumed. No new syntax ⇒ the TextMate grammars and both editors are a verified no-op.
- **Invariant 19** — SLICE-STATE cursor, this plan, the spec's stale line ~49 (*"`E-NO-ENTRY-FOR-ROLE`
  has 0 src hits"*, false once this lands) and the decision register updated in the same change.

## 6. Not in this slice — stated, not skipped

`phg benchmark`, `phg test` and library embedders calling `interpret_main` on a Web-only program keep
today's backend message. [Verified 2026-08-28 by running both: `phg benchmark web_only.phg` still
prints `no entry point: …` — it goes through `parse_checked`, which is deliberately unguarded — and
`phg test` fails earlier still, in its own `<check>`.] D6 rules `run` and `serve`; widening it further would put a CLI-shaped
diagnostic inside the backends, which is where it least belongs. S3.5 (inbound TLS via rustls,
feature-gated `http-server-tls`) remains separate and last.

## 7. What shipped, and what §4 changed on contact

Built as planned, with two amendments the build forced:

- **`main.rs` had to shrink, not just avoid growing.** §2 assumed the prompt wiring would fit inside
  the 12 lines of headroom under its 622-line baseline. It did not (634 at the first attempt, then 640
  with the switch inline). Two moves fixed it, both of which the Invariant-13 ratchet wanted anyway:
  the switch decision went into `role_mismatch.rs` behind `switch_run_to_serve`/`switch_serve_to_run`
  (returning the outcome, so `std::process::exit` stays out of the library — nothing under `src/`
  calls it today), and the 140-line `serve` argv branch moved to a new `src/cli/serve_cli.rs`.
  **`main.rs` ended at 496 and left `scripts/size-baseline.txt` entirely.** The move was verified
  faithful by diffing the extracted body against `HEAD:src/main.rs`: exactly four deltas, all
  intended.
- **`serve_preamble` came out of that move, not out of §3.** The plan called for ONE shared preamble;
  the extraction is what made it natural, since the real serve branch and the switch now sit in the
  same module.

**A defect in my own design, caught at 6C and fixed by ordering.** §3 ruled ONE shared preamble so a
run→serve switch binds what real `phg serve` binds — and never mirrored it. `serve_preamble` calls
`set_stdin_disabled()`, which `src/native/input.rs:49` offers with **no inverse**, and pins the profile
to `Release`. A serve→run switch taken afterwards would therefore have run the user's CLI program with
stdin already dead (`Input.readLine()` → `null` instead of the line they typed) and faults rendered
under the wrong profile — breaking, in the other direction, the exact promise `serve_preamble` exists
to keep. Because the flag cannot be unset, **ordering is the only available fix**: `serve_cli` now runs
the role guard BEFORE any serve process setup, and `switch_serve_to_run` sets the `Dev` profile a real
`phg run` sets. `prepare_serve` keeps its own guard — it is the invariant for every other caller, and
the reason the earlier one is a UX ordering rather than a duplicate. **[Verified on a pty: a CLI
program reading `Input.readLine()` prints `read: hello` identically under a real `phg run` and under
the switched path.]** Pinned by `serve_preamble_disables_stdin_which_is_why_the_role_guard_must_run_first`,
which guards the PREMISE — the day the flag stops being one-way, the comment justifying the ordering
becomes false and the test goes red.

**Certified by execution** — driven on a real pty, all four paths: `n` → exit 1; bare Enter → exit 1;
`y` on `phg run` → serve bound the program's own `ServeConfig` port (42911, not 8080), which is the
shared preamble proving itself; `y` on `phg serve` → the CLI program ran and exited 0. Non-TTY
verified to print, exit 1 and never read stdin. **Nothing in this slice is
`UNCERTIFIED-BY-EXECUTION`** — the `is_terminal()` branch that a unit test cannot reach was exercised
directly.

**Sabotage-verified twice**, each mutation valid code: no-op'ing `pipeline::run_guard` reds the two
run-direction tests; deleting the `prepare_serve` guard reds the serve-direction one, and its failure
message shows `E-SERVE-NO-HANDLER` returning — i.e. the exact regression the guard exists to prevent.
Both restores checksum-verified byte-for-byte.

**Verified, not assumed (Invariant 17).** The LSP has no surface for this code: `diagnostics_for` is
`lex → parse → front_end_diagnostics` and the LSP never calls a run/serve verb (`grep` → 0 hits), so
`phg check` ≡ LSP is preserved precisely because neither emits it. Both editors are a verified no-op —
the TextMate grammar carries no diagnostic codes at all (`grep -c "E-"` → 0) and no new syntax landed.
Transpile and lift are untouched: this is a CLI verb diagnostic, not a language feature.

## 8. Found in passing, NOT fixed here

`phg test` on any program importing an injected prelude module fails its `<check>` pseudo-test with
spurious `E-UNKNOWN-IDENT`s, because `src/cli/test_runner.rs:90` calls the RAW
`checker::check_tests` — the same hole DEC-252 closed for the LSP and never closed here.
**Pre-existing** [Verified: identical output from the release binary built at `6145a64d`]. Recorded
as `KNOWN_ISSUES.md` §TEST-RAW-CHECKER rather than fixed, because the fix needs the test-mode flag
threaded through the front end and is its own slice.

