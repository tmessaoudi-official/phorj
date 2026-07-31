//! The printer's ENTRY POINTS and state struct, split out of `items.rs` (Invariant 13 — that file hit
//! the hard cap when DEC-419's doc plumbing landed). Pure movement: no printing logic here.

use crate::ast::Program;

pub fn print_program(p: &Program) -> Result<String, String> {
    print_program_with_docs(p, &std::collections::BTreeMap::new())
}

/// [`print_program`] plus PHPDoc recovered from the lifted PHP (DEC-419), keyed by declaration name —
/// each becomes a phorj `/** … */` doc comment above its declaration. `print_program` is this with an
/// empty map.
pub fn print_program_with_docs(
    p: &Program,
    docs: &std::collections::BTreeMap<String, String>,
) -> Result<String, String> {
    let mut pr = Printer {
        out: String::new(),
        indent: 0,
        docs: docs.clone(),
    };
    pr.program(p)?;
    Ok(pr.out)
}

pub(crate) struct Printer {
    pub(super) out: String,
    pub(super) indent: usize,
    /// Declaration name → doc body (DEC-419). Empty for every caller that has no PHP source behind it.
    pub(super) docs: std::collections::BTreeMap<String, String>,
}
