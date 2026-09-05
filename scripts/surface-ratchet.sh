#!/usr/bin/env bash
# Surface ratchet — Invariant 17's 100% RULE, made mechanical.
#
# WHY THIS EXISTS. Invariant 17 says the LSP and both editors must support EVERYTHING we implement,
# "no exceptions, no lag", and Invariant 9 says a feature ships with an example. Both were being
# asserted in prose and neither was measured, so the true state on 2026-08-07 was:
#
#     310 `E-*` codes emitted in src/ — only 64 asserted by ANY test (21%)
#     conformance/ referenced 10 of them
#     the LSP advertised 8 providers; signatureHelp/codeAction/semanticTokens/inlayHint absent
#     editors/phpstorm/ contained nothing but a README
#
# The ONE surface that was complete — `phg explain`, 305/310 — is complete precisely because
# `explain_ratchet` fails the build when a code has no entry. Nothing else had that, so nothing else
# stayed current. A prose rule that no gate enforces decays to whatever the last hurried commit left
# behind; this session watched `#[Config]` documentation rot within a single week.
#
# WHAT IT DOES. Counts today's covered surface and refuses to let it SHRINK. It deliberately does NOT
# demand 100% on day one — that would mean authoring ~246 diagnostic fixtures before anything else
# could land, so the gate would be turned off within the hour and we would be back to prose. Instead
# the baseline is a FLOOR that only moves up: a new diagnostic code must arrive with coverage, or the
# count drops below the floor and the push fails.
#
# Same shape as `scripts/size-gate.sh` (grandfathered file sizes) and `scripts/doc-guards.sh`
# (frozen doc violations): freeze the debt, forbid its growth, pay it down deliberately.
#
# Usage:  bash scripts/surface-ratchet.sh          # gate (used by pre-push + CI)
#         bash scripts/surface-ratchet.sh --emit   # re-freeze the floor AFTER coverage improves
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"
BASELINE="scripts/surface-baseline.txt"
EMIT=0
[[ "${1:-}" == "--emit" ]] && EMIT=1

