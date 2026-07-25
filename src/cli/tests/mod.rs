//! CLI unit tests, split by tested topic (M-Decomp, Invariant 13). Shared fixtures
//! (`wp`, `SAMPLE`, `rest`) live here; each sibling submodule pulls the cli surface via
//! `use super::super::*;` and the fixtures it needs via `use super::{...};`.

mod backends;
mod execution;
mod explain_coverage;
mod explain_ratchet;
mod foreach;
mod imports_sugar;
mod source_resolution;
mod transpile_lift;

/// Prepend the reserved `package Main; import Core.Runtime.Entry;` (M5 S1: every file is packaged, never inferred) unless
/// already declared, so the CLI command tests need no per-case package boilerplate. The segment
/// carries no newline, so line numbers in fault diagnostics are preserved.
fn wp(src: &str) -> String {
    // DEC-191: the run-path tests need an attributed entry — inject `#[Entry]` before a bare
    // `function main(` so the ~1000 inline programs don't each repeat the ceremony. A test that
    // writes its own `#[Entry]` (or has no main) is passed through untouched.
    let src = if src.contains("function main(") && !src.contains("#[Entry") {
        src.replacen(
            "function main(",
            "#[Entry(kind: EntryKind.Cli)] function main(",
            1,
        )
    } else {
        src.to_string()
    };
    let src = if src.trim_start().starts_with("package ") {
        src
    } else {
        format!("package Main; {src}")
    };
    // DEC-191 addendum: the attribute is import-gated — inject its import once too, AFTER the
    // package segment (imports may not precede `package`); same-line, preserving line numbers.
    // DEC-337: `kind: EntryKind.Cli` is import-gated too — inject the `EntryKind` import alongside.
    if src.contains("#[Entry") {
        // Inject whichever of the two entry imports the source doesn't already declare. Match the
        // exact `import …;` statement, NOT a bare substring: `Core.Runtime.Entry` is a prefix of
        // `Core.Runtime.EntryKind`, so a substring test misfires on an EntryKind-only source
        // (would skip the needed `Core.Runtime.Entry` import → spurious E-UNIMPORTED). All current
        // inline sources carry neither import, so both are injected — behaviour unchanged for them.
        let mut inject = String::new();
        if !src.contains("import Core.Runtime.Entry;") {
            inject.push_str(" import Core.Runtime.Entry;");
        }
        if !src.contains("import Core.Runtime.EntryKind;") {
            inject.push_str(" import Core.Runtime.EntryKind;");
        }
        if inject.is_empty() {
            src
        } else {
            let i = src.find(';').expect("package decl ends with ;");
            format!("{}{}{}", &src[..=i], inject, &src[i + 1..])
        }
    } else {
        src
    }
}

const SAMPLE: &str = r#"package Main;
import Core.Runtime.Entry; import Core.Runtime.EntryKind;
import Core.Output;

enum Shape {
    Circle(float radius),
    Rect(float w, float h),
}

function area(Shape s): float {
    return match s {
        Circle(r)  => 3.14159 * r * r,
        Rect(w, h) => w * h,
    };
}

class Greeter {
    private string name;
    constructor(private string name) {}
    function greet(): string { return "Hello {this.name}"; }
}

#[Entry(kind: EntryKind.Cli)]
function main(): void {
    Greeter g = new Greeter("Tak");
    Output.printLine(g.greet());
    List<Shape> shapes = [new Circle(2.0), new Rect(3.0, 4.0)];
    for (Shape s in shapes) {
        Output.printLine("area = {area(s)}");
    }
}
"#;

fn rest(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|s| (*s).to_string()).collect()
}
