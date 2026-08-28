//! Tests: `phg explain` self-documentation coverage per diagnostic family.
use super::super::*;

#[test]
fn explain_covers_shadow_import_code() {
    // The M3 Wave 1 shadowing diagnostic is self-documenting via `phg explain`.
    let body = explain_text("E-SHADOW-IMPORT").expect("E-SHADOW-IMPORT has an explanation");
    assert!(body.contains("module qualifier"), "{body}");
}

#[test]
fn explain_covers_totality_codes() {
    // The M-RT totality cluster diagnostics are self-documenting via `phg explain`.
    for code in [
        "E-MISSING-RETURN",
        "E-NEVER-RETURN",
        "W-UNREACHABLE",
        "W-MATCH-UNREACHABLE",
    ] {
        let body = explain_text(code).unwrap_or_else(|| panic!("{code} has an explanation"));
        assert!(body.starts_with(code), "{body}");
    }
}

#[test]
fn explain_covers_main_signature_code() {
    // Batch-1 B: the entry-point signature diagnostic self-documents via `phg explain`.
    let body = explain_text("E-MAIN-SIGNATURE").expect("E-MAIN-SIGNATURE has an explanation");
    assert!(body.starts_with("E-MAIN-SIGNATURE"), "{body}");
    assert!(body.contains("exit code"), "{body}");
}

#[test]
fn explain_covers_test_outside_tests_code() {
    // M-Test: the test-block placement diagnostic self-documents via `phg explain`.
    let body =
        explain_text("E-TEST-OUTSIDE-TESTS").expect("E-TEST-OUTSIDE-TESTS has an explanation");
    assert!(body.starts_with("E-TEST-OUTSIDE-TESTS"), "{body}");
    assert!(body.contains("phg test"), "{body}");
}

#[test]
fn explain_covers_member_visibility_codes() {
    // Wave 1.1 member-visibility diagnostics self-document via `phg explain`.
    for code in ["E-FIELD-VISIBILITY", "E-METHOD-VISIBILITY"] {
        let body = explain_text(code).unwrap_or_else(|| panic!("{code} has an explanation"));
        assert!(body.starts_with(code), "{body}");
    }
}

#[test]
fn explain_covers_struct_pattern_codes() {
    // The pattern-cluster S5.2 struct-destructuring diagnostics self-document via `phg explain`.
    for code in [
        "E-STRUCT-PAT-TYPE",
        "E-STRUCT-FIELD-UNKNOWN",
        "E-PATTERN-DUP-BIND",
    ] {
        let body = explain_text(code).unwrap_or_else(|| panic!("{code} has an explanation"));
        assert!(body.starts_with(code), "{body}");
    }
}

#[test]
fn explain_covers_destructuring_codes() {
    // The Phase 1 slice 5 let-destructuring diagnostics self-document via `phg explain`.
    for code in [
        "E-DESTRUCTURE-TYPE",
        "E-DESTRUCTURE-NOT-CLASS",
        "E-DESTRUCTURE-FIELD-UNKNOWN",
        "E-DESTRUCTURE-NOT-LIST",
        "E-DESTRUCTURE-NEEDS-ELSE",
        "E-DESTRUCTURE-ELSE-IRREFUTABLE",
        "E-DESTRUCTURE-ELSE-FALLTHROUGH",
        "E-DESTRUCTURE-DUP-BIND",
        "E-FIXEDLIST-DESTRUCTURE-LEN",
    ] {
        let body = explain_text(code).unwrap_or_else(|| panic!("{code} has an explanation"));
        assert!(body.starts_with(code), "{body}");
    }
}

#[test]
fn explain_covers_error_model_2a_codes() {
    // The M-faults Slice 2a diagnostics (`?` propagation + fault intrinsics) self-document.
    for code in [
        "E-PROPAGATE-POSITION",
        "E-PROPAGATE-CONTEXT",
        "E-PROPAGATE-ERR",
        "E-RESERVED-INTRINSIC",
        "E-INTRINSIC-LITERAL",
    ] {
        let body = explain_text(code).unwrap_or_else(|| panic!("{code} has an explanation"));
        assert!(body.starts_with(code), "{body}");
    }
}

#[test]
fn explain_covers_error_model_2b_codes() {
    // The M-faults 2b exception codes are self-documenting via `phg explain`.
    for code in [
        "E-THROW-TYPE",
        "E-THROW-UNDECLARED",
        "E-CALL-UNHANDLED",
        "E-UNCAUGHT-THROW",
        "E-THROWS-TOO-BROAD",
        "E-CATCH-TYPE",
        "W-CATCH-UNREACHABLE",
    ] {
        let body = explain_text(code).unwrap_or_else(|| panic!("{code} has an explanation"));
        assert!(body.starts_with(code), "{body}");
    }
}

#[test]
fn explain_covers_m5_package_codes() {
    // The M5 S1 package diagnostics are self-documenting via `phg explain`.
    let np = explain_text("E-NO-PACKAGE").expect("E-NO-PACKAGE has an explanation");
    assert!(np.contains("package Main"), "{np}");
    let rp = explain_text("E-RESERVED-PACKAGE").expect("E-RESERVED-PACKAGE has an explanation");
    assert!(rp.contains("standard library"), "{rp}");
}

