#!/usr/bin/env bash
# Test suite for surface-ratchet.sh — the Invariant 17 / Invariant 9 coverage gate.
#
# WHY THIS EXISTS. The ratchet shipped 2026-08-08 with no test, and on 2026-08-22 it was found to
# have been measuring the wrong thing the whole time: it decided "is this code asserted?" with
# `grep --include`, which matches the BASENAME, not the path, so 101 test files living in a `tests/`
# DIRECTORY were invisible. It reported 83/307 (27%) against a true 252/307 (82%) — and, far worse,
# froze its FLOOR at 83, leaving 169 codes' coverage unprotected. A gate that reads green while not
# covering what it claims is this repo's characteristic failure (see the DEC-191 no-op example glob).
#
# The fix was sabotage-verified BY HAND, which is exactly the state that let the original bug live.
# These cases re-run that proof on every push, so the ratchet cannot go dark again.
#
# ISOLATION. `surface-ratchet.sh` resolves its repo root from `BASH_SOURCE[0]` (`dirname/..`) and
# `cd`s there, so running it from another directory would still measure THIS repo. Every case
# therefore COPIES the tool into a throwaway git repo and runs it from inside — the bash-script
# isolation pattern.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TOOL="$HERE/surface-ratchet.sh"
PASS=0
FAIL=0
ok() {
  printf '  ok   — %s\n' "$1"
  PASS=$((PASS + 1))
}
bad() {
  printf '  FAIL — %s\n' "$1"
  FAIL=$((FAIL + 1))
}

TMP=$(mktemp -d /tmp/test-surface-ratchet-XXXXXX)
trap 'rm -rf "$TMP"' EXIT

# A minimal repo with every surface the ratchet measures. Deliberately tiny: three emitted codes —
# one asserted from a `tests/` DIRECTORY (the shape the bug missed), one pinned by a conformance
# fixture, and one left UNASSERTED so a case can raise coverage.
mkrepo() {
  local r="$1"
  mkdir -p "$r/scripts" "$r/src/checker/tests" "$r/src/lsp" "$r/conformance/diagnostics" "$r/examples" "$r/tests"
  cp "$TOOL" "$r/scripts/surface-ratchet.sh"

  # Three codes emitted by "the compiler" (a non-test src file). E-GAMMA starts UNASSERTED so a
  # case can raise coverage; without it, "coverage went up" is unrepresentable in the fixture.
  printf 'fn a() { err("E-ALPHA"); }\nfn b() { err("E-BETA"); }\nfn c() { err("E-GAMMA"); }\n' >"$r/src/checker/emit.rs"
  # E-ALPHA asserted ONLY from a tests/ DIRECTORY — invisible to the pre-fix ratchet.
  printf 'assert_eq!(code, Some("E-ALPHA"));\n' >"$r/src/checker/tests/alpha_test.rs"
  # E-BETA pinned by a conformance fixture.
  printf 'type error at 1:1: boom\n  [E-BETA]\n' >"$r/conformance/diagnostics/beta.expected"
  # A code-free integration test. Its ONLY job is to keep the test-file enumeration non-empty when a
  # case deletes the src test module — otherwise the empty-enumeration guard fires first and masks
  # the floor breach the case is actually about. (It caught exactly that when first run.)
  printf 'fn smoke() { assert!(true); }\n' >"$r/tests/integration.rs"

  # The REAL shape of `INITIALIZE_RESULT`: a JSON string inside a Rust string, every quote `\"`.
  printf 'const INITIALIZE_RESULT: &str = "{\\"capabilities\\":{\\"hoverProvider\\":true}}";\n' >"$r/src/lsp/mod.rs"
  printf 'package Main;\n' >"$r/examples/a.phg"
  (
    cd "$r" && git init -q . && git add -A \
      && git -c user.email=t@t -c user.name=t commit -qm init
  ) 2>/dev/null
}

commit_all() { (cd "$1" && git add -A && git -c user.email=t@t -c user.name=t commit -qm x) 2>/dev/null; }
run() { (cd "$1" && bash scripts/surface-ratchet.sh "${2:-}" 2>&1); }
floor_of() { awk -v k="$2" '$1==k{print $2}' "$1/scripts/surface-baseline.txt"; }

echo "== surface-ratchet.sh =="
if [[ ! -f "$TOOL" ]]; then
  bad "tool exists at $TOOL"
  printf '\n%s passed, %s failed\n' "$PASS" "$FAIL"
  exit 1
fi
ok "tool exists"
bash -n "$TOOL" && ok "bash -n parses" || bad "bash -n parses"

