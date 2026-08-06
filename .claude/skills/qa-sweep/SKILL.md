---
name: qa-sweep
spotlight: true
description: Exhaustive end-to-end QA on the SHIPPED phg binary and the surfaces cargo test never touches — the real CLI on real files, the LSP over real stdio JSON-RPC, the package-manager lifecycle in a scratch project, the editor integrations, and the playground's rendered output. Use before a release, after any change to the CLI/LSP/editor/playground surface, or when a defect is reported that the test suites do not reproduce. Not a substitute for the differential suite; it answers "does the product work?" rather than "do the legs agree?".
user-invocable: true
args: "[--only <substr>] [--skip-build] [--journey <name>] [--no-playground] [--keep-scratch]"
disallowed-tools: AskUserQuestion
---

<!-- ═══════════════════════════════════════════════════════════════════════════════════
  phorj CONTAINER ADAPTATION — WRITTEN, NOT PORTED (2026-08-06, second cross-repo bundle round).
  pdfturbo's `/qa-sweep` drives a browser over a PDF editor with axe-core and CSP workarounds; none
  of that has an analogue here, so copying it would have produced a skill about machinery this repo
  does not own. What DOES transfer is the premise, and it is the whole reason this skill exists:

      every one of this repo's worst defects was invisible to the suite that was supposed to catch it.

  Precedents, all real: a `html"…"` literal inside a tuple reached the compiler and hit
  `unreachable!()` on VALID user code; the playground's VM pane compiled through a path that skipped
  `check_and_expand_reified`, hiding a VM≠tree-walker divergence off the differential's CLI route
  (now Invariant 6); `SLICE-STATE` sat stale by a full wave with four BUILT features recorded as
  "build queued"; a test asserted on a disclosure COMMENT instead of the emitted artefact and passed
  green. `cargo nextest` links the library and drives it in-process — it does not run the binary a
  user installs, does not speak JSON-RPC to the language server, does not open an editor, and does
  not render the playground.

  The canonical container deltas 1-7 (see any sibling skill in `.claude/skills/`) all apply. The two
  that bite hardest here:
    • ≤5 CONCURRENT SUBAGENTS, and any agent that must persist a file is `general-purpose`, never
      `Explore` (which cannot Write). Raw output to `var/claude/qa-sweep/<stamp>/raw/` BEFORE
      returning — autocompact does not preserve conversation results, only disk files.
    • REPORTS GO TO `var/claude/qa-sweep/<stamp>/` (gitignored via `/var`), never `~/.claude/…`,
      which is wiped when the container is reclaimed. Never `git add` one.
  And delta 6: this skill's output is a reply, so it ends with `⏹ NO QUESTION — …` or `❓ QUESTION — …`.
═══════════════════════════════════════════════════════════════════════════════════════════════ -->

## --help

> If `$ARGUMENTS` contains `--help`: print the block below verbatim, then STOP — `--help` takes
> precedence over every other flag.
>
> ```
> /qa-sweep — drive the SHIPPED phg binary and the non-cargo-test surfaces, and report what breaks.
>
> Usage: /qa-sweep [--only <substr>] [--journey <name>] [--skip-build]
>                  [--no-playground] [--keep-scratch]
>
>   --only <substr>    restrict to journeys whose name contains <substr>
>   --journey <name>   run exactly one journey (see the table in the skill body)
>   --skip-build       reuse the existing target/release/phg instead of rebuilding
>   --no-playground    skip the wasm/rendered-surface journey
>   --keep-scratch     leave the scratch project on disk for inspection
> ```

---

# /qa-sweep

## Ground rules

1. **Drive `target/release/phg`, not the library.** The whole point is the artefact a user gets. Build
   it first (`cargo build --release`) unless `--skip-build`, and report the path and version you
   actually exercised (`./target/release/phg --version`) — a sweep of a stale binary is a false green.
2. **Work in a scratch directory under the scratchpad**, never in the repo tree. Anything a journey
   creates is removed at the end unless `--keep-scratch`.
3. **No network.** Invariant 10: `run`/`check`/`transpile` never touch the network. The
   package-manager journey therefore exercises only the *local* paths and asserts the network verbs
   fail cleanly offline rather than hanging — it does not reach a registry.
4. **A journey that cannot run is reported as SKIPPED with the reason.** Never silently drop one, and
   never report a sweep as clean while a journey was skipped — that is the DEC-365 NO-HIDDEN-LOSS
   discipline applied to QA.
5. **Verify every negative with a control.** Before reporting "no errors on stderr", plant a known
   failure and confirm your check catches it. A probe that cannot fail launders a live defect into a
   documented non-finding.

## The journeys

Each row is a real, runnable surface. Run them in order; later ones assume the binary from journey 0.

