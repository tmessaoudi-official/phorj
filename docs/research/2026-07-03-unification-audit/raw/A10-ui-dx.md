# A10 — UI / DX / UX surface audit (phg CLI, LSP, formatter, debugger, playground)

Auditor: batch-2 agent A10 · Date: 2026-07-03 · HEAD: `0691228` (clean tree)
Binary: `target/release/phg` (phg 0.5.1-alpha.1, rebuilt fresh — 0 crates recompiled).
Method: every claim below was exercised against the real binary or grepped in source; grades per
Rule 18. Certification note: this subagent has no `advisor()` tool — the pre-completion check is
**self-graded** (three lenses: completeness vs the 8-task brief, worst-failure-mode, blast radius),
disclosed per the Phase 3C/6C subagent carve-out.

---

## 1. CLI command inventory — 18 dispatched, 16 documented, and the two help surfaces disagree

**[Verified: ran `phg --help`, `phg frobnicate`, and read src/main.rs:73-75]**

- Dispatch (`src/main.rs:73-75`) accepts **18** verbs: run, runvm, check, parse, tokenize,
  transpile, lift, disassemble, benchmark, build, vendor, serve, **lsp**, test, format, explain,
  **debug** (+ `-h`/`-v`).
- Long `--help` lists **16** — `debug` and `lsp` are absent. Confirms the sibling (A4) claim
  independently.
- **The terse usage line contradicts the long help**: on any bad invocation
  (`phg frobnicate`, `phg run` with no source, `phg check --nope f.phg`) the one-line usage
  string DOES list all 18 verbs *including* `lsp|debug`. So `phg`'s two help surfaces disagree
  with each other about the command set — a user who mistypes learns about `debug`; a user who
  reads `--help` never does.
- `phg debug --help` and `phg lsp --help` print the **generic** top-level help and exit 0 — the
  only two verbs with no per-command help at all. Every other verb has real, dedicated help.

**Dead docs verbs confirmed** [Verified: ran each]: `phg fmt`, `phg lex`, `phg disasm`,
`phg bench` all exit 2 with the generic usage line — no such verbs, no aliases, and no
"did you mean `format`?" suggestion. Docs still instructing them: see §8.

## 2. Per-subcommand help quality

**[Verified: captured `--help` for all 16 documented verbs]**

Positives: every documented verb has a consistent template (one-line summary, `usage:`,
optional `flags:`, `examples:`). `serve`, `format`, `test`, `vendor`, `lift` are genuinely
excellent — they explain semantics (worker model, idempotency contract, project-root discovery,
offline guarantee, review-required draft) not just syntax. Flag naming is consistent across
verbs: no `--out` vs `--output` collision anywhere; `-o` exists only on `build`; `--check` only
on `format`; `--json` on `check`.

**Finding 2a — the first example a new user copy-pastes is broken.**
[Verified: ran the `run --help` example verbatim]
```
$ phg run -e 'function main() -> void { Output.printLine("hi"); }'
type error at 1:1: … [E-NO-PACKAGE] …
type error at 1:27: unknown identifier `Output` [E-UNKNOWN-IDENT]
```
The working form needs `package Main; import Core.Output;` prepended
([Verified: that variant prints `hi`, exit 0]). The same broken shape appears in `runvm --help`
and `disassemble --help` examples (`src/cli/mod.rs:77,85,129`). This is the single worst UX
papercut found: the very first thing the help teaches produces two errors.

**Finding 2b — the same three help examples use the purged `->` return syntax** (`function
main() -> void`), i.e. the exact syntax Phase-1/P1-remainder is removing. Today they'd fail
anyway (2a); after the parser-reject flips they become syntactically rejected examples. The
serve help + the top-level help line also use arrow prose: `respond(bytes) -> bytes`
(`src/cli/mod.rs:52,201,638`, `src/main.rs:301`). Canonical Phorj signature prose would be
`respond(bytes data): bytes`. [Verified: grep + P1 plan in `docs/plans/wave0-remainder.plan.md`]

**Finding 2c — undocumented flags** [Verified: grep src/main.rs:137-420 vs all captured help]:
- `run`/`runvm` `--dump-on-fault` (main.rs:416) — in no help text (0 hits in the captured help
  corpus); referenced only in MASTER-PLAN.md:890.
- `benchmark --json` (main.rs:412) — benchmark help documents only `--vs-php`.
- `build --dev` (main.rs, sets Dev profile) and `build --sign` (stub: prints
  `signing is Phase 3`, exit 2) — build help documents only `-o/--target/--all`. A stub flag
  that exits 2 with a one-liner is fine as a placeholder, but it is discoverable only by
  reading source or specs.

## 3. Error-message UX — 7 scenarios probed

**[Verified: all outputs captured live]**