# ── Measure ───────────────────────────────────────────────────────────────────────────────────────
# Codes EMITTED by the compiler — scanned over NON-TEST src only. Scanning all of `src/` counted a
# token that exists solely in a test as both an emitted code AND an asserted one, inflating both
# sides of the ratio: `E-MULTIPLE-MAIN` (no emit site at all — `src/ast/entry.rs` names it only in a
# NOTE explaining it is not the rule) and `E-VARIADIC` (only ever a quoted literal in a test; the
# real codes are its `E-VARIADIC-*` children). The `E-FOO`/`E-NOPE` fixtures were the same class,
# handled by an ad-hoc blocklist that did not scale — excluding test files by PATH generalizes it.
# `E-TYPE` still needs the blocklist: it lives in a `#[cfg(test)]` module INSIDE a non-test file.
# The `phg explain` catalog (`src/cli/explain/` + `src/cli/explain_*.rs`) is excluded for the same
# reason `cli::tests::explain_ratchet` skips it: its string literals DEFINE explanations, they are not
# emission sites. Counting them inflated `codes_total` with the catalog's deliberate TOMBSTONES —
# `E-MODULE-UNAVAILABLE` (retired by DEC-273, kept so old readers are redirected) and
# `E-VENDOR-MISSING` (folded into `E-MODULE-NOT-FOUND` by DEC-282/316) — which then showed up as
# "unasserted debt" that no test could ever pay, because nothing emits them [found 2026-09-05].
mapfile -t src_files < <(
  git ls-files -- 'src/*.rs' 'src/**/*.rs' \
    | grep -vE '(^|/)tests/|(^|/)tests\.rs$|[^/]*tests[^/]*\.rs$' \
    | grep -vE '^src/cli/explain(/|_[a-z_]+\.rs$)'
)
# An empty array would pass `grep` NO file operands, at which point it reads STDIN and blocks
# forever. A gate that hangs is worse than one that fails — CI just times out with no diagnosis, and
# locally it looks like a slow build. Fail fast and say why.
if ((${#src_files[@]} == 0)); then
  echo "surface-ratchet: FAIL — found NO non-test src files to scan for diagnostic codes." >&2
  echo "  Refusing to run: the code scan would read stdin and hang." >&2
  exit 1
fi
# THREE emit forms, matching `cli::tests::explain_ratchet`'s reading of the compiler plus one it also
# misses: a standalone quoted code (`err_coded("E-FOO", …)`), a bracketed code inside a message
# (`[E-FOO]` — the loader's plain-`String` errors), and a code used as a MESSAGE PREFIX
# (`"E-FOO: …"` — the transpiler's ladder gates). Until 2026-09-05 only the first form was scanned;
# the 25 loader codes and the 3 prefix codes were being counted only because the `phg explain`
# catalog (now excluded above) happened to name them. Nothing new is emitted here — the scan now
# sees what was always there.
mapfile -t codes < <(
  # `|| true` on each form: a tree with no code in ONE form makes that grep exit 1, and under
  # `set -e` inside this process substitution that aborted the group — the remaining forms were
  # never scanned and the script died with no message (test-surface-ratchet case 8 caught it).
  {
    grep -rhoE '"E-[A-Z0-9-]+"' "${src_files[@]}" | tr -d '"' || true
    grep -rhoE '\[E-[A-Z0-9-]+\]' "${src_files[@]}" | tr -d '[]' || true
    grep -rhoE '"E-[A-Z0-9-]+:' "${src_files[@]}" | tr -d '":' || true
  } | sort -u | grep -vxE 'E-TYPE'
)
total="${#codes[@]}"
# `pct` below divides by `total`, so zero codes kills the script with a bash arithmetic error
# instead of a diagnosis. Zero is never legitimate here — this repo emits hundreds — so it means the
# scan broke, which is precisely the silent-miscount failure this ratchet was fixed for.
if ((total == 0)); then
  echo "surface-ratchet: FAIL — scanned ${#src_files[@]} src file(s) and found ZERO diagnostic codes." >&2
  echo "  That is never legitimate here; the scan is broken. Refusing to report a ratio." >&2
  exit 1
fi

# Every file that can ASSERT a code. `grep --include` matches the BASENAME, not the path, so the
# three original patterns (`tests/**.rs`, `*tests*.rs`, `tests.rs`) silently missed the commonest
# shape in this repo by far: a test module in a `tests/` DIRECTORY, e.g. `src/checker/tests/
# mutation.rs`. That is 101 files, and it made the ratchet report 83/307 (27%) when the true figure
# was 253/307 (82%). The percentage being wrong was the harmless half — the damage was that the
# FLOOR sat at 83, so the coverage of 170 codes was unprotected: deleting the only test asserting
# `E-ASSIGN-TYPE` would not have tripped this gate. Enumerate by PATH instead, and never reintroduce
# a basename-only filter here.
mapfile -t test_files < <(
  git ls-files -- 'tests/*.rs' 'tests/**/*.rs' \
    'src/**/tests/*.rs' 'src/**/*tests*.rs' 'src/**/tests.rs' 2>/dev/null | sort -u
)
if ((${#test_files[@]} == 0)); then
  echo "surface-ratchet: FAIL — found NO test files; the assertion scan would count every code as" >&2
  echo "  unasserted and the ratchet would be measuring nothing. Refusing to report a number." >&2
  exit 1
fi

# One pass over the test corpus + conformance, rather than 4 recursive greps per code (307 codes ×
# 4 = 1228 full-tree scans). Substring presence, matching the original semantics exactly.
mapfile -t asserted_list < <(
  {
    grep -rhoE 'E-[A-Z0-9-]+' "${test_files[@]}" 2>/dev/null || true
    grep -rhoE 'E-[A-Z0-9-]+' conformance/ 2>/dev/null || true
  } | sort -u
)
mapfile -t conformance_list < <(
  grep -rhoE 'E-[A-Z0-9-]+' conformance/ 2>/dev/null | sort -u || true
)
# Intersect the emitted set with each scanned set. `-x` (WHOLE-LINE match) is load-bearing, not
# tidiness: the old substring test counted `E-MISSING-RETURN` as covered because a fixture rendered
# `E-MISSING-RETURN-TYPE`, i.e. one code's coverage was credited to a DIFFERENT code that merely
# has it as a prefix. That inflated `codes_in_conformance` to 25 when the honest count was 24.
# `|| true` because grep exits 1 on no match, and under `set -e` + `pipefail` an empty intersection
# would abort the script rather than report zero.
asserted=$(printf '%s\n' "${codes[@]}" \
  | grep -cFxf <(printf '%s\n' "${asserted_list[@]}") || true)
conformance=$(printf '%s\n' "${codes[@]}" \
  | grep -cFxf <(printf '%s\n' "${conformance_list[@]}") || true)

# LSP capabilities ACTUALLY advertised: the `…Provider` keys of `INITIALIZE_RESULT`, the JSON the
# server sends. The previous pattern (`"[a-zA-Z]+Provider"`) could not match that constant at all —
# inside a Rust string every quote is `\"` — so it was counting the provider names that TESTS happened
# to quote, i.e. what the suite mentioned rather than what the server offered [found 2026-09-05 when
# `signatureHelpProvider` shipped and the number stayed at 8].
# `|| true`: zero providers is a legitimate reading (the floor catches it), not a script abort.
lsp=$( (grep -hoE '[a-zA-Z]+Provider\\"' src/lsp/mod.rs 2>/dev/null || true) | sort -u | wc -l)

# Runnable examples — Invariant 9's corpus, which is also the byte-identity gate.
examples=$(find examples -name '*.phg' | wc -l)

# ── Emit ──────────────────────────────────────────────────────────────────────────────────────────
if [[ "$EMIT" == 1 ]]; then
  cat >"$BASELINE" <<EOF
# Surface-coverage FLOOR (scripts/surface-ratchet.sh). Each number may only GO UP.
# Re-freeze with \`bash scripts/surface-ratchet.sh --emit\` after coverage improves, and say in the
# commit message what was added. Lowering a number by hand is laundering the gate, not fixing it.
#
# codes_total is informational and may move in EITHER direction (a code can be legitimately retired);
# the ratio floors below are what actually gate.
codes_total $total
codes_asserted $asserted
codes_in_conformance $conformance
lsp_providers $lsp
examples $examples
EOF
  echo "surface-ratchet: wrote $BASELINE"
  sed -n '6,$p' "$BASELINE" | sed 's/^/  /'
  exit 0
fi

# ── Gate ──────────────────────────────────────────────────────────────────────────────────────────
[[ -f "$BASELINE" ]] || {
  echo "surface-ratchet: no $BASELINE — create it with --emit" >&2
  exit 1
}
get() { awk -v k="$1" '$1==k{print $2}' "$BASELINE"; }

fails=0
check() { # name actual floor
  local name="$1" actual="$2" floor="$3"
  if ((actual < floor)); then
    echo "surface-ratchet: FAIL $name = $actual, floor is $floor — coverage went DOWN" >&2
    fails=$((fails + 1))
  else
    local extra=""
    ((actual > floor)) && extra="  (+$((actual - floor)) — re-emit to lock it in)"
    printf '  ok   %-22s %s / floor %s%s\n' "$name" "$actual" "$floor" "$extra"
  fi
}

echo "surface-ratchet: Invariant 17 / Invariant 9 coverage floors"
check codes_asserted "$asserted" "$(get codes_asserted)"
check codes_in_conformance "$conformance" "$(get codes_in_conformance)"
check lsp_providers "$lsp" "$(get lsp_providers)"
check examples "$examples" "$(get examples)"

pct=$((asserted * 100 / total))
if ((asserted >= total)); then
  echo "surface-ratchet: $asserted/$total diagnostic codes asserted (${pct}%) — the 100% RULE is MET for diagnostics; the floor keeps it there"
else
  echo "surface-ratchet: $asserted/$total diagnostic codes asserted (${pct}%) — the 100% RULE is NOT met yet"
  echo "surface-ratchet:   remaining debt is tracked, not hidden; see docs/plans/SLICE-STATE.md"
fi

if ((fails > 0)); then
  echo "surface-ratchet: FAILED — $fails floor(s) breached" >&2
  exit 1
fi
echo "surface-ratchet: PASS"
