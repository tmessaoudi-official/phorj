//! Lifter tests added 2026-09-05: the `lift_from` end-to-end ratchet and LIFT-ECHO-INT. Split from
//! `lifter_tests.rs` at the Invariant-13 hard cap.
use super::lifter::lift_source;
use super::lifter_tests::{assert_reparses, lift};

/// EVERY registered `lift_from` builtin must actually resolve end to end — not merely be spelled in
/// a row.
///
/// Resolution is arity-gated (`lift::lifter::exprs` requires `nat.params.len() == args.len()`), and a
/// mismatch fails SILENTLY: the call stays as a bare PHP name in the draft, while the registration
/// still greps as handled. That is the same "named catch-all reads as deliberate" failure the
/// DEC-356 ratchet exists to stop, one layer down. Before this test, three builtins were checked by
/// name (`strlen`, `strtoupper`, `sqrt`) and the other 70 rested on the uniqueness test, which does
/// not lift anything.
///
/// Ratchet, not a spot check: a new `lift_from` entry is exercised the moment it is written.
///
/// **What this does NOT assert**, stated so the green is not read as more than it is: (a) that the
/// claim is SEMANTICALLY right — registering `array_pad` on `List.slice` would still resolve, and
/// only reading the two implementations catches that; (b) anything about arities other than the
/// native's own, since the probe builds its argument list from `params.len()`. (b) was measured
/// rather than assumed: `substr($s, 1)`, `number_format($n)`, `round($n)`, `str_pad($s, 5)`,
/// `array_slice($a, 1)` and ten more all resolve at their common SHORTER PHP arity too, so there is
/// no optional-argument gap to gate today [Verified 2026-09-04].
#[test]
fn every_registered_builtin_lifts_end_to_end() {
    let (mut dead, mut errs) = (vec![], vec![]);
    for n in crate::native::registry() {
        for b in n.lift_from {
            // Arity taken from the native itself, which is exactly what the resolver compares.
            let args = vec!["\"x\""; n.params.len()].join(", ");
            let php = format!("<?php echo {b}({args});");
            match lift_source(&php) {
                // The `import <module>;` line appears ONLY on resolution — an unresolved call is
                // left as a bare PHP name with no import, which is what the negative test above
                // pins. So the import is the resolution signal, and it does not depend on how the
                // receiver form happens to render.
                Ok(out) => {
                    // Two signals, because either alone can lie: the import can appear while the
                    // call is left verbatim, and a call can vanish into an unrelated rewrite.
                    //
                    // The second signal is "the call is now QUALIFIED", never "the builtin's name is
                    // gone" — 32 rows share their PHP name (`sqrt`, `min`, `log`, `exp` …), so the
                    // name test would have reported every one of them dead. `.name(` matches both
                    // shapes the lifter emits: the module form `Math.sqrt(x)` and DEC-326's receiver
                    // form `"hi".upperCase()`.
                    let imported = out.contains(&format!("import {};", n.module));
                    let qualified = out.contains(&format!(".{}(", n.name));
                    if !imported || !qualified {
                        dead.push(format!(
                            "{b} -> {}.{} (arity {}) did not resolve (import={imported}, \
                             qualified={qualified}):\n{out}",
                            n.module,
                            n.name,
                            n.params.len()
                        ));
                    }
                }
                Err(e) => errs.push(format!("{b}: {e}")),
            }
        }
    }
    let registered: usize = crate::native::registry()
        .iter()
        .map(|n| n.lift_from.len())
        .sum();
    // Vacuity guard: if the registry ever comes back empty this test would pass having lifted
    // nothing, which is the exact shape of green-but-measuring-nothing this repo keeps finding.
    assert!(
        registered >= 70,
        "only {registered} lift registrations found — the scan is broken, not the registry"
    );
    assert!(
        dead.is_empty() && errs.is_empty(),
        "{} registration(s) DEAD (registered but never fire):\n{}\n\n{} lift error(s):\n{}",
        dead.len(),
        dead.join("\n"),
        errs.len(),
        errs.join("\n")
    );
}

/// LIFT-ECHO-INT — `echo` of a non-string EXPRESSION (a call, a literal, a `.` chain) lifts to an
/// interpolation the checker accepts; a string literal and a bare variable stay as written; a `.`-chain is flattened into ONE interpolation rather than a `+` chain
/// (which would be a type error on an int operand); int/bool literals become the text PHP prints.
#[test]
fn echo_of_a_non_string_lifts_to_an_interpolation() {
    let out = lift(
        "<?php function half(int $n): int { return intdiv($n, 2); }\nfunction main(): void { echo half(3); echo \"x\" . half(4) . \"y\"; echo \"plain\"; echo 42; echo true; }",
    );
    assert!(out.contains("Output.print(\"{half(3)}\")"), "{out}");
    assert!(out.contains("Output.print(\"x{half(4)}y\")"), "{out}");
    assert!(out.contains("Output.print(\"plain\")"), "{out}");
    assert!(out.contains("Output.print(\"42\")"), "{out}");
    assert!(out.contains("Output.print(\"1\")"), "{out}");
    assert_reparses(&out);
    // The draft must CHECK — that is the whole point.
    let prog = crate::cli::parse_program(&out).expect("parses");
    crate::cli::check_and_expand(&prog, &out).expect("the lifted echo draft type-checks");
}