| # | name | what it covers that `cargo test` does not |
|---|---|---|
| 0 | `binary` | `cargo build --release` clean, `phg --version`, `phg --help` lists every verb the docs claim, and every verb answers `<verb> --help` without panicking. A verb documented but absent — or present but undocumented — is a finding. Note `phg vendor` is **retired and must ERROR** (DEC-282), and there is **no `runvm`**: the VM is `run`'s default engine, `--tree-walker` the oracle. |
| 1 | `spine` | The three legs on the SHIPPED binary over `examples/**/*.phg`: `phg run` ≡ `phg run --tree-walker` ≡ `phg transpile` under the pinned php from `scripts/toolchain.env` (**PHP 8.5** floor — never gate against the bare `php` on PATH, which is 8.6-dev and too permissive). Also `phg run --no-jit`, which must be byte-identical with no rebuild. This duplicates `tests/differential.rs` *deliberately*: it re-runs it through the installed artefact. |
| 2 | `lsp` | The language server over **real stdio JSON-RPC**, not handler calls. Frame `initialize` → `initialized` → a document open → the request under test → `shutdown` → `exit`. Assert the advertised capabilities against Invariant 17's 100% RULE. **Verified 2026-08-06 by live handshake:** `completionProvider`, `hoverProvider`, `definitionProvider`, `referencesProvider`, `documentSymbolProvider`, `documentHighlightProvider`, `renameProvider`, `documentFormattingProvider` are advertised (**eight** — `documentHighlightProvider` was missing from the first version of this list, which would have made a later sweep read a long-standing capability as newly appeared); `signatureHelpProvider`, `codeActionProvider`, `semanticTokensProvider` and `inlayHintProvider` are **NOT**. Those four are known standing gaps — report them as such, not as new findings, and DO flag any new call-site or diagnostic surface that ships without them. Exit codes are conformant (`shutdown`+`exit` → 0; a cold stdin close → 1); do not report that as a defect. |
| 3 | `diagnostics` | `phg check` ≡ LSP diagnostics (DEC-252, same pipeline, never diverging). Take a file with a real error, run `phg check`, then ask the LSP for diagnostics on the same bytes, and compare code, message and range. A divergence here is a P0 — it is the invariant, not a nicety. Also `phg explain <CODE>` for every code the run emitted. |
| 4 | `project` | The package-manager lifecycle in a scratch project: `phg add` / `install` / `update` / `remove` (all four verified present 2026-08-06; there is **no `phg init`**), then `phg build`, `phg run`, `phg test`. Offline, so assert clean refusals rather than successes on the network paths. |
| 5 | `lift` | `phg lift` on a real PHP tree, then `phg check` the draft it produced, then `phg transpile` back. Invariant 17 requires transpile AND lift to move in the same change, and this is the only place the round trip is driven end to end. |
| 6 | `format` | `phg format --check` over `examples` and `selftest`, plus idempotency: format twice, expect byte-identical output. Then `phg format -` on stdin. |
| 7 | `editors` | The two integrations, which nothing else exercises: `editors/vscode/` and `editors/phpstorm/` (the LSP4IJ path). Check the TextMate/syntax grammars actually cover the syntax the compiler accepts — take a construct added recently and confirm it is in the grammar. **Be honest about the ceiling:** VS Code and a JetBrains IDE cannot be launched in this container, so this journey verifies configuration and grammar coverage, NOT live editor behaviour. Say so in the report; a config-only check reported as "editors verified" is exactly the over-claim Invariant 17 exists to prevent. |
| 8 | `playground` | The one **rendered** surface (`playground/`, wasm + `playground/web/`). Per the Completion Gate's visual-evidence clause, passing tests are not sufficient Coverage evidence for a rendered surface: capture the actual output. Critically, check the VM pane still compiles through `check_and_expand_reified` + `compile_with` — **Invariant 6 exists because it once did not**, and a plain `compile` there hides a VM≠tree-walker divergence off the differential's CLI path. If `wasm-pack` is unavailable, SKIP with that reason. |
| 9 | `gates` | The scripts the git hooks own, run directly: `scripts/size-gate.sh`, `scripts/doc-guards.sh`, `scripts/validate-infra.sh`, `scripts/microbench-gate.sh`, `scripts/perf-gate.sh`. These are the repo's own tripwires; a sweep that does not run them is checking less than `git push` does. |

## Interpretation — the part that is your job, not the commands'

A command's exit code is the cheapest signal in the sweep and the least informative. For each journey
ask:

- **Did the failure reproduce off the harness?** A defect that only appears through the shipped binary
  is the highest-value output of this skill, because it is invisible to every gate the repo has.
- **Is this a surface left behind, or a real break?** Invariant 17's failure mode is "the compiler
  knows it but the editor doesn't". Classify accordingly — an unimplemented LSP capability is a
  completeness finding, a *wrong* diagnostic range is a correctness one.
- **Did anything panic?** EV-7: every bad input is a clean fault, never a panic. A panic reached from
  valid user input is a P0 regardless of which journey found it.
- **Did an output depend on iteration order?** Invariant 10: any user-facing list derived from a
  `HashMap`/`HashSet` must be sorted. Run the journey twice and diff.

## Certify before reporting

Per `CLAUDE.md` § "Certification ladder" (DEC-268 MAXIMAL): spawn the three lenses —
`backend-parity-reviewer`, `safety-promises-reviewer`, `completeness-reviewer` — **by name via the
Agent tool, in ONE message** so they run concurrently on independent contexts. Give them the report
and the raw output; they read the artefacts themselves. TWO consecutive fully-clean rounds; any
finding resets the counter; cap 5 rounds, then ask in plain text. If no reviewer is available,
self-grade and **disclose that it was self-graded**.

## Report format

Write to `var/claude/qa-sweep/<stamp>/report.md`, with raw per-journey output under `raw/`.

```
# qa-sweep <stamp>

Binary: target/release/phg <version>   (built | reused)
Journeys: N run, M skipped

| # | journey | verdict | note |
|---|---------|---------|------|
| 0 | binary  | PASS    | 18 verbs, all answer --help |
| 8 | playground | SKIPPED | wasm-pack not installed in this container |

## Findings
P0/P1/P2/P3, each with: the journey, the exact command, its output, and why it is wrong.

## What this sweep could NOT verify
Enumerate it. A sweep silent about its blind spots reads as more complete than it is.
```

Findings go through `/aggregate-findings` if another review skill ran in the same session, so a defect
found twice is reported once.