#[test]
fn explain_covers_visibility_codes() {
    // The declaration-visibility diagnostics are self-documenting via `phg explain`.
    let p = explain_text("E-VIS-PRIVATE").expect("E-VIS-PRIVATE has an explanation");
    assert!(p.contains("`private`") && p.contains(".phg"), "{p}");
    let i = explain_text("E-VIS-INTERNAL").expect("E-VIS-INTERNAL has an explanation");
    assert!(i.contains("`internal`") && i.contains("package"), "{i}");
}

#[test]
fn explain_covers_entry_kind_codes() {
    // DEC-331 D1 + DEC-337: the `#[Entry(kind: EntryKind.…)]` diagnostics self-document via `phg explain`.
    for code in [
        "E-ENTRY-KIND-REQUIRED",
        "E-ENTRY-KIND-UNKNOWN",
        "E-ENTRY-KIND-RESERVED",
        "E-ENTRY-SIG",
        "E-ENTRY-TARGET",
        "E-DUPLICATE-ENTRY-KIND",
        "E-INJECTED-VARIANT-BARE",
    ] {
        let body = explain_text(code).unwrap_or_else(|| panic!("{code} has an explanation"));
        assert!(body.starts_with(code), "{body}");
    }
    // E-ENTRY-KIND-UNKNOWN fires from TWO sites (walk.rs): a wrong qualifier (`Foo.Cli`) and an
    // unrecognized variant name (`EntryKind.Banana`) — the explain must describe BOTH (DEC-337).
    let unknown = explain_text("E-ENTRY-KIND-UNKNOWN").unwrap();
    assert!(
        unknown.contains("qualifier"),
        "must describe the wrong-qualifier emission path: {unknown}"
    );
    assert!(
        unknown.contains("Banana"),
        "must describe the unknown-variant-name emission path: {unknown}"
    );
}

#[test]
fn explain_covers_s8_trait_codes() {
    // M-RT S8: the trait diagnostics are self-documenting via `phg explain`.
    let u = explain_text("E-USE-UNKNOWN").expect("E-USE-UNKNOWN has an explanation");
    assert!(u.contains("trait") && u.contains("extends"), "{u}");
    let t = explain_text("E-USE-AS-TYPE").expect("E-USE-AS-TYPE has an explanation");
    assert!(t.contains("NOT a type") && t.contains("instanceof"), "{t}");
    let cc = explain_text("E-TRAIT-CTOR-COLLISION").expect("E-TRAIT-CTOR-COLLISION explained");
    assert!(cc.contains("constructor") && cc.contains("collide"), "{cc}");
    let sh = explain_text("W-TRAIT-CTOR-SHADOWED").expect("W-TRAIT-CTOR-SHADOWED explained");
    assert!(sh.contains("shadow") || sh.contains("wins"), "{sh}");
    let ps =
        explain_text("W-TRAIT-CTOR-PARENT-SKIPPED").expect("W-TRAIT-CTOR-PARENT-SKIPPED explained");
    assert!(ps.contains("parent") && ps.contains("auto-run"), "{ps}");
}

#[test]
fn explain_covers_mi_field_conflict_code() {
    // The M-RT S6c.1 field-collision diagnostic is self-documenting via `phg explain`.
    let body = explain_text("E-MI-FIELD-CONFLICT").expect("E-MI-FIELD-CONFLICT has an explanation");
    assert!(
        body.contains("insteadof") && body.contains("redeclar"),
        "{body}"
    );
}

#[test]
fn explain_covers_lambda_this_code() {
    // E-LAMBDA-THIS now covers only the field-initializer case (a method-body lambda may capture
    // `this`); the explanation is self-documenting via `phg explain`.
    let body = explain_text("E-LAMBDA-THIS").expect("E-LAMBDA-THIS has an explanation");
    assert!(
        body.contains("`this`") && body.contains("initializer"),
        "{body}"
    );
}

#[test]
fn explain_known_code_returns_paragraph_unknown_errors() {
    let ok = cmd_explain("E-UNKNOWN-IDENT").unwrap();
    assert!(ok.contains("E-UNKNOWN-IDENT"), "{ok}");
    assert!(ok.len() > 40, "explanation too short: {ok}");
    assert!(cmd_explain("E-NOPE").is_err());
}

#[test]
fn explain_covers_role_mismatch_code() {
    // DEC-331 S3.4: the wrong-verb diagnostic self-documents via `phg explain`, and the message it
    // renders points the reader there — a code with no entry fails `explain_ratchet` outright, so
    // this asserts the CONTENT the reader actually needs rather than mere existence.
    let body = explain_text(crate::cli::E_NO_ENTRY_FOR_ROLE).expect("E-NO-ENTRY-FOR-ROLE explains");
    assert!(body.starts_with(crate::cli::E_NO_ENTRY_FOR_ROLE), "{body}");
    for expected in ["phg run", "phg serve", "EntryKind.Cli", "EntryKind.Web"] {
        assert!(
            body.contains(expected),
            "explain must name {expected}: {body}"
        );
    }
    assert!(
        body.contains("E-SERVE-NO-HANDLER") && body.contains("E-ENTRY-KIND-RESERVED"),
        "and must distinguish itself from the two diagnoses it is NOT: {body}"
    );
}