| Scenario | Output | vs "PHP-familiar, exact info + one exact fix" bar |
|---|---|---|
| Missing file (`phg run does-not-exist.phg`) | `cannot read does-not-exist.phg: No such file or directory (os error 2)`, exit 1 | Clear, but `(os error 2)` is Rust errno leakage — cosmetic. |
| Syntax error (`int x = ;`) | `parse error at 4:11: expected an expression, found Semicolon` + source line + caret, exit 1 | **Meets the bar.** Exact position, caret. No code though (see below). |
| Type error (`int x = "hello"`) | `type error at 4:3: expected `int`, found `string`` + caret, exit 1 | Meets the bar for content — but **no `[E-…]` code**. |
| Missing package + unknown ident | Both diagnostics carry codes; `E-NO-PACKAGE` has an exact-fix hint (`add \`package Main;\``) | **Exemplary** — this is the M-DX bar. |
| Unknown subcommand | one-line generic usage, exit 2 | Weakest: doesn't echo the offending word, no suggestion. |
| Missing required arg (`phg run`) | same generic usage, exit 2 | Same. |
| Unknown flag (`--nope`) | same generic usage, exit 2 | Same — doesn't name the flag. |

**No raw Rust panic or backtrace was produced by any probe.** [Verified: all 7 above +
formatter/debugger/DAP runs]

**Finding 3a — diagnostic codes are inconsistently attached.** Common type errors
(`expected int, found string`) carry no `[E-…]` code, so (i) a user can't `phg explain` them and
(ii) `check --json` emits `"code":null,"hint":null` [Verified: ran `check --json` on the type-error
file] — editors get a null code on the most common diagnostic class. The code/hint machinery
exists and is excellent where used; coverage of *which* diagnostics get codes is the gap.

**Finding 3b (positive) — `phg explain` coverage is 100%.** All 186 distinct `E-…` codes greppable
in src/checker+parser+loader+interpreter+vm are present in the explain DB
(`src/cli/explain.rs`, 191 entries); `comm -23` delta = 0. Bogus codes get a precise error with the
exact usage fix. [Verified: extracted + diffed both sets; ran `phg explain E-UNKNOWN-IDENT` and a
bogus code]

## 4. Formatter (`phg format` — NOT `fmt`)

- **Idempotent: yes.** [Verified: `format -` twice on a committed guide example → byte-identical;
  then whitespace-mangled the file, formatted twice → byte-identical AND converged back to the
  committed form.]
- `--check` semantics match its help exactly [Verified]: exit 1 when files would be reformatted,
  exit 2 + "did not parse (left unchanged)" + the parse diagnostic on an unparseable file, exit 0
  clean.