# ── 1. THE REGRESSION. A code asserted only from a `tests/` DIRECTORY must COUNT. ────────────────
# This is the entire bug: `--include='*tests*.rs'` matches `list_tests.rs` but not
# `src/checker/tests/alpha_test.rs`, so E-ALPHA read as unasserted and its coverage was unprotected.
R="$TMP/dir"
mkrepo "$R"
run "$R" --emit >/dev/null
[[ "$(floor_of "$R" codes_asserted)" == "2" ]] \
  && ok "a code asserted from a tests/ DIRECTORY counts (codes_asserted=2)" \
  || bad "tests/ DIRECTORY assertion counted (got codes_asserted=$(floor_of "$R" codes_asserted), want 2)"

# ── 2. …and the FLOOR it freezes actually protects that code. ────────────────────────────────────
# Freezing the right number is only half of it; the gate must FAIL when the assertion is deleted.
# Under the pre-fix script this deletion was invisible, which is what made the bug damaging.
rm "$R/src/checker/tests/alpha_test.rs"
commit_all "$R"
out=$(run "$R")
rc=$?
[[ $rc -ne 0 ]] && ok "deleting the only assertion FAILS the gate" || bad "deleting the only assertion FAILS the gate (exit $rc)"
grep -qF 'coverage went DOWN' <<<"$out" && ok "names the breach" || bad "names the breach (got: $out)"

# ── 3. A PREFIX must not be credited another code's coverage. ────────────────────────────────────
# The old substring test counted `E-MISSING-RETURN` as covered because a fixture rendered
# `E-MISSING-RETURN-TYPE`. Here E-ALPHA is emitted but only E-ALPHA-EXTRA is pinned.
R="$TMP/prefix"
mkrepo "$R"
rm "$R/src/checker/tests/alpha_test.rs" "$R/conformance/diagnostics/beta.expected"
printf 'type error at 1:1: boom\n  [E-ALPHA-EXTRA]\n' >"$R/conformance/diagnostics/x.expected"
commit_all "$R"
run "$R" --emit >/dev/null
[[ "$(floor_of "$R" codes_in_conformance)" == "0" ]] \
  && ok "a longer code does not credit its prefix (codes_in_conformance=0)" \
  || bad "prefix false positive (codes_in_conformance=$(floor_of "$R" codes_in_conformance), want 0)"

# ── 4. A test-only token must not enter the DENOMINATOR. ─────────────────────────────────────────
# Scanning all of src/ counted a code existing only in a test as both emitted AND asserted, which
# inflated both sides of the ratio (E-MULTIPLE-MAIN, E-VARIADIC in the real repo).
R="$TMP/denom"
mkrepo "$R"
printf 'assert_eq!(code, Some("E-TESTONLY"));\n' >>"$R/src/checker/tests/alpha_test.rs"
commit_all "$R"
run "$R" --emit >/dev/null
[[ "$(floor_of "$R" codes_total)" == "3" ]] \
  && ok "a test-only token stays out of the denominator (codes_total=3)" \
  || bad "test-only token polluted the denominator (codes_total=$(floor_of "$R" codes_total), want 3)"

# ── 5. An EMPTY test-file enumeration must fail loudly, never report a number. ───────────────────
# Otherwise every code reads as unasserted and the ratchet silently measures nothing — the exact
# shape of the bug this suite exists for, one level up.
R="$TMP/notests"
mkrepo "$R"
(cd "$R" && git rm -q src/checker/tests/alpha_test.rs tests/integration.rs)
commit_all "$R"
out=$(run "$R" --emit)
rc=$?
[[ $rc -ne 0 ]] && ok "no test files at all = hard failure" || bad "no test files at all = hard failure (exit $rc)"
grep -qF 'measuring nothing' <<<"$out" && ok "explains WHY it refuses" || bad "explains why it refuses (got: $out)"

# ── 6. An EMPTY src enumeration must fail, not HANG. ─────────────────────────────────────────────
# `grep -rhoE PATTERN "${src_files[@]}"` with an empty array passes grep no file operands, so it
# reads STDIN and blocks forever. A gate that hangs is worse than one that fails: CI just times out.
R="$TMP/nosrc"
mkrepo "$R"
(cd "$R" && git rm -q "src/checker/emit.rs" "src/lsp/mod.rs")
commit_all "$R"
# Deliberately NOT redirecting stdin. An earlier version of this case passed `</dev/null`, which
# hands grep an immediate EOF and makes the hang structurally impossible to observe — the case was
# green while proving nothing. `timeout` is what makes the hang detectable instead.
out=$(timeout 20 bash -c "cd '$R' && bash scripts/surface-ratchet.sh --emit" 2>&1)
rc=$?
[[ $rc -eq 124 ]] && bad "empty src HANGS (timed out) — must fail fast" || ok "empty src does not hang"
[[ $rc -ne 0 ]] && ok "empty src is a hard failure" || bad "empty src is a hard failure (exit $rc)"
grep -qF 'would read stdin and hang' <<<"$out" && ok "names the hang risk" || bad "names the hang risk (got: $out)"

