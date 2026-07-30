//! THE canonical fault-body vocabulary (DEC-361 — Wave 1.5).
//!
//! **The finding this closes.** A fault body is *parity-affecting*: Invariant 1 demands identical
//! failure behaviour across `phg run`, `--tree-walker` and the transpiled PHP, and Invariant 4 already
//! single-sources the arithmetic fault consts in [`super::arith`] for exactly that reason. Everything
//! else had been re-typed at each site — `"stack overflow"` at five VM sites plus a JIT const,
//! `"recv from empty channel"` in both backends, `"no field …"` in four places,
//! `"list index out of range"` three times. And the worst of it: `tests/differential.rs::classify`,
//! the test whose entire job is to catch fault-body drift, keyed on its OWN independent copies of all
//! twelve bodies — so a drift was not merely untested, it was *invisible*. The ruling was explicit that
//! single-sourcing alone would be insufficient for that reason.
//!
//! **The division of labour with [`super::arith`].** The arithmetic consts stay where Invariant 4 puts
//! them, next to the checked kernels that raise them. This module is the INDEX: it re-exports them and
//! defines every other body, so there is one import surface (`crate::value::faults::*`) and one place a
//! reviewer has to look. Adding a body here without teaching `classify` about it fails the ratchet in
//! `tests/differential.rs` — the derivation the ruling asked for.
//!
//! **Bodies with a payload** are functions, not consts, so the *shape* is single-sourced too — a
//! `format!` re-typed at the call site is the same defect wearing a different hat.

pub use super::arith::{
    FAULT_DECIMAL_DIV_ZERO, FAULT_DECIMAL_MOD_ZERO, FAULT_DECIMAL_NONTERMINATING,
    FAULT_DECIMAL_OVERFLOW, FAULT_DECIMAL_SCALE, FAULT_DIV_ZERO, FAULT_INT_OVERFLOW,
    FAULT_MOD_ZERO, FAULT_NEGATIVE_EXPONENT, FAULT_NEGATIVE_SHIFT,
};

/// Call depth exceeded `limits::MAX_CALL_DEPTH`. Raised at five VM call sites and mirrored by the JIT's
/// own depth guard, which keeps the fault at the VM's exact threshold — so the body must be one string.
pub const FAULT_STACK_OVERFLOW: &str = "stack overflow";

/// A list index outside `0..len` — checker-valid (the checker proves the index is an `int`, never that
/// it is in range), runtime-reachable, and raised on both the read and the write path.
pub const FAULT_INDEX_OOB: &str = "list index out of range";

/// A range literal wider than [`super::MAX_RANGE_LEN`], caught before allocating.
pub const FAULT_RANGE_TOO_LARGE: &str = "range too large";

/// `Channel.recv()` on an empty channel.
pub const FAULT_CHANNEL_EMPTY: &str = "recv from empty channel";

/// `Task.join()` on a task that has not completed.
pub const FAULT_JOIN_INCOMPLETE: &str = "join on an incomplete task";

/// Non-exhaustive `match` fall-through — a checker-unreachable backstop on every leg, including the
/// PHP one. **This body is why the ruling exists:** the PHP leg threw a bare
/// `new \UnhandledMatchError()`, whose `getMessage()` is the EMPTY STRING
/// [Verified: ran `throw new \UnhandledMatchError()` under php-8.5.8 and printed `[]`], while both Rust
/// legs said this. A drift that had already happened, in the one body a reader would assume was safe
/// because PHP has a built-in for it.
pub const FAULT_NON_EXHAUSTIVE_MATCH: &str = "non-exhaustive match at runtime";

/// `opt!` force-unwrap of a `null`.
pub const FAULT_FORCE_UNWRAP_NULL: &str = "force-unwrap of null";

/// `todo()` — an unimplemented path.
pub const FAULT_TODO: &str = "todo: not yet implemented";

/// `unreachable()` — a path the programmer asserts cannot happen.
pub const FAULT_UNREACHABLE: &str = "entered unreachable code";

/// A failed `assert(cond)` with no message given. The message-carrying form is [`assert_with`].
pub const FAULT_ASSERT: &str = "assertion failed";

/// `panic("msg")` — an explicit programmer abort. Prefix only; the body is [`panic_with`].
pub const FAULT_PANIC_PREFIX: &str = "panic: ";

/// Reading a field absent from an instance — checker-valid and runtime-reachable when an explicit
/// (uninitialized) field member is read, since construction only populates promoted ctor params.
/// Prefix only; the body is [`no_field`].
pub const FAULT_NO_FIELD_PREFIX: &str = "no field ";

/// `Enum.from(value)` with no matching case. Prefix only; the body is [`no_enum_case`].
pub const FAULT_NO_ENUM_CASE_PREFIX: &str = "no case of enum ";

/// `panic("msg")`'s full body.
#[must_use]
pub fn panic_with(msg: &str) -> String {
    format!("{FAULT_PANIC_PREFIX}{msg}")
}

/// `assert(cond, "msg")`'s full body. An empty `msg` degrades to the bare [`FAULT_ASSERT`], which is
/// what `assert(cond)` with no message produces — one function so the two forms cannot drift apart.
#[must_use]
pub fn assert_with(msg: &str) -> String {
    if msg.is_empty() {
        FAULT_ASSERT.to_string()
    } else {
        format!("{FAULT_ASSERT}: {msg}")
    }
}