- **Finding 4a — `format --check .` at repo root flags 4 files** [Verified: exit 1, full list]:
  - `./target/s2c_php_check/src/{main,acme/util/compute}.phg` — the formatter recurses into
    `target/` **build artifacts**. Papercut: repo-root `--check` (the documented CI idiom,
    `phg format --check .`) is polluted by generated files. No exclude mechanism visible in help.
  - `./selftest/{arithmetic,faults}.phg` — **git-tracked** corpus files that escaped the Phase-1
    canonical reformat ("ALL 121 .phg purged + reformatted" — these two weren't).
- **Finding 4b — one `->` survivor in the .phg corpus**: `selftest/faults.phg:4` comment:
  `` `Test.assertFaults(() -> T)` `` — comment-level, but contradicts the canonical `=>`
  function-type syntax and the "corpus purged" claim. [Verified: grep]

## 5. LSP — three-way (implemented vs marked-done vs marked-not-done)

**Implemented** [Verified: `src/lsp/mod.rs:436` capabilities JSON + 4 `publishDiagnostics`
send-sites]: full-document sync, **hover, go-to-definition, completion** (trigger `.`),
documentSymbol, **references, documentHighlight, rename, documentFormatting**, plus push
diagnostics. Cross-file: CHANGELOG:248 claims cross-buffer definition/hover (not re-verified
live — [Unverified: would need a scripted LSP session; out of this pass's budget]).
Not implemented: signatureHelp, codeAction, semanticTokens, inlayHints, workspaceSymbols.

**Three-way contradiction** [Verified: grep FEATURES.md]:
- `FEATURES.md:74` — formatter **✅** (correct).
- `FEATURES.md:80` — "Editor/LSP, formatter **🔲 M7**" — marks BOTH the LSP and the formatter
  not-done, six lines after marking the formatter done in the same file, while `phg lsp` +
  `src/lsp/` + two editor integrations exist. Confirms A4/corpus-audit; independently re-verified.
- `FEATURES.md:67` references **`phg lex`** — no such verb (it's `tokenize`). [Verified: exit 2]

## 6. Debugger (`phg debug`) — works, interpreter-only, no variable-print

**[Verified: scripted REPL session + DAP initialize handshake]**

- REPL: pauses at first statement, `break 6` sets a breakpoint, `continue` hits it, `locals`
  works, program resumes and prints the correct value (42), clean exit. Command set (from live
  `help`): `step(s) next(n) stepout(o) continue(c) break(b) <line> clear(d) <line> locals(l)
  backtrace(bt) quit(q)`.
- DAP (`phg debug --dap`): responds to `initialize` with a well-formed success response +
  `initialized` event. Not a stub.
- **Finding 6a — no `print <var>` / eval command.** Inspection is all-or-nothing via `locals`.
  `print x` → `unknown command \`print\` — try \`help\``; same for `run` (classic gdb/xdebug
  muscle memory). The unknown-command message is decent, but single-variable inspection —
  the most common debugger action — is missing from the REPL surface.
- Interpreter-only limitation is honestly disclosed (KNOWN_ISSUES.md:219). Not in `--help`
  though — because `debug` has no help at all (§1).

## 7. Playground

- `playground/web/pkg/` prebuilt WASM assets present (`phorj_playground_bg.wasm` + JS/d.ts
  bindings). [Verified: ls]
- **`examples.js` is current**: ran `gen_examples.py`; regenerated output is **byte-identical**
  to the committed file. It contains **135 examples across 9 categories** (guide 116, plus
  start-here/bench/build/cli/…) — memory's "130/9" count is slightly stale (5 examples added
  since), the artifact itself is not. [Verified: regen + byte-compare, file restored after]
- In-browser load behavior not smoke-tested in this pass — [Unverified: no dev server/browser
  driven here; prior session verified via Playwright per memory].

## 8. Terminology consistency across surfaces

**[Verified: greps across src/ (user-facing strings), playground/web, editors/]**

Clean:
- **"package"** is used consistently as the language concept (E-NO-PACKAGE hint, checker
  messages). "namespace" appears only where it *means PHP namespaces* (transpile output
  `src/transpile/program.rs:315`, PHP keyword list in the lifter) — correct, not leakage.
- **No "Phorge"** (pre-rename name) anywhere in src/, playground/web, or editors/. 
- **No `fn `** keyword leakage in user-facing strings.
- Old enum leaf names (Obj/Arr/Str) — no user-facing hits found.

Dirty — the one systemic drift is **`->`** and the **dead `fmt` verb**:
- `->` in help examples/prose: `src/cli/mod.rs:52,77,85,129,201` (§2b) + test/fixture strings
  (`src/ast/walk.rs:571-604`, `src/bundle/section.rs:39,60`, `src/diagnostic.rs:387` — these are
  the known P1-remainder embedded-arrow population, listed here because walk.rs's strings are
  user-visible in `parse` output docs).
- `phg fmt` in **all three editor READMEs** — `editors/vscode/README.md:21`,
  `editors/phpstorm/README.md:12,41,45`, `editors/README.md:4` — actively wrong instructions on
  the most user-facing docs pages the editors ship with. (`CONTRIBUTING.md:66` `phg bench` and
  `examples/README.md:136` `phg bench --vs-php` already logged by A4; re-confirmed.)

---

## Finding index (severity-ordered)

| # | Finding | Grade |
|---|---|---|
| F1 | `run`/`runvm`/`disassemble` help examples fail verbatim (missing `package` + `import Core.Output`) — first-copy-paste breaks | Verified |
| F2 | Same examples + serve prose use purged `->` syntax; will be parser-rejected after P1 | Verified |
| F3 | `debug` + `lsp` absent from long `--help` but present in the terse usage line — two help surfaces disagree; neither verb has per-command help | Verified |
| F4 | Common type errors carry no `[E-…]` code → `check --json` `"code":null`; explain unusable for the most frequent diagnostic class | Verified |
| F5 | Editors READMEs (×3) instruct dead verb `phg fmt`; FEATURES.md:67 `phg lex` | Verified |
| F6 | FEATURES.md:80 marks LSP+formatter 🔲 while both are shipped (and :74 contradicts :80 in-file) | Verified |
| F7 | Debugger REPL has no `print <var>`/eval — `locals`-only inspection | Verified |
| F8 | `format --check .` recurses into `target/` build artifacts (2 false positives at repo root) | Verified |
| F9 | `selftest/{arithmetic,faults}.phg` tracked but not canonically formatted; `faults.phg:4` has a comment `->` survivor of the purge | Verified |
| F10 | Undocumented flags: `--dump-on-fault` (run/runvm), `benchmark --json`, `build --dev`, `build --sign` (stub, exit 2) | Verified |
| F11 | Bad-invocation errors (unknown verb/flag/missing arg) are a bare generic usage line — never name the offending token, no suggestions | Verified |
| F12 | Missing-file error leaks Rust `(os error 2)` | Verified |
| F13 | Dead verbs `fmt/lex/disasm/bench` give no "did you mean" (compounds F5) | Verified |
| F14 | Playground artifacts current: examples.js regen byte-identical, 135 examples/9 categories (memory's 130 stale); wasm pkg present; in-browser load not re-smoke-tested | Verified (load: Unverified) |

Positives worth keeping: 100% explain-DB coverage of emitted codes (F+); formatter idempotency
holds including convergence from mangled input; zero Rust panics across every error probe; zero
old-name/keyword terminology leakage; DAP handshake real; per-command help template consistent
and often excellent (serve, format, lift).
