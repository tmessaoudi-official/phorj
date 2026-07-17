//! Built-in attributes resolve in EVERY "nothing in the wind" import form — the developer rule
//! (2026-07-18): a built-in like `Entry` works either (a) member-imported to the leaf then used
//! bare (`import Core.Runtime.Entry;` → `#[Entry]`, the RECOMMENDED surface the `examples/` use),
//! OR (b) written fully-qualified (`#[Core.Runtime.Entry]`, self-gating, no import). The qualified
//! form previously errored `E-UNKNOWN-ATTRIBUTE`; this is the regression guard. Recognition is
//! single-sourced in `Attribute::is_entry`/`is_route`/… via `ast::attr_path_matches`.

use phorj::ast::Attribute;
use phorj::cli::{cmd_run, cmd_treewalk};
use phorj::token::Span;

/// An attribute with the given name and no arguments (span irrelevant — recognizers read only `name`).
fn attr(name: &str) -> Attribute {
    Attribute {
        name: name.to_string(),
        args: vec![],
        span: Span {
            start: 0,
            len: 0,
            line: 0,
            col: 0,
        },
    }
}

/// Run the program on BOTH backends and assert they agree (run ≡ runvm); return the shared stdout.
fn run_both(src: &str) -> String {
    let tree = cmd_treewalk(src).expect("interpreter runs the program");
    let vm = cmd_run(src).expect("VM runs the program");
    assert_eq!(tree, vm, "run ≡ runvm");
    tree
}

#[test]
fn entry_fully_qualified_no_import_selects_the_entry_point() {
    // `#[Core.Runtime.Entry]` — fully qualified, self-gating: NO `import Core.Runtime.Entry;`.
    let src = r#"
package Main;
import Core.Output;

#[Core.Runtime.Entry]
function main(): void {
    Output.printLine("qualified-entry");
}
"#;
    assert_eq!(run_both(src), "qualified-entry\n");
}

#[test]
fn entry_bare_after_leaf_import_still_selects_the_entry_point() {
    // The RECOMMENDED form: member-import the leaf, then bare `#[Entry]`. Must keep working.
    let src = r#"
package Main;
import Core.Output;
import Core.Runtime.Entry;

#[Entry]
function main(): void {
    Output.printLine("bare-entry");
}
"#;
    assert_eq!(run_both(src), "bare-entry\n");
}

/// Boundary coverage of `attr_path_matches` through the public recognizers (advisor 6C note): every
/// built-in resolves in bare / partial / full-canonical form, and a non-segment-boundary suffix or a
/// foreign qualifier is rejected. The end-to-end tests above only exercise Entry; this pins Route,
/// UncheckedOverflow (the deep 4-segment path), the marker, and the DI built-ins in their qualified
/// forms plus the negative cases the matcher must reject.
#[test]
fn built_in_attributes_resolve_in_every_import_form_and_reject_non_boundaries() {
    // Entry — every form matches; non-segment suffix / foreign qualifier / trailing junk do not.
    assert!(attr("Entry").is_entry());
    assert!(attr("Runtime.Entry").is_entry());
    assert!(attr("Core.Runtime.Entry").is_entry());
    assert!(!attr("try").is_entry()); // ends_with("try") but no '.' boundary
    assert!(!attr("Foo.Entry").is_entry()); // foreign qualifier
    assert!(!attr("Entryx").is_entry());
    assert!(!attr("").is_entry());
    // Route.
    assert!(attr("Route").is_route());
    assert!(attr("Http.Route").is_route());
    assert!(attr("Core.Http.Route").is_route());
    assert!(!attr("Core.Runtime.Entry").is_route());
    // UncheckedOverflow — the deep canonical path, in every partial depth.
    assert!(attr("UncheckedOverflow").is_unchecked_overflow());
    assert!(attr("Integer.UncheckedOverflow").is_unchecked_overflow());
    assert!(attr("Runtime.Integer.UncheckedOverflow").is_unchecked_overflow());
    assert!(attr("Core.Runtime.Integer.UncheckedOverflow").is_unchecked_overflow());
    assert!(!attr("Overflow").is_unchecked_overflow());
    // Marker + DI built-ins, qualified.
    assert!(attr("Attribute").is_attribute_marker());
    assert!(attr("Core.Runtime.Attribute").is_attribute_marker());
    assert!(attr("Injectable").is_di_builtin());
    assert!(attr("Core.DependencyInjection.Injectable").is_di_builtin());
}
