//! PHP builtin interfaces a lifted class may `implements` that phorj has no counterpart for.
//!
//! Left in the draft, `class Bag implements Countable` fails `phg check` with `E-IFACE-IMPL` ("not an
//! interface"): phorj's `implements` lists DECLARED interfaces only, and none of these is one. The
//! lifter's contract is that what it cannot do is refused loudly, so each is dropped from the header and
//! named in a `// CANNOT LIFT:` note — except `Stringable`, whose whole meaning is `__toString`, which
//! the lifter already turns into `#[ToString]`, so dropping it loses nothing and needs no note.
//!
//! A user cannot declare an interface with one of these names (PHP refuses the redeclaration), so the
//! match is by name alone: case-insensitive, a leading `\` ignored.

use crate::lift::ast as php;

/// PHP's always-loaded builtin interfaces (lowercased) that a user class commonly implements.
const BUILTIN_INTERFACES: &[&str] = &[
    "countable",
    "stringable",
    "jsonserializable",
    "iteratoraggregate",
    "iterator",
    "traversable",
    "arrayaccess",
    "serializable",
];

/// Is `name` one of [`BUILTIN_INTERFACES`]?
pub(in crate::lift::lifter) fn is_builtin_interface(name: &str) -> bool {
    let bare = name.trim_start_matches('\\').to_ascii_lowercase();
    BUILTIN_INTERFACES.contains(&bare.as_str())
}

/// `Stringable` — dropped silently, because `__toString` lifts to `#[ToString]`.
fn is_stringable(name: &str) -> bool {
    name.trim_start_matches('\\')
        .eq_ignore_ascii_case("stringable")
}

/// The `// CANNOT LIFT:` notes: one per class, naming every dropped interface in declaration order
/// (Invariant 10 — a lifted draft must not vary run to run). Classes without a droppable interface, and a
/// class whose only builtin is `Stringable`, produce nothing.
pub(in crate::lift::lifter) fn dropped_interface_notes(prog: &php::PhpProgram) -> String {
    let mut out = String::new();
    for item in &prog.items {
        let php::PhpItem::Class(c) = item else {
            continue;
        };
        let dropped: Vec<&str> = c
            .implements
            .iter()
            .map(String::as_str)
            .filter(|n| is_builtin_interface(n) && !is_stringable(n))
            .collect();
        if dropped.is_empty() {
            continue;
        }
        let names = dropped
            .iter()
            .map(|n| format!("`{}`", canonical(n)))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!(
            "// CANNOT LIFT: `{}` implements the PHP builtin interface {names}, which phorj has no \
             counterpart for — `implements` lists declared interfaces only, so it was dropped from the \
             header. Port the contract as a declared interface if callers rely on it.\n",
            c.name
        ));
    }
    out
}

/// The interface's canonical spelling, so the note reads `Countable` however the PHP wrote it.
fn canonical(name: &str) -> &'static str {
    match name.trim_start_matches('\\').to_ascii_lowercase().as_str() {
        "countable" => "Countable",
        "stringable" => "Stringable",
        "jsonserializable" => "JsonSerializable",
        "iteratoraggregate" => "IteratorAggregate",
        "iterator" => "Iterator",
        "traversable" => "Traversable",
        "arrayaccess" => "ArrayAccess",
        _ => "Serializable",
    }
}
