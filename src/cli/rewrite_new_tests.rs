use super::cmd_rewrite_new;

/// Audit A2 (CD-32): the tool's walk skipped tuples, named arguments and `test` bodies, so a bare
/// construction in any of them kept no `new` and the migrated file then failed `E-NEW-REQUIRED`.
#[test]
fn constructions_in_tuples_named_args_and_test_bodies_get_new() {
    let dir = std::env::temp_dir().join(format!("phorj_rewrite_new_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("m.phg");
    std::fs::write(
        &path,
        "package Main;\n\
         class Box {\n    constructor(public int v) {}\n}\n\
         function take(Box b): int {\n    return b.v;\n}\n\
         function main(): void {\n    var (a, n) = (Box(4), 2);\n    int k = take(b: Box(5));\n}\n\
         test \"boxes\" {\n    var c = Box(6);\n}\n",
    )
    .unwrap();
    let report = cmd_rewrite_new(path.to_str().unwrap()).unwrap();
    let out = std::fs::read_to_string(&path).unwrap();
    for want in [
        "(new Box(4), 2)",
        "take(b: new Box(5))",
        "var c = new Box(6);",
    ] {
        assert!(out.contains(want), "missing `{want}` ({report}):\n{out}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
