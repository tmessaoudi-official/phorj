#![cfg(feature = "http-client")]
//! `Core.HttpClientModule` (W3-2) end-to-end fixture — the `tests/db.rs`/`tests/mail.rs` pattern.
//!
//! The natives are `pure:false` (live network), so importing programs are quarantined from the
//! byte-identity differential; THIS file gates the surface on BOTH backends (`run ≡ runvm`)
//! against an in-process `std::net::TcpListener` fixture server (deterministic, loopback-only —
//! no external network). Wire-level details (URL parsing, chunked decoding, redirects, timeouts,
//! header-injection gate) are unit-tested server-free in `src/native/http_client.rs`.

use phorj::cli::{cmd_run, cmd_transpile, cmd_treewalk};
use std::io::{Read, Write};

fn both(src: &str, expected: &str) {
    let tree = cmd_treewalk(src).expect("program runs on the interpreter");
    assert_eq!(tree, expected, "interpreter output");
    assert_eq!(
        cmd_run(src).expect("program runs on the VM"),
        tree,
        "run ≡ runvm"
    );
}

/// An N-shot fixture server returning canned responses (both backends make their own requests, so
/// the response list is served round-robin-by-accept).
fn fixture(responses: Vec<Vec<u8>>) -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for resp in responses {
            let (mut sock, _) = match listener.accept() {
                Ok(s) => s,
                Err(_) => return,
            };
            let mut buf = [0u8; 16384];
            let _ = sock.read(&mut buf);
            let _ = sock.write_all(&resp);
        }
    });
    port
}

#[test]
fn http_get_reads_status_headers_and_body_on_both_backends() {
    // One canned response per backend run (interpreter + VM).
    let resp =
        b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 13\r\n\r\n{\"ok\": true}\n"
            .to_vec();
    let port = fixture(vec![resp.clone(), resp]);
    let src = format!(
        r#"package Main;
import Core.Output;
import Core.HttpClientModule;
import Core.HttpClientModule.HttpClient;
import Core.HttpClientModule.HttpResponse;
import Core.HttpClientModule.HttpClientError;
function main(): void {{
  try {{
    HttpClient c = new HttpClient();
    discard c.timeout(5000);
    HttpResponse r = c.get("http://127.0.0.1:{port}/api");
    Output.printLine("status {{r.status()}}");
    Output.printLine("type {{r.header("content-type") ?? "<none>"}}");
    Output.printLine("body {{r.body()}}");
  }} catch (HttpClientError e) {{ Output.printLine("unexpected: {{e.message}}"); }}
}}
"#
    );
    both(
        &src,
        "status 200\ntype application/json\nbody {\"ok\": true}\n\n",
    );
}

#[test]
fn http_post_sends_body_and_typed_timeout_fires() {
    let resp = b"HTTP/1.1 201 Created\r\nContent-Length: 0\r\n\r\n".to_vec();
    let port = fixture(vec![resp.clone(), resp]);
    // A silent listener for the timeout half (accepts, never answers; one per backend).
    let silent = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let sport = silent.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let mut held = Vec::new();
        for _ in 0..2 {
            if let Ok((sock, _)) = silent.accept() {
                held.push(sock);
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(2000));
    });
    let src = format!(
        r#"package Main;
import Core.Output;
import Core.HttpClientModule;
import Core.HttpClientModule.HttpClient;
import Core.HttpClientModule.HttpResponse;
import Core.HttpClientModule.HttpTimeoutError;
import Core.HttpClientModule.HttpClientError;
function main(): void {{
  try {{
    HttpClient c = new HttpClient();
    HttpResponse r = c.post("http://127.0.0.1:{port}/make", "application/json", "\{{\"a\": 1\}}");
    Output.printLine("created {{r.status()}}");
    discard c.timeout(80);
    try {{
      HttpResponse t = c.get("http://127.0.0.1:{sport}/slow");
      Output.printLine("unreachable {{t.status()}}");
    }} catch (HttpTimeoutError te) {{
      Output.printLine("timed out");
    }}
  }} catch (HttpClientError e) {{ Output.printLine("unexpected: {{e.message}}"); }}
}}
"#
    );
    both(&src, "created 201\ntimed out\n");
}

/// THE LADDER RULE: `Core.HttpClientModule` is native-only — transpile is the clean ladder error.
#[test]
fn http_client_transpile_is_a_clean_ladder_error() {
    let src = r#"package Main;
import Core.Output;
import Core.HttpClientModule;
function main(): void { Output.printLine("x"); }
"#;
    match cmd_transpile(src) {
        Ok(php) => panic!("expected E-TRANSPILE-HTTPCLIENT, got PHP: {php:?}"),
        Err(e) => {
            assert!(e.contains("E-TRANSPILE-HTTPCLIENT"), "{e}");
            assert!(!e.contains("E-UNKNOWN-IDENT"), "{e}");
        }
    }
}
