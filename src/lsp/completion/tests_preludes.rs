//! INSTANCE-member completion on a **prelude** class (`ServeConfig cfg` → `cfg.`), S3.3e.
//!
//! The gap this closes was documented twice in the source and never measured: `catalog::class_members`
//! only ever looked at the USER program, so a receiver whose declared type is a stdlib class — every
//! `Core.Http` type, `Date`/`Instant`/`Uri`, `Session`, … — completed to NOTHING. Invariant 17's 100%
//! rule makes that an incomplete feature: the compiler knows `cfg.port`, the editor did not.
//!
//! Split into its own file rather than grown onto `completion/tests.rs`, which sits at 439 lines
//! against Invariant 13's 500-line hard cap.
use super::complete;

/// Extract every `"label":"…"` value from a completion response (assert on CONTENT, not just count).
/// Local copy — `tests.rs`'s helper is private to that module, and that file must not grow.
fn labels(resp: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = resp;
    while let Some(i) = rest.find("\"label\":\"") {
        rest = &rest[i + 9..];
        if let Some(end) = rest.find('"') {
            out.push(rest[..end].to_string());
            rest = &rest[end..];
        }
    }
    out
}

/// Complete at the offset just past the LAST occurrence of `anchor` in `src`.
fn labels_after(src: &str, anchor: &str) -> Vec<String> {
    let offset = src.rfind(anchor).expect("anchor present") + anchor.len();
    labels(&complete(
        src,
        offset,
        None,
        None,
        &std::collections::HashMap::new(),
    ))
}

#[test]
fn prelude_class_instance_members_surface_for_a_bare_declared_type() {
    // The DEC-455.3 shape: `Http.ServeConfig`'s promoted ctor params are its instance fields.
    let src = "package Main;\n\
               import Core.Http;\n\
               function main(): void {\n  ServeConfig cfg = new ServeConfig();\n  cfg.\n}\n";
    let got = labels_after(src, "  cfg.");
    for want in [
        "host",
        "port",
        "workers",
        "timeout",
        "cert",
        "key",
        "serverName",
        "maxBodySize",
        "tlsMinVersion",
        "requestParsing",
    ] {
        assert!(got.iter().any(|l| l == want), "want {want} in {got:?}");
    }
}

#[test]
fn prelude_class_instance_members_surface_for_the_qualified_spelling() {
    // `Http.ServeConfig cfg` is the spelling DEC-331 D4's §1 surface writes. The declared type's
    // LEAF is what names the class, so both spellings must reach the same member list.
    let src = "package Main;\n\
               import Core.Http;\n\
               function main(): void {\n  Http.ServeConfig cfg = new Http.ServeConfig();\n  cfg.\n}\n";
    let got = labels_after(src, "  cfg.");
    for want in ["host", "port", "workers", "requestParsing"] {
        assert!(
            got.iter().any(|l| l == want),
            "qualified spelling: want {want} in {got:?}"
        );
    }
}

#[test]
fn prelude_instance_members_hide_private_fields_and_static_methods() {
    // `Request` is the class that makes both filters load-bearing: its wire internals are PRIVATE
    // promoted ctor params (real members, so a naive `collect_members` would offer them), and
    // `parse`/`fake` are STATICS — `req.parse(…)` is not a call anyone can write. Offering either
    // advertises surface that does not exist, which is worse than offering nothing (the same
    // reasoning `prelude_class_statics` applies to `private static` helpers).
    let src = "package Main;\n\
               import Core.Http;\n\
               function main(): void {\n  Request req = Request.fake(\"GET\", \"/\");\n  req.\n}\n";
    let got = labels_after(src, "  req.");
    for want in ["method", "path", "headers", "query", "body", "withHeader"] {
        assert!(got.iter().any(|l| l == want), "want {want} in {got:?}");
    }
    for hidden in [
        "rawTarget",
        "rawHeaderLines",
        "rawBody",
        "parse",
        "fake",
        "guardHeaderText",
        "rebuild",
    ] {
        assert!(
            !got.iter().any(|l| l == hidden),
            "internal/static `{hidden}` must NOT be offered as an instance member: {got:?}"
        );
    }
}

#[test]
fn the_serve_receiver_offers_its_static_and_nothing_else() {
    // `Http.serve` is a prelude STATIC on a class whose name equals its own module qualifier
    // (DEC-331 D5). It was believed to complete "because `Http` is in `bare_types`" — an inference
    // no test asserted, which is exactly the shape of claim DEC-348's row overstated. Pin it.
    let src = "package Main;\nimport Core.Http;\nfunction main(): void {\n  Http.\n}\n";
    let got = labels_after(src, "  Http.");
    assert!(
        got.iter().any(|l| l == "serve"),
        "`Http.serve` must be offered: {got:?}"
    );
    // The Core.Http row's TYPES (`Request`, `ServeConfig`, …) are not members of `class Http` — they
    // are reached by `import Core.Http.<Type>;`, and offering them here would advertise
    // `Http.Request()` as a call. Only the class's own public statics belong on this receiver.
    for hidden in ["Request", "Response", "ServeConfig", "Router"] {
        assert!(
            !got.iter().any(|l| l == hidden),
            "type `{hidden}` is not a member of `class Http`: {got:?}"
        );
    }
}

#[test]
fn a_user_class_still_shadows_a_prelude_class_of_the_same_name() {
    // The user program is consulted FIRST. A project that declares its own `Response` must complete
    // to ITS members — the prelude lookup is a fallback for names the buffer does not declare, never
    // an override of one it does.
    let src = "package Main;\n\
               import Core.Http;\n\
               class Response { public int mine = 1; }\n\
               function main(): void {\n  Response r = new Response();\n  r.\n}\n";
    let got = labels_after(src, "  r.");
    assert!(got.iter().any(|l| l == "mine"), "user member: {got:?}");
    assert!(
        !got.iter().any(|l| l == "serialize"),
        "prelude members must not be merged into a user class of the same name: {got:?}"
    );
}
