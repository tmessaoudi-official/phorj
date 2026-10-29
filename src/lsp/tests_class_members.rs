//! Row 5o — §LSP-CLASS-QUALIFIED-MEMBERS: hover, go-to-definition and completion on a
//! CLASS-QUALIFIED member (`K.MAX`, `K.twice(1)`, `K.`) resolve against the named class and its
//! supertypes — constants, static fields and static methods, never an instance member. Before this
//! all three answered nothing, and a top-level function sharing the member's name was a decoy that
//! hover and definition would have reached instead. (A user enum's variants are written bare —
//! `Defend()`, `new Percent(10)` — so there is no `Enum.Variant` form for this row to resolve.)

use super::tests::did_open;
use super::*;

const SRC: &str = "package Main;
function twice(int n): int { return n; }
open class Base {
    public static function twice(int n): int { return n * 2; }
}
class K extends Base {
    public const int MAX = 3;
    public const Map<string, int> M = [\"a\" => 1];
    mutable static int count = 0;
    public function describe(): string { return \"k\"; }
}
function f(): int { K.count = 1; var m = K.M; return K.MAX + K.twice(1) + K::MAX; }
";

/// A `textDocument/<method>` request with the cursor `into` bytes past the start of the `nth`
/// occurrence of `needle` in [`SRC`].
fn req(method: &str, needle: &str, nth: usize, into: usize) -> Json {
    let at = SRC.match_indices(needle).nth(nth).expect("needle").0 + into;
    let line = SRC[..at].matches('\n').count();
    let col = at - SRC[..at].rfind('\n').map_or(0, |i| i + 1);
    Json::parse(&format!(
        r#"{{"id":7,"method":"textDocument/{method}","params":{{"textDocument":{{"uri":"file:///x.phg"}},"position":{{"line":{line},"character":{col}}}}}}}"#
    ))
    .unwrap()
}

fn ask(method: &str, needle: &str, nth: usize, into: usize) -> String {
    let mut s = Server::default();
    s.handle(&did_open("file:///x.phg", SRC));
    s.handle(&req(method, needle, nth, into))[0].clone()
}

/// The 0-based line of the first occurrence of `needle` in [`SRC`].
fn line_of(needle: &str) -> usize {
    SRC[..SRC.find(needle).expect("needle")]
        .matches('\n')
        .count()
}

#[test]
fn hover_on_a_class_qualified_constant_shows_its_declaration() {
    let body = ask("hover", "K.MAX", 0, 3);
    assert!(body.contains("public const int MAX = 3"), "{body}");
    let body = ask("hover", "K.M;", 0, 2);
    assert!(body.contains("public const Map<string, int> M"), "{body}");
}

#[test]
fn hover_on_a_static_field_and_an_inherited_static_method() {
    let body = ask("hover", "K.count", 0, 3);
    assert!(body.contains("mutable static int count"), "{body}");
    // The top-level `twice` is the decoy: the answer must be Base's STATIC method.
    let body = ask("hover", "K.twice", 0, 3);
    assert!(body.contains("public static function twice"), "{body}");
}

#[test]
fn hover_on_the_double_colon_form() {
    let body = ask("hover", "K::MAX", 0, 4);
    assert!(body.contains("public const int MAX = 3"), "{body}");
}

#[test]
fn definition_jumps_to_the_member_not_a_same_named_function() {
    let body = ask("definition", "K.MAX", 0, 3);
    let want = format!("\"start\":{{\"line\":{},", line_of("const int MAX"));
    assert!(body.contains(&want), "{want} in {body}");
    let body = ask("definition", "K.twice", 0, 3);
    let want = format!("\"start\":{{\"line\":{},", line_of("static function twice"));
    assert!(body.contains(&want), "{want} in {body}");
}

#[test]
fn an_instance_member_is_not_class_qualified() {
    // `K.describe` names an INSTANCE method — no class-qualified answer.
    let src = SRC.replace("K.count = 1;", "K.describe;");
    let mut s = Server::default();
    s.handle(&did_open("file:///x.phg", &src));
    let at = src.find("K.describe").unwrap() + 3;
    let line = src[..at].matches('\n').count();
    let col = at - src[..at].rfind('\n').unwrap() - 1;
    let body = &s.handle(&super::tests::req_at("hover", line as u32, col as u32))[0];
    assert!(body.ends_with("\"result\":null}"), "{body}");
}

#[test]
fn completion_after_a_class_name_lists_its_static_members() {
    let src = SRC.replace("function f(): int { ", "function f(): int {\n    K.\n    ");
    let offset = src.find("K.\n").unwrap() + 2;
    let resp =
        super::completion::complete(&src, offset, None, None, &std::collections::HashMap::new());
    let got: Vec<&str> = resp
        .split("\"label\":\"")
        .skip(1)
        .filter_map(|r| r.split('"').next())
        .collect();
    for want in ["MAX", "M", "count", "twice"] {
        assert!(got.contains(&want), "want {want} in {got:?}");
    }
    assert!(!got.contains(&"describe"), "instance method in {got:?}");
}

#[test]
fn completion_lists_a_trait_static_on_its_user_and_on_the_users_subclass() {
    // Plan row 4c item 5 (Invariant 17): `U.sf()` / `V.s` check clean, so `U.` and `V.` must list a
    // trait's static members — the chain walked only the classes' own declarations.
    let src = "package Main;
trait T { mutable static int s = 1; static function sf(): int { return 3; } }
open class U { use T; }
class V extends U {}
function f(): int {
    V.
    return 0; }
";
    let offset = src.find("V.\n").unwrap() + 2;
    let resp =
        super::completion::complete(src, offset, None, None, &std::collections::HashMap::new());
    let got: Vec<&str> = resp
        .split("\"label\":\"")
        .skip(1)
        .filter_map(|r| r.split('"').next())
        .collect();
    for want in ["s", "sf"] {
        assert!(got.contains(&want), "want {want} in {got:?}");
    }
}

/// Hover and definition on an inherited static name the NEAREST declaration — the checker's and the
/// backends' owner (`ast::class_lookup_order`), not the first ancestor by name (row 4c close panel,
/// completeness F2: `M.s` landed on `A` when `Z` redeclares it). Also pins a trait static on a
/// subclass of its user (F5).
#[test]
fn hover_and_definition_on_an_inherited_static_reach_the_nearest_declaration() {
    let src = "package Main;
open class A { static int s = 1; }
open class Z extends A { static int s = 2; }
class M extends Z {}
trait T { mutable static int ts = 7; }
open class U { use T; }
class V extends U {}
function f(): int { return M.s + V.ts; }
";
    let mut s = Server::default();
    s.handle(&did_open("file:///x.phg", src));
    let pos = |needle: &str, into: usize| {
        let at = src.find(needle).unwrap() + into;
        let line = src[..at].matches('\n').count();
        let col = at - src[..at].rfind('\n').unwrap() - 1;
        (line as u32, col as u32)
    };
    let (l, c) = pos("M.s ", 2);
    let hover = &s.handle(&super::tests::req_at("hover", l, c))[0];
    assert!(hover.contains("static int s = 2"), "nearest is Z: {hover}");
    let def = &s.handle(&super::tests::req_at("definition", l, c))[0];
    assert!(def.contains("\"start\":{\"line\":2,"), "Z's line: {def}");
    let (l, c) = pos("V.ts", 2);
    let def = &s.handle(&super::tests::req_at("definition", l, c))[0];
    assert!(def.contains("\"start\":{\"line\":4,"), "T's static: {def}");
}
