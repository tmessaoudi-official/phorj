use super::*;

// --- public-surface file-naming rule (E-FILE-*) -----------------------

#[test]
fn public_type_in_mismatched_file_is_rejected() {
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\nimport Ui.Widget;\n\
         #[Entry(kind: EntryKind.Cli)] function main() -> void { Widget w = Widget(); }",
    );
    // A file declaring one public type must be named after it; `widget.phg` ≠ `Widget`.
    tmp.write(
        "src/Ui/widget.phg",
        "package Ui;\npublic class Widget { constructor() {} }",
    );
    let err = load(&entry).unwrap_err();
    assert!(err.contains("E-FILE-NAME"), "got: {err}");
}

#[test]
fn public_type_in_matching_file_is_allowed() {
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\n#[Entry(kind: EntryKind.Cli)] function main() -> void { Widget w = Widget(); }",
    );
    tmp.write(
        "src/Widget.phg",
        "package Main;\npublic class Widget { constructor() {} }",
    );
    assert!(load(&entry).is_ok());
}

#[test]
fn two_public_types_in_one_file_is_rejected() {
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\nimport Lib.A;\n\
         #[Entry(kind: EntryKind.Cli)] function main() -> void { A a = A(); }",
    );
    tmp.write(
        "src/Lib/A.phg",
        "package Lib;\npublic class A { constructor() {} }\npublic class B { constructor() {} }",
    );
    let err = load(&entry).unwrap_err();
    assert!(err.contains("E-FILE-MULTI-PUBLIC"), "got: {err}");
}

#[test]
fn public_type_plus_public_fn_in_one_file_is_rejected() {
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\nimport Lib.Box;\n\
         #[Entry(kind: EntryKind.Cli)] function main() -> void { Box b = Box(); }",
    );
    tmp.write(
        "src/Lib/Box.phg",
        "package Lib;\npublic class Box { constructor() {} }\npublic function helper() -> int { return 1; }",
    );
    let err = load(&entry).unwrap_err();
    assert!(err.contains("E-FILE-MIXED-PUBLIC"), "got: {err}");
}

#[test]
fn private_helper_type_rides_along_in_a_type_module() {
    // A type module may carry private/internal helper types + functions — they ride along free.
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\n#[Entry(kind: EntryKind.Cli)] function main() -> void { Widget w = Widget(); }",
    );
    tmp.write(
        "src/Widget.phg",
        "package Main;\npublic class Widget { constructor() {} }\nprivate class Cache { constructor() {} }\nprivate function tweak() -> int { return 1; }",
    );
    assert!(load(&entry).is_ok(), "private helpers should ride along");
}

#[test]
fn main_file_with_multiple_public_types_is_exempt() {
    let tmp = TempDir::new();
    // The entry file declares `main` → exempt: multiple public types + functions are fine, any name.
    let entry = tmp.write(
        "src/main.phg",
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\npublic class A { constructor() {} }\npublic class B { constructor() {} }\n#[Entry(kind: EntryKind.Cli)] function main() -> void { A a = A(); B b = B(); }",
    );
    assert!(
        load(&entry).is_ok(),
        "a main file is exempt from the public-surface rule"
    );
}

#[test]
fn forward_and_cross_file_type_references_resolve() {
    // Order-independence (the prebind pre-pass): `Order` references `OrderLine`, which sorts/merges
    // AFTER it — and a forward reference within the entry file — both resolve.
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\n#[Entry(kind: EntryKind.Cli)] function main() -> void { B b = makeB(); }\nfunction makeB() -> B { return B(7); }",
    );
    // Two library files; `Order.phg` (merged first, alphabetically) references `OrderLine` (later).
    tmp.write(
        "src/Acme/Lib/Order.phg",
        "package Acme.Lib;\npublic class Order { constructor(public OrderLine line) {} }",
    );
    tmp.write(
        "src/Acme/Lib/OrderLine.phg",
        "package Acme.Lib;\npublic class OrderLine { constructor(public int qty) {} }",
    );
    tmp.write(
        "src/B.phg",
        "package Main;\npublic class B { constructor(public int x) {} }",
    );
    assert!(
        load(&entry).is_ok(),
        "forward + cross-file type refs must resolve"
    );
}