# ── 7. A zero denominator must not divide by zero. ───────────────────────────────────────────────
# `pct=$((asserted * 100 / total))` aborts the script on total=0, so the gate dies with a bash
# arithmetic error instead of a diagnosis.
R="$TMP/zero"
mkrepo "$R"
run "$R" --emit >/dev/null
printf 'fn a() { plain(); }\n' >"$R/src/checker/emit.rs"
commit_all "$R"
out=$(run "$R")
rc=$?
grep -qiE 'divide by zero|division by 0' <<<"$out" && bad "zero codes divides by zero" || ok "zero codes does not divide by zero"
[[ $rc -ne 0 ]] && ok "zero emitted codes is a hard failure" || bad "zero emitted codes is a hard failure (exit $rc)"

# ── 8. The happy path still passes when nothing changed. ────────────────────────────────────────
R="$TMP/steady"
mkrepo "$R"
run "$R" --emit >/dev/null
out=$(run "$R")
rc=$?
[[ $rc -eq 0 ]] && ok "an unchanged tree passes" || bad "an unchanged tree passes (exit $rc: $out)"
grep -qF 'PASS' <<<"$out" && ok "reports PASS" || bad "reports PASS"

# ── 9. Coverage going UP passes and is reported as re-emittable. ────────────────────────────────
printf 'assert_eq!(code, Some("E-GAMMA"));\n' >>"$R/src/checker/tests/alpha_test.rs"
commit_all "$R"
out=$(run "$R")
rc=$?
[[ $rc -eq 0 ]] && ok "coverage going UP passes" || bad "coverage going UP passes (exit $rc)"
grep -qF 're-emit to lock it in' <<<"$out" && ok "invites a re-emit" || bad "invites a re-emit (got: $out)"

# ── 10. A BRACKETED emit (`[E-FOO]` in a message — the loader's form) is in the denominator. ────
# Until 2026-09-05 only `"E-FOO"` was scanned; the loader's 25 codes were counted only because the
# `phg explain` catalog named them, and vanished when the catalog was excluded.
R="$TMP/bracket"
mkrepo "$R"
printf 'fn d() { Err(format!("no such module [E-DELTA]")) }\n' >>"$R/src/checker/emit.rs"
commit_all "$R"
out=$(run "$R" --emit)
grep -qE 'codes_total 4' <<<"$out" && ok "a bracketed [E-…] emit enters the denominator" || bad "a bracketed [E-…] emit enters the denominator (got: $out)"

# ── 11. A PREFIX emit (`"E-FOO: …"` — the transpiler's ladder gates) is in the denominator. ──────
printf 'fn e() { fail("E-EPSILON: native-only") }\n' >>"$R/src/checker/emit.rs"
commit_all "$R"
out=$(run "$R" --emit)
grep -qE 'codes_total 5' <<<"$out" && ok "a prefix \"E-…:\" emit enters the denominator" || bad "a prefix emit enters the denominator (got: $out)"

# ── 12. The `phg explain` CATALOG is not an emit site. ──────────────────────────────────────────
# A code that exists ONLY as an explanation (a tombstone for a retired code) must not enter the
# denominator — it is debt no test could ever pay, because nothing emits it.
mkdir -p "$R/src/cli/explain"
printf '"E-ZETA" => "E-ZETA — RETIRED",\n' >"$R/src/cli/explain/retired.rs"
printf '"E-ETA" => "E-ETA — also catalog-only",\n' >"$R/src/cli/explain_config.rs"
commit_all "$R"
out=$(run "$R" --emit)
grep -qE 'codes_total 5' <<<"$out" && ok "the explain catalog does not enter the denominator" || bad "the explain catalog does not enter the denominator (got: $out)"

# ── 13. `lsp_providers` counts what the server ADVERTISES, not what a test happens to quote. ─────
# The old pattern could not match `INITIALIZE_RESULT` at all (its quotes are `\"`), so it was
# counting provider names from test assertions — the suite's vocabulary, not the server's offer.
printf 'assert!(out.contains("completionProvider"));\n' >"$R/src/checker/tests/alpha_test.rs"
commit_all "$R"
out=$(run "$R" --emit)
grep -qE 'lsp_providers 1' <<<"$out" && ok "a provider quoted only by a test does not count" || bad "a provider quoted only by a test does not count (got: $out)"
printf 'const INITIALIZE_RESULT: &str = "{\\"capabilities\\":{\\"hoverProvider\\":true,\\"signatureHelpProvider\\":{}}}";\n' >"$R/src/lsp/mod.rs"
commit_all "$R"
out=$(run "$R" --emit)
grep -qE 'lsp_providers 2' <<<"$out" && ok "a provider added to the initialize response counts" || bad "a provider added to the initialize response counts (got: $out)"

printf '\n%s passed, %s failed\n' "$PASS" "$FAIL"
[[ $FAIL -eq 0 ]]
