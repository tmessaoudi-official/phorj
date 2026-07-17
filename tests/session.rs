//! `Core.SessionModule` (W3, TOP-20 #3) end-to-end fixture — sessions over synthetic `Core.Http`
//! requests on BOTH backends (the `tests/db.rs` pattern; `Core.Native.Session` is impure →
//! quarantined from the byte-identity differential). The store internals (open/reuse, idle
//! expiry never resurrecting ids, regenerate moving data) are unit-tested in
//! `src/native/session.rs`; THIS file proves the phorj-visible story: cookie round-trip,
//! persistence across requests, fixation defense, secure cookie attributes.

use phorj::cli::{cmd_run, cmd_transpile, cmd_treewalk};

fn both(src: &str, expected: &str) {
    let tree = cmd_treewalk(src).expect("program runs on the interpreter");
    assert_eq!(tree, expected, "interpreter output");
    assert_eq!(
        cmd_run(src).expect("program runs on the VM"),
        tree,
        "run ≡ runvm"
    );
}

/// The full session story in one deterministic program: request #1 arrives with no cookie → a
/// fresh session; the handler stores a value and applies the cookie; request #2 arrives carrying
/// that cookie → the SAME session with the value intact; regenerate changes the id but keeps the
/// data; the applied Set-Cookie carries the security attributes.
#[test]
fn session_cookie_round_trip_persistence_and_fixation_defense() {
    let src = r#"package Main;
import Core.Output;
import Core.String;
import Core.Bytes;
import Core.SessionModule;
import Core.SessionModule.Session;
import Core.Http;
import Core.Http.Request;
import Core.Http.Response;

function firstRequest(): string {
  Request? r = Request.parse(Bytes.fromString("GET / HTTP/1.1\r\nHost: x\r\n\r\n"));
  if (var req = r) {
    Session s = Session.start(req);
    s.set("user", "ada");
    Response resp = s.apply(Response.text(200, "hi"));
    // The applied cookie carries the secure attributes.
    for (string h in resp.headerLines) {
      if (String.startsWith(h, "Set-Cookie:")) {
        Output.printLine("httponly {String.contains(h, "HttpOnly")} samesite {String.contains(h, "SameSite=Lax")}");
      }
    }
    return s.id();
  }
  return "";
}

function secondRequest(string sid): void {
  string raw = "GET / HTTP/1.1\r\nHost: x\r\nCookie: theme=dark; phorjsid={sid}\r\n\r\n";
  Request? r = Request.parse(Bytes.fromString(raw));
  if (var req = r) {
    Session s = Session.start(req);
    Output.printLine("resumed {s.id() == sid}");
    Output.printLine("user {s.get("user") ?? "<absent>"}");
    // Fixation defense: regenerate → new id, same data, old id dead.
    string old = s.id();
    discard s.regenerate();
    Output.printLine("regenerated {s.id() == old} keeps {s.get("user") ?? "<absent>"}");
    s.destroy();
    Output.printLine("destroyed {s.get("user") ?? "<absent>"}");
  }
}

function main(): void {
  string sid = firstRequest();
  Output.printLine("sid32 {String.length(sid) == 32}");
  secondRequest(sid);
}
"#;
    both(
        src,
        "httponly true samesite true\nsid32 true\nresumed true\nuser ada\nregenerated false keeps ada\ndestroyed <absent>\n",
    );
}

/// An unknown/expired cookie id silently gets a FRESH EMPTY session (never resurrected, never an
/// error — the visitor-with-a-stale-cookie path).
#[test]
fn session_unknown_cookie_id_gets_a_fresh_session() {
    let src = r#"package Main;
import Core.Output;
import Core.String;
import Core.Bytes;
import Core.SessionModule;
import Core.SessionModule.Session;
import Core.Http;
import Core.Http.Request;

function main(): void {
  string raw = "GET / HTTP/1.1\r\nHost: x\r\nCookie: phorjsid=deadbeefdeadbeefdeadbeefdeadbeef\r\n\r\n";
  Request? r = Request.parse(Bytes.fromString(raw));
  if (var req = r) {
    Session s = Session.start(req);
    Output.printLine("fresh {s.id() == "deadbeefdeadbeefdeadbeefdeadbeef"}");
    Output.printLine("empty {s.get("user") ?? "<absent>"}");
  }
}
"#;
    both(src, "fresh false\nempty <absent>\n");
}

/// THE LADDER RULE (for-now form): `Core.SessionModule` transpile is the clean E-TRANSPILE-SESSION.
#[test]
fn session_transpile_is_a_clean_ladder_error() {
    let src = r#"package Main;
import Core.Output;
import Core.SessionModule;
function main(): void { Output.printLine("x"); }
"#;
    match cmd_transpile(src) {
        Ok(php) => panic!("expected E-TRANSPILE-SESSION, got PHP: {php:?}"),
        Err(e) => {
            assert!(e.contains("E-TRANSPILE-SESSION"), "{e}");
            assert!(!e.contains("E-UNKNOWN-IDENT"), "{e}");
        }
    }
}
