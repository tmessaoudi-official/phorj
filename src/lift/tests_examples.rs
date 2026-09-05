//! The shipped `examples/lift/*.php` / `*.phg` pairs are the lifter's OUTPUT, byte for byte — a
//! pair that drifts silently would document a lift the tool no longer produces. Before this test
//! (Lane R-5, 2026-09-05) nothing checked the pairs: the `.phg` side ran under the differential
//! glob, and the `.php` side was never lifted again. Hand-finished pairs are exempt BY NAME with
//! the reason, so an exemption is a visible decision rather than a missing file.

use std::path::Path;

/// `.php` files whose `.phg` sibling was hand-finished after lifting, with why.
const HAND_FINISHED: &[(&str, &str)] = &[(
    "errors.php",
    "KNOWN_ISSUES §LIFT-THROWS: the `throws` clauses and the second `catch` are added by hand",
)];

#[test]
fn every_shipped_lift_pair_is_the_lifters_own_output() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/lift");
    let mut checked = 0;
    let mut entries: Vec<_> = std::fs::read_dir(&dir)
        .expect("examples/lift exists")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "php"))
        .collect();
    entries.sort();
    for php in entries {
        let name = php.file_name().unwrap().to_string_lossy().to_string();
        let phg = php.with_extension("phg");
        assert!(phg.exists(), "{name} has no .phg sibling");
        if let Some((_, why)) = HAND_FINISHED.iter().find(|(n, _)| *n == name) {
            eprintln!("exempt {name}: {why}");
            continue;
        }
        let src = std::fs::read_to_string(&php).unwrap();
        // Exactly what `phg lift` writes — banner and canonical formatting included. Comparing
        // against the raw library call instead let the pair gate pass while the repo format sweep
        // failed on the same files, each certain the other's shape was wrong.
        let lifted =
            crate::cli::cmd_lift(&src).unwrap_or_else(|e| panic!("{name} no longer lifts: {e}"));
        let expected = std::fs::read_to_string(&phg).unwrap();
        // Both sides now come from `cmd_lift`, so a banner the formatter dropped or moved would
        // agree with itself and every shipped draft would lose its "verify" disclaimer silently.
        // This is the only thing in the repo that pins the banner to the FIRST line of the output.
        assert!(
            expected.starts_with(crate::lift::LIFT_BANNER),
            "{name}: the lifted draft no longer opens with the verify banner"
        );
        assert_eq!(lifted.trim_end(), expected.trim_end(), "{name}: the shipped .phg is not the lifter's output — re-run `phg lift {name} > {}` or exempt it by name", phg.file_name().unwrap().to_string_lossy());
        checked += 1;
    }
    assert!(
        checked >= 4,
        "the pair gate ran on {checked} pairs — it must not go vacuous"
    );
}