/// The absent-field body. Backtick-quoted on both names, which is what makes the VM's line-prefixed
/// render and the interpreter's prefix-less one classify to the same kind by substring.
#[must_use]
pub fn no_field(field: &str, class: &str) -> String {
    format!("{FAULT_NO_FIELD_PREFIX}`{field}` on `{class}`")
}

/// The no-matching-enum-case body. `shown` is the already-rendered offending value.
#[must_use]
pub fn no_enum_case(enum_name: &str, shown: &str) -> String {
    format!("{FAULT_NO_ENUM_CASE_PREFIX}`{enum_name}` has value {shown}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_payload_forms_compose_from_their_prefixes() {
        assert_eq!(panic_with("boom"), "panic: boom");
        // `assert(cond)` and `assert(cond, "why")` are ONE function so they cannot drift apart.
        assert_eq!(assert_with(""), "assertion failed");
        assert_eq!(assert_with("why"), "assertion failed: why");
        assert_eq!(no_field("n", "Counter"), "no field `n` on `Counter`");
        assert_eq!(
            no_enum_case("Color", "9"),
            "no case of enum `Color` has value 9"
        );
        // Each payload body must START with its prefix const — that is what lets the differential
        // harness classify by prefix alone (DEC-361's derivation).
        for (body, prefix) in [
            (panic_with("x"), FAULT_PANIC_PREFIX),
            (assert_with("x"), FAULT_ASSERT),
            (assert_with(""), FAULT_ASSERT),
            (no_field("a", "B"), FAULT_NO_FIELD_PREFIX),
            (no_enum_case("E", "1"), FAULT_NO_ENUM_CASE_PREFIX),
        ] {
            assert!(
                body.starts_with(prefix),
                "{body:?} must start with {prefix:?}"
            );
        }
    }

    /// DEC-361's anti-re-inlining ratchet: a fault body may appear as a string LITERAL only where it is
    /// defined. Everywhere else it must come through this module (or [`super::arith`], which Invariant 4
    /// names as the home of the arithmetic bodies).
    ///
    /// This is the half that stops the regression rather than merely fixing it. The bodies had been
    /// re-typed at fifteen-odd sites across the VM, the interpreter, the JIT and the transpiler, and
    /// nothing failed when they diverged — `"non-exhaustive match at runtime"` had ALREADY drifted on
    /// the PHP leg. A literal is now a test failure naming the file and line.
    #[test]
    fn no_backend_re_inlines_a_canonical_fault_body() {
        // Files allowed to contain a body literally: the definitions themselves, plus the `Core.Test`
        // assertion natives, whose `"assertion failed: …"` messages are that module's OWN surface
        // (a test-framework report, not the `assert()` intrinsic's fault) and are checked by its tests.
        const DEFINITIONS: &[&str] = &["value/faults.rs", "value/arith.rs", "ext/test/natives.rs"];
        // Bodies distinctive enough that a bare substring match cannot false-positive on prose. The
        // short prefixes (`panic: `, `no field `, `assertion failed`) are excluded on purpose: they
        // appear inside legitimate longer messages and inside doc comments describing them.
        const BODIES: &[&str] = &[
            FAULT_STACK_OVERFLOW,
            FAULT_INDEX_OOB,
            FAULT_RANGE_TOO_LARGE,
            FAULT_CHANNEL_EMPTY,
            FAULT_JOIN_INCOMPLETE,
            FAULT_NON_EXHAUSTIVE_MATCH,
            FAULT_FORCE_UNWRAP_NULL,
            FAULT_TODO,
            FAULT_UNREACHABLE,
            FAULT_INT_OVERFLOW,
            FAULT_DIV_ZERO,
            FAULT_MOD_ZERO,
            FAULT_DECIMAL_NONTERMINATING,
        ];
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut offenders = Vec::new();
        let mut scanned = 0usize;
        walk(&root, &mut |path, text| {
            let rel = path.to_string_lossy().replace('\\', "/");
            if DEFINITIONS.iter().any(|d| rel.ends_with(d)) {
                return;
            }
            // Test modules assert on rendered output; that is their job, not a re-inlined emitter.
            if rel.contains("/tests/") || rel.contains("tests.rs") || rel.contains("/tests_") {
                return;
            }
            scanned += 1;
            for (i, line) in text.lines().enumerate() {
                // An inline `#[cfg(test)] mod tests` at module level ends the emitter region: a test
                // asserting on a rendered body is doing its job, not re-typing an emitter.
                if line.starts_with("#[cfg(test)]") {
                    break;
                }
                let trimmed = line.trim_start();
                if trimmed.starts_with("//") {
                    continue; // prose naming a body is fine; emitting one is not
                }
                for body in BODIES {
                    if line.contains(&format!("\"{body}")) {
                        offenders.push(format!("{rel}:{}: {}", i + 1, line.trim()));
                    }
                }
            }
        });
        assert!(
            scanned > 100,
            "the scan only reached {scanned} files — it broke"
        );
        assert!(
            offenders.is_empty(),
            "{} site(s) re-inline a canonical fault body instead of using `value::faults` \
             (DEC-361 — a fault body is parity-affecting, Invariant 4):\n  {}",
            offenders.len(),
            offenders.join("\n  ")
        );
    }

    fn walk(dir: &std::path::Path, f: &mut impl FnMut(&std::path::Path, &str)) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        let mut paths: Vec<_> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
        paths.sort();
        for p in paths {
            if p.is_dir() {
                walk(&p, f);
            } else if p.extension().is_some_and(|x| x == "rs") {
                if let Ok(text) = std::fs::read_to_string(&p) {
                    f(&p, &text);
                }
            }
        }
    }
}
