# SPEC — Labeled `break` / `continue` (DEC-331 D10c design slice)

> Status: **SPEC FROZEN, awaiting dev ruling.** D10c ruling: the safe, structured, fully
> typeable "goto for nested-loop escape"; RAW goto stays rejected.

## 1. Surface (proposed — cross-language scan per META-7)

Rust/Java/Kotlin-style labels; Kotlin's `label@` prefix form recommended (reads naturally,
parses unambiguously — a label is an identifier followed by `@` directly before a loop
keyword):

```phg
function findPair(List<int> xs, int target): int {
    outer@ for (i in Range.of(0, List.length(xs))) {
        for (j in Range.of(0, List.length(xs))) {
            if (xs[i] + xs[j] == target) {
                break outer@;        // exits BOTH loops
            }
            if (xs[j] < 0) {
                continue outer@;     // next i
            }
        }
    }
    return 0;
}
```

Alternatives surveyed: Java/JS `outer:` prefix (colon collides with nothing here but reads
like a type annotation), Rust `'outer:` (tick is alien to phorj). **Recommended: `label@`.**

## 2. Semantics

- A label may prefix `while` / `for` / `loop` only (`E-LABEL-TARGET` otherwise).
- `break L@` / `continue L@` must name a label of a LEXICALLY ENCLOSING loop
  (`E-UNKNOWN-LABEL`); unlabeled forms unchanged (innermost).
- Labels are block-scoped; shadowing an outer label = `E-DUPLICATE-LABEL` (stricter than
  Java — no silent inner rebind).
- Fully typeable: no new control-flow edges beyond structured multi-level exit; definite
  return/init analysis extends mechanically (a labeled break is an exit edge to the labeled
  loop's join point).

## 3. Backends (Invariant 17)

- **Compiler**: labels resolve at compile time to jump targets — `Op::Jump` to the labeled
  loop's break/continue address. NO new `Op` (the label is compile-time only, Invariant 5);
  the loop-context stack in `compiler/stmt/loops.rs` gains a label field.
- **Interp**: mirrors with a labeled loop-control signal (the existing break/continue signal
  carries an optional depth/label).
- **Transpile**: PHP has `break N`/`continue N` (numeric levels) — the emitter computes the
  LEVEL DISTANCE from the labeled construct (faithful, tier 1; PHP ≥ 5.4 semantics identical).
- **Lift**: PHP `break 2`/`continue 2` lifts to a synthesized label on the target loop
  (`l1@`, `l2@`, …) — closes an existing lift gap (today `break 2` presumably refuses).

## 4. Examples & tests

`examples/labeled_loops.phg` + README row; differential cases: labeled break/continue from
2- and 3-deep nests, label+`??`/match interplay, `continue` on a labeled `while` (condition
re-eval); checker negatives for all three errors; transpile snapshot proving `break 2`
round-trips (lift → run → transpile) byte-identically.

## 5. PENDING for dev

- **P1**: the surface form — `label@ for` (recommended) vs `label: for`.
- **P2**: allow labels on plain blocks with `break label@` (Kotlin/Rust allow; recommended:
  NO for v1 — loops only, smallest surface).
