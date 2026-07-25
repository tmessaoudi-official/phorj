use super::*;

// --- loose mode --------------------------------------------------------

#[test]
fn loose_main_is_accepted() {
    let u = load_loose_src(
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\n#[Entry(kind: EntryKind.Cli)] function main() -> void {}",
    )
    .unwrap();
    assert_eq!(u.program.package, ["Main"]);
    assert_eq!(
        u.diag_src,
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\n#[Entry(kind: EntryKind.Cli)] function main() -> void {}"
    );
}

#[test]
fn loose_non_main_is_rejected() {
    let err = load_loose_src("package app.util;\nfunction f() -> void {}").unwrap_err();
    assert!(err.contains("cannot run from stdin/-e"), "got: {err}");
}

#[test]
fn loose_empty_package_defers_to_checker() {
    // No package decl — loader stays silent (checker reports E-NO-PACKAGE downstream).
    let u = load_loose_src("#[Entry(kind: EntryKind.Cli)] function main() -> void {}").unwrap();
    assert!(u.program.package.is_empty());
}

#[test]
fn loose_load_has_no_stats() {
    let u = load_loose_src(
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\n#[Entry(kind: EntryKind.Cli)] function main() -> void {}",
    )
    .unwrap();
    assert!(u.stats.is_none(), "loose mode reports no project stats");
}
