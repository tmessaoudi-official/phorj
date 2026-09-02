//! M6 W3 conformance — the serve runtime, checked OUTSIDE the byte-identity spine.
//!
//! `tests/differential.rs` never touches `src/serve.rs` (the determinism quarantine); this file
//! drives the serve loop over a deterministic in-memory [`Transport`] so no socket is needed. It
//! asserts each response is exactly the expected raw HTTP/1.1 bytes AND that the loop's output equals
//! calling the registered handler directly (self-consistency). `tcp_smoke` exercises the real
//! `TcpTransport` end to end and RUNS — it is not `#[ignore]`d (this line used to claim it was).
//!
//! Since DEC-331 S3.3c every program here registers its handler with `Http.serve(cfg, handler)` from
//! a `(): void` `Web` entry — the named `respond(bytes): bytes` entry, and the `handle` bridge that
//! synthesized it, are both retired.
use std::collections::VecDeque;

use std::sync::Arc;

use phorj::interpreter::call_named;
use phorj::serve::{serve, HandlerFactory, Transport};
use phorj::value::Value;

/// Build the interpreter-backend request factory from a checked program — the correctness oracle,
/// and the byte-identity reference every existing test below asserts against.
///
/// S3.3c repointed this at the D5 WEB factory: the named-`respond` entry it used to resolve no longer
/// exists, so every serve program in this file is now a `Web` closure factory. The loop, transport and
/// response-shaping tests below are unchanged by that — they assert the LOOP's behaviour, and the loop
/// never knew how its handler was obtained.
fn ifac(prog: &phorj::ast::Program) -> HandlerFactory {
    phorj::serve::web_interp_factory(Arc::new(prog.clone())).expect("web entry registers a handler")
}

/// Build the bytecode-VM factory from inline source (the default `phg serve` backend). Uses the same
/// reified-operand path the CLI does, so `Vm::run_closure_entry` ≡ `call_closure_in` on the
/// registered handler.
fn vfac(src: &str) -> HandlerFactory {
    let (prog, reified) =
        phorj::cli::parse_checked_program_reified(src).expect("serve program type-checks");
    phorj::serve::web_vm_factory(Arc::new(prog), Arc::new(reified))
        .expect("serve program compiles and registers")
}

/// The serve program the loop/transport tests below drive: a two-route dispatch registered through
/// `Http.serve(cfg, handler)` — the D5 shape, and since S3.3c the ONLY shape a program can be served
/// in.
///
/// It used to hand-roll its own `Request`/`Response` classes plus parse/serialize in pure Phorj, and
/// register through a top-level `respond(bytes): bytes`. That is no longer expressible here: `Http.serve`
/// takes a `(Request) => Response` in CORE.HTTP's types, so a program declaring its own classes of
/// those names cannot pass its handler to it. The hand-rolled-HTTP demonstration is not lost — it is
/// what the `examples/web/server/` PROJECT exists to show (S3.3d converted the flat
/// `examples/web/server.phg` into it), and that one is byte-identity-gated by `tests/differential.rs`.
/// Here it was incidental: every test below asserts the LOOP, not the parser.
const SERVE_PROGRAM: &str = r#"
package Main;
import Core.Runtime.Entry; import Core.Runtime.EntryKind;
import Core.Output;
import Core.Http;
import Core.Http.Request;
import Core.Http.Response;
import Core.Http.ServeConfig;

function dispatch(Request req) -> Response {
  if (req.path == "/") {
    return Response.text(200, "home");
  }
  return Response.text(404, "not found");
}

#[Entry(kind: EntryKind.Web)] function web() -> void {
  Http.serve(new ServeConfig(), function(Request req) -> Response { return dispatch(req); });
}

#[Entry(kind: EntryKind.Cli)] function main() -> void {
  Output.printLine("dispatch is driven by tests/serve.rs, not by main");
}
"#;

/// The RETIRED web-entry shape: `#[Entry(kind: EntryKind.Web)] function handle(Request) -> Response`.
///
/// Until S3.3c this was servable — `Core.Http` injected a `respond(bytes): bytes` bridge that wrapped
/// it. The bridge is deleted, so this program's only remaining job is to be REFUSED, with a message
/// that tells its author what to write instead. It still type-checks (narrowing the checker is S3.3d's
/// job, with the example migration), which is exactly why the serve factory has to catch it.
const HTTP_HANDLE_PROGRAM: &str = r#"
package Main;
import Core.Runtime.Entry; import Core.Runtime.EntryKind;
import Core.Http;
import Core.Http.Request;
import Core.Http.Response;
#[Entry(kind: EntryKind.Web)] function handle(Request req) -> Response {
  if (req.path == "/") {
    return Response.text(200, "home");
  }
  return Response.text(404, "missing");
}
#[Entry(kind: EntryKind.Cli)] function main() -> void { }
"#;

/// A web program whose registered HANDLER faults on every request (an out-of-range index). Drives the
/// per-request resilience assertions: a fault degrades to a 500 and the loop continues.
const FAULTING_WEB_PROGRAM: &str = r#"
package Main;
import Core.Runtime.Entry; import Core.Runtime.EntryKind;
import Core.Http;
import Core.Http.Request;
import Core.Http.Response;
import Core.Http.ServeConfig;
#[Entry(kind: EntryKind.Web)] function web() -> void {
  Http.serve(new ServeConfig(), function(Request req) -> Response {
    List<int> xs = [1];
    return Response.text(200, "{xs[5]}");
  });
}
#[Entry(kind: EntryKind.Cli)] function main() -> void { }
"#;

/// Deterministic in-memory transport: `recv` pops a canned request; `send` records the response.
struct FixtureTransport {
    inbox: VecDeque<Vec<u8>>,
    sent: Vec<Vec<u8>>,
}
impl FixtureTransport {
    fn new(requests: Vec<Vec<u8>>) -> Self {
        Self {
            inbox: requests.into_iter().collect(),
            sent: Vec::new(),
        }
    }
}
impl Transport for FixtureTransport {
    fn recv(&mut self) -> std::io::Result<Option<Vec<u8>>> {
        Ok(self.inbox.pop_front())
    }
    fn send(&mut self, response: &[u8]) -> std::io::Result<()> {
        self.sent.push(response.to_vec());
        Ok(())
    }
}

fn program() -> phorj::ast::Program {
    phorj::cli::parse_checked_program(SERVE_PROGRAM).expect("serve program type-checks")
}

/// Build the exact raw HTTP/1.1 response the program emits (the serializer always recomputes
/// Content-Length, lists `Content-Type` as the sole user header, then CRLFCRLF + body).
fn http(status_line: &str, body: &str) -> Vec<u8> {
    format!(
        "{status_line}\r\nContent-Length: {}\r\nContent-Type: text/plain\r\n\r\n{body}",
        body.len()
    )
    .into_bytes()
}

#[test]
fn serves_known_unknown_and_malformed() {
    let prog = program();
    let get_root = b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n".to_vec();
    let get_missing = b"GET /missing HTTP/1.1\r\nHost: localhost\r\n\r\n".to_vec();
    let malformed = b"GET / HTTP/1.1 no terminator".to_vec();

    let mut fx = FixtureTransport::new(vec![
        get_root.clone(),
        get_missing.clone(),
        malformed.clone(),
    ]);
    serve(&ifac(&prog), &mut fx, false).expect("serve loop completes");

    assert_eq!(fx.sent.len(), 3, "one response per request");
    assert_eq!(fx.sent[0], http("HTTP/1.1 200 OK", "home"));
    assert_eq!(fx.sent[1], http("HTTP/1.1 404 Not Found", "not found"));
    // "Bad Request" (not the old "bad request"): the malformed-request policy now lives in ONE place,
    // the `Http.serve` prelude bridge, instead of being re-spelled by every serve program.
    assert_eq!(fx.sent[2], http("HTTP/1.1 400 Bad Request", "Bad Request"));

    // Self-consistency: the loop's bytes equal the registered handler's own return, obtained WITHOUT
    // the loop. This bypasses `respond_once` entirely — the static-file intercept, the stdout drain
    // and the 500 shaping — so a loop that mangled, reordered or duplicated a response still fails
    // here even though both sides now share the web factory.
    let mut direct = ifac(&prog)();
    for (req, expected) in [
        (get_root, &fx.sent[0]),
        (get_missing, &fx.sent[1]),
        (malformed, &fx.sent[2]),
    ] {
        let (v, out) = direct(&req).expect("the registered handler answers");
        assert!(out.is_empty(), "the handler emits no stdout");
        match v {
            Value::Bytes(b) => assert_eq!(b.as_ref(), expected),
            other => panic!("the handler returned {}, expected bytes", other.type_name()),
        }
    }
}

/// The default VM serve path must produce **byte-identical** responses to the interpreter path
/// (`--tree-walker`). serve is deliberately OUTSIDE the differential harness (the determinism
/// quarantine), so this is where `interp ≡ VM` is asserted for the served `respond` entry — covering
/// the normal 200/404/400 routes, the production (non-dev) 500, and the injected Core.Http bridge.
#[test]
fn vm_serve_is_byte_identical_to_interpreter() {
    // Drive the same requests through both backends over the deterministic fixture transport.
    let run = |factory: &HandlerFactory, requests: Vec<Vec<u8>>| {
        let mut fx = FixtureTransport::new(requests);
        serve(factory, &mut fx, false).expect("serve loop completes");
        fx.sent
    };
    let routes = || {
        vec![
            b"GET / HTTP/1.1\r\nHost: x\r\n\r\n".to_vec(),
            b"GET /missing HTTP/1.1\r\nHost: x\r\n\r\n".to_vec(),
            b"GET / HTTP/1.1 no terminator".to_vec(), // malformed → 400
        ]
    };

    // Normal routes: 200 / 404 / 400.
    let prog = program();
    let interp = run(&ifac(&prog), routes());
    let vm = run(&vfac(SERVE_PROGRAM), routes());
    assert_eq!(
        interp, vm,
        "VM serve responses must byte-match the interpreter"
    );
    assert_eq!(vm.len(), 3, "three responses (not both empty/broken)");
    assert_eq!(vm[0], http("HTTP/1.1 200 OK", "home"));
    // Body "Bad Request" since S3.3c: the malformed-request 400 is the `Http.serve` prelude bridge's,
    // single-sourced in phorj so both legs cannot answer it differently.
    assert_eq!(vm[2], http("HTTP/1.1 400 Bad Request", "Bad Request"));

    // Production 500: a `respond` that faults degrades to the bare 500 on BOTH backends (dev = false).
    let fault = FAULTING_WEB_PROGRAM;
    let req = || vec![b"GET / HTTP/1.1\r\n\r\n".to_vec()];
    let i500 = run(&ifac(&checked(fault)), req());
    let v500 = run(&vfac(fault), req());
    assert_eq!(i500, v500, "production 500 must byte-match across backends");
    assert!(v500[0].starts_with(b"HTTP/1.1 500 Internal Server Error"));

    // The third block here used to serve `HTTP_HANDLE_PROGRAM` through the injected `respond` bridge
    // on both backends. S3.3c deleted that bridge, and the Core.Http path it covered is now asserted
    // on both backends by `http_serve_closure_handler_is_servable` (S3.3a) — including the malformed
    // -> 400 case this block never checked. Coverage rose; it did not move here.
}

/// A transport with a scripted sequence of `recv` results (including errors), so the loop's
/// resilience (GA blocker B3) can be tested deterministically without a socket.
struct ScriptedTransport {
    recvs: VecDeque<std::io::Result<Option<Vec<u8>>>>,
    sent: Vec<Vec<u8>>,
}
impl Transport for ScriptedTransport {
    fn recv(&mut self) -> std::io::Result<Option<Vec<u8>>> {
        self.recvs.pop_front().unwrap_or(Ok(None))
    }
    fn send(&mut self, response: &[u8]) -> std::io::Result<()> {
        self.sent.push(response.to_vec());
        Ok(())
    }
}

/// Type-check an inline program for the degradation tests below.
fn checked(src: &str) -> phorj::ast::Program {
    phorj::cli::parse_checked_program(src).expect("program type-checks")
}

/// B3: a per-connection `recv` error (client reset, transient accept) is logged and skipped — the
/// surrounding good request is still served and the loop ends cleanly on `Ok(None)`.
#[test]
fn recv_error_does_not_kill_the_loop() {
    let prog = program();
    let good = b"GET / HTTP/1.1\r\nHost: x\r\n\r\n".to_vec();
    let mut t = ScriptedTransport {
        recvs: VecDeque::from(vec![
            Err(std::io::Error::new(
                std::io::ErrorKind::ConnectionReset,
                "reset",
            )),
            Ok(Some(good)),
            Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "pipe")),
            Ok(None),
        ]),
        sent: Vec::new(),
    };
    serve(&ifac(&prog), &mut t, false)
        .expect("loop survives per-connection errors and ends cleanly");
    assert_eq!(
        t.sent.len(),
        1,
        "the one good request was served despite surrounding errors"
    );
    assert_eq!(t.sent[0], http("HTTP/1.1 200 OK", "home"));
}

/// B3: a listener that only ever errors (unrecoverable) eventually shuts the loop down via the
/// consecutive-error circuit breaker, rather than spinning forever.
#[test]
fn unrecoverable_listener_eventually_stops() {
    let prog = program();
    let recvs = (0..1000)
        .map(|_| Err(std::io::Error::other("listener dead")))
        .collect();
    let mut t = ScriptedTransport {
        recvs,
        sent: Vec::new(),
    };
    assert!(
        serve(&ifac(&prog), &mut t, false).is_err(),
        "a listener that only errors must eventually end the loop"
    );
    assert!(t.sent.is_empty(), "nothing could be served");
}

/// P1-e: a request that *faults* inside `respond` degrades to a 500 and the loop continues to the
/// next request (one bad request never aborts the server).
#[test]
fn respond_fault_degrades_to_500_and_loop_continues() {
    let prog = checked(FAULTING_WEB_PROGRAM);
    let req = b"GET / HTTP/1.1\r\n\r\n".to_vec();
    let mut fx = FixtureTransport::new(vec![req.clone(), req]);
    serve(&ifac(&prog), &mut fx, false).expect("loop completes despite per-request faults");
    assert_eq!(
        fx.sent.len(),
        2,
        "both faulting requests answered; loop continued"
    );
    for resp in &fx.sent {
        assert!(
            resp.starts_with(b"HTTP/1.1 500 Internal Server Error"),
            "a request fault degrades to 500, got: {}",
            String::from_utf8_lossy(&resp[..resp.len().min(40)])
        );
    }
}

/// M-DX S0: under the **Dev** profile an uncaught fault renders the rich HTML error page (trace +
/// request), while **Release** returns the bare `text/plain` 500 — the profile is the sole switch and
/// it changes only this side-channel. (Fills the coverage gap: no test previously exercised `dev=true`.)
#[test]
fn dev_profile_shows_rich_error_page_release_shows_bare_500() {
    use phorj::profile::Profile;
    let prog = checked(FAULTING_WEB_PROGRAM);
    let req = b"GET /boom HTTP/1.1\r\n\r\n".to_vec();

    // Dev profile → rich HTML page.
    let mut dev_fx = FixtureTransport::new(vec![req.clone()]);
    serve(&ifac(&prog), &mut dev_fx, Profile::Dev.is_dev()).expect("loop completes");
    let dev_resp = String::from_utf8_lossy(&dev_fx.sent[0]);
    assert!(
        dev_resp.starts_with("HTTP/1.1 500 Internal Server Error"),
        "{dev_resp}"
    );
    assert!(
        dev_resp.contains("Content-Type: text/html"),
        "dev page is HTML: {dev_resp}"
    );
    assert!(
        dev_resp.contains("Runtime fault"),
        "dev page shows the fault: {dev_resp}"
    );
    assert!(
        dev_resp.contains("development only"),
        "dev page is labelled dev-only"
    );

    // Release profile → bare plain-text 500, no trace/source leak.
    let mut rel_fx = FixtureTransport::new(vec![req]);
    serve(&ifac(&prog), &mut rel_fx, Profile::Release.is_dev()).expect("loop completes");
    let rel_resp = String::from_utf8_lossy(&rel_fx.sent[0]);
    assert!(
        rel_resp.starts_with("HTTP/1.1 500 Internal Server Error"),
        "{rel_resp}"
    );
    assert!(
        rel_resp.contains("Content-Type: text/plain"),
        "release 500 is plain: {rel_resp}"
    );
    assert!(
        !rel_resp.contains("Runtime fault"),
        "release must NOT leak a trace: {rel_resp}"
    );
}

/// P1-e: a handler that returns a non-`bytes` value degrades to a 500 (the runtime never trusts the
/// declared return type — it checks the actual value).
///
/// Driven by a SYNTHETIC `Handler` rather than a phorj program, because S3.3c made this
/// unrepresentable in source: `Http.serve` takes a `(Request) => Response`, the prelude wraps it into
/// `(bytes) => bytes`, and the checker rejects anything else long before the runtime sees it. The
/// branch stays because the `Handler` TYPE still admits any `Value` — a future registration path, or a
/// native returning the wrong thing, lands right here — and an untested defensive branch is how a
/// panic reaches production. `HandlerFactory` is public precisely so a caller can supply its own.
#[test]
fn a_non_bytes_handler_return_degrades_to_500() {
    let factory: HandlerFactory =
        Box::new(|| Box::new(|_raw: &[u8]| Ok((Value::Int(7), String::new()))));
    let mut fx = FixtureTransport::new(vec![b"GET / HTTP/1.1\r\n\r\n".to_vec()]);
    serve(&factory, &mut fx, false).expect("loop completes");
    assert_eq!(fx.sent.len(), 1);
    assert!(fx.sent[0].starts_with(b"HTTP/1.1 500 Internal Server Error"));
}

#[test]
fn unknown_entry_reports_cleanly() {
    let prog = program();
    let err = call_named(&prog, "no_such_fn", vec![]).expect_err("missing entry is an error");
    assert!(err.to_string().contains("no `no_such_fn` function"));
}

#[test]
fn tcp_smoke() {
    use phorj::serve::TcpTransport;
    use std::io::{Read, Write};
    use std::net::TcpStream;

    let prog = program();
    let mut t = TcpTransport::bind("127.0.0.1:0").expect("bind ephemeral port");
    let addr = t.local_addr().expect("addr");
    // Detached server thread: serves the one connection we make, then blocks on the next accept
    // (harmless — the process exits at end of test).
    std::thread::spawn(move || {
        let _ = serve(&ifac(&prog), &mut t, false);
    });

    let mut s = TcpStream::connect(addr).expect("connect");
    s.write_all(b"GET / HTTP/1.1\r\nHost: x\r\n\r\n")
        .expect("write request");
    let mut resp = Vec::new();
    s.read_to_end(&mut resp).expect("read response");
    assert_eq!(resp, http("HTTP/1.1 200 OK", "home"));
}

/// S4.1 — HTTP/1.1 keep-alive: two requests on ONE socket get two responses (the connection is reused,
/// not closed after the first). Keep-alive requires a configured timeout (the idle-socket guard), so the
/// transport is bound with one. Each response is self-delimiting (`Content-Length`), so the client reads
/// exactly one response's worth of bytes per request.
#[test]
fn tcp_keepalive_serves_two_requests_on_one_socket() {
    use phorj::serve::TcpTransport;
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::time::Duration;

    let prog = program();
    let mut t = TcpTransport::bind("127.0.0.1:0").expect("bind ephemeral port");
    t.set_timeout(Some(Duration::from_secs(5))); // keep-alive only with a timeout (idle guard)
    let addr = t.local_addr().expect("addr");
    std::thread::spawn(move || {
        let _ = serve(&ifac(&prog), &mut t, false);
    });

    let expected = http("HTTP/1.1 200 OK", "home");
    let mut s = TcpStream::connect(addr).expect("connect");
    s.set_read_timeout(Some(Duration::from_secs(5)))
        .expect("client timeout");
    // Two HTTP/1.1 requests (no `Connection: close`) on the SAME socket.
    for i in 0..2 {
        s.write_all(b"GET / HTTP/1.1\r\nHost: x\r\n\r\n")
            .unwrap_or_else(|e| panic!("write request {i}: {e}"));
        let mut resp = vec![0u8; expected.len()];
        s.read_exact(&mut resp)
            .unwrap_or_else(|e| panic!("read response {i} on the kept-alive socket: {e}"));
        assert_eq!(resp, expected, "response {i}");
    }
}

/// S4.1 — `Connection: close` closes the socket after one response even with keep-alive available: the
/// client's `read_to_end` returns exactly one response and then EOF (the server drops the socket).
#[test]
fn tcp_connection_close_closes_after_one_response() {
    use phorj::serve::TcpTransport;
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::time::Duration;

    let prog = program();
    let mut t = TcpTransport::bind("127.0.0.1:0").expect("bind ephemeral port");
    t.set_timeout(Some(Duration::from_secs(5)));
    let addr = t.local_addr().expect("addr");
    std::thread::spawn(move || {
        let _ = serve(&ifac(&prog), &mut t, false);
    });

    let mut s = TcpStream::connect(addr).expect("connect");
    s.set_read_timeout(Some(Duration::from_secs(5)))
        .expect("client timeout");
    s.write_all(b"GET / HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n")
        .expect("write request");
    let mut resp = Vec::new();
    s.read_to_end(&mut resp).expect("read response then EOF");
    assert_eq!(resp, http("HTTP/1.1 200 OK", "home"));
}

/// M6 W3 — the worker pool serves many concurrent connections correctly. 24 clients hit a 4-worker
/// pool at once; every one must get the exact `home` response (correctness under concurrency — no
/// deadlock, no interleaved/corrupted responses, no lost connection). Real sockets on an ephemeral
/// port. Robust by construction (asserts correctness of all responses, not flaky wall-clock overlap).
#[test]
fn pool_serves_concurrent_connections() {
    use phorj::serve::serve_pool;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};

    let prog = program();
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
    let addr = listener.local_addr().expect("addr");
    // Detached 4-worker pool (blocks forever in accept; the process exits at end of test).
    std::thread::spawn(move || {
        let _ = serve_pool(listener, ifac(&prog), None, false, 4);
    });

    let expected = http("HTTP/1.1 200 OK", "home");
    let clients: Vec<_> = (0..24)
        .map(|_| {
            std::thread::spawn(move || {
                let mut s = TcpStream::connect(addr).expect("connect");
                s.write_all(b"GET / HTTP/1.1\r\nHost: x\r\n\r\n")
                    .expect("write");
                let mut resp = Vec::new();
                s.read_to_end(&mut resp).expect("read");
                resp
            })
        })
        .collect();

    for (i, c) in clients.into_iter().enumerate() {
        let resp = c.join().expect("client thread");
        assert_eq!(
            resp, expected,
            "concurrent client {i} got the wrong response"
        );
    }
}

/// The **default** `phg serve` (no `--workers`) runs the VM through the multi-worker POOL — a path
/// the single-threaded byte-identity test above does not cover. Each worker calls `compile_with`
/// itself (the compiled `Rc`-bearing program can't cross threads), so this also proves concurrent
/// per-worker compilation is race-free. 24 clients / 4 VM workers; every response must be the exact
/// `home` bytes (byte-identical to the interpreter, so the same expectation the interp pool test uses).
#[test]
fn pool_serves_concurrent_connections_on_the_vm() {
    use phorj::serve::serve_pool;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
    let addr = listener.local_addr().expect("addr");
    // Detached 4-worker VM pool — each worker compiles its own program concurrently at startup.
    std::thread::spawn(move || {
        let _ = serve_pool(listener, vfac(SERVE_PROGRAM), None, false, 4);
    });

    let expected = http("HTTP/1.1 200 OK", "home");
    let clients: Vec<_> = (0..24)
        .map(|_| {
            std::thread::spawn(move || {
                let mut s = TcpStream::connect(addr).expect("connect");
                s.write_all(b"GET / HTTP/1.1\r\nHost: x\r\n\r\n")
                    .expect("write");
                let mut resp = Vec::new();
                s.read_to_end(&mut resp).expect("read");
                resp
            })
        })
        .collect();

    for (i, c) in clients.into_iter().enumerate() {
        let resp = c.join().expect("client thread");
        assert_eq!(
            resp, expected,
            "concurrent VM client {i} got the wrong response"
        );
    }
}

/// S4.2 — graceful shutdown: after a request is served, flipping the shutdown flag makes
/// `serve_pool_with` stop accepting, drain in-flight work, **join every worker**, and return `Ok` —
/// no abrupt cut, no hang. (The `join` blocks the test until the drain completes, so a regression that
/// failed to drain/return would surface as a hang the harness times out.)
#[test]
fn pool_graceful_shutdown_drains_and_returns() {
    use phorj::serve::serve_pool_with;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    let prog = program();
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
    let addr = listener.local_addr().expect("addr");
    let flag = Arc::new(AtomicBool::new(false));
    let server_flag = Arc::clone(&flag);
    let server = std::thread::spawn(move || {
        serve_pool_with(
            listener,
            ifac(&prog),
            Some(Duration::from_secs(5)),
            false,
            2,
            Some(server_flag),
            None, // S3.5: plain HTTP — the graceful-shutdown contract is transport-independent
        )
    });

    // One request completes normally before shutdown.
    {
        let mut s = TcpStream::connect(addr).expect("connect");
        s.write_all(b"GET / HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n")
            .expect("write");
        let mut resp = Vec::new();
        s.read_to_end(&mut resp).expect("read");
        assert_eq!(resp, http("HTTP/1.1 200 OK", "home"));
    }

    // Signal shutdown; the pool must drain, join workers, and return Ok.
    flag.store(true, Ordering::SeqCst);
    let joined = server.join().expect("server thread panicked");
    assert!(
        joined.is_ok(),
        "graceful shutdown returns Ok, got {joined:?}"
    );
}

/// S4.1 — the worker pool also keeps connections alive (when a timeout is configured): two requests on
/// one socket served by the same worker get two responses. Exercises the pool's per-connection
/// keep-alive loop (a separate code path from the single-threaded `TcpTransport`).
#[test]
fn pool_keepalive_serves_two_requests_on_one_socket() {
    use phorj::serve::serve_pool;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::time::Duration;

    let prog = program();
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
    let addr = listener.local_addr().expect("addr");
    std::thread::spawn(move || {
        let _ = serve_pool(
            listener,
            ifac(&prog),
            Some(Duration::from_secs(5)),
            false,
            2,
        );
    });

    let expected = http("HTTP/1.1 200 OK", "home");
    let mut s = TcpStream::connect(addr).expect("connect");
    s.set_read_timeout(Some(Duration::from_secs(5)))
        .expect("client timeout");
    for i in 0..2 {
        s.write_all(b"GET / HTTP/1.1\r\nHost: x\r\n\r\n")
            .unwrap_or_else(|e| panic!("write request {i}: {e}"));
        let mut resp = vec![0u8; expected.len()];
        s.read_exact(&mut resp)
            .unwrap_or_else(|e| panic!("read response {i} on the kept-alive pool socket: {e}"));
        assert_eq!(resp, expected, "pool response {i}");
    }
}

// ── DEC-282 site mode: the docroot static layer ──────────────────────────────────────────────────

/// The static intercept (`respond_once` consults the process-global docroot) serves real files
/// with MIME + ETag/Last-Modified, guards `.phg` bytes, and falls through to the program for
/// everything else. Runs over the in-memory transport — nextest's process-per-test isolation
/// makes the OnceLock docroot safe to set here.
#[test]
fn site_mode_statics_serve_guard_and_fall_through() {
    let dir = std::env::temp_dir().join(format!("phorj_site_{}", std::process::id()));
    let public = dir.join("public");
    std::fs::create_dir_all(&public).expect("mkdir public");
    std::fs::write(public.join("app.css"), "body{}").expect("css");
    std::fs::write(public.join("secret.phg"), "package Main;\n").expect("phg");
    phorj::serve::set_docroot(public.clone());

    let get_css = b"GET /app.css HTTP/1.1\r\nHost: x\r\n\r\n".to_vec();
    let get_phg = b"GET /secret.phg HTTP/1.1\r\nHost: x\r\n\r\n".to_vec();
    let get_root = b"GET / HTTP/1.1\r\nHost: x\r\n\r\n".to_vec();
    let mut fx = FixtureTransport::new(vec![get_css, get_phg, get_root]);
    let factory: HandlerFactory = ifac(&program());
    serve(&factory, &mut fx, false).expect("serve loop");

    let css = String::from_utf8_lossy(&fx.sent[0]).to_string();
    assert!(css.starts_with("HTTP/1.1 200 OK"), "css: {css}");
    assert!(css.contains("Content-Type: text/css"), "css: {css}");
    assert!(css.contains("ETag: "), "css: {css}");
    assert!(css.contains("Last-Modified: "), "css: {css}");
    assert!(css.ends_with("body{}"), "css: {css}");
    // 304 on a matching If-None-Match.
    let etag = css
        .lines()
        .find_map(|l| l.strip_prefix("ETag: "))
        .expect("etag header")
        .trim()
        .to_string();
    let conditional =
        format!("GET /app.css HTTP/1.1\r\nHost: x\r\nIf-None-Match: {etag}\r\n\r\n").into_bytes();
    let mut fx2 = FixtureTransport::new(vec![conditional]);
    serve(&factory, &mut fx2, false).expect("serve loop 2");
    assert!(
        String::from_utf8_lossy(&fx2.sent[0]).starts_with("HTTP/1.1 304 Not Modified"),
        "conditional: {}",
        String::from_utf8_lossy(&fx2.sent[0])
    );

    let phg = String::from_utf8_lossy(&fx.sent[1]).to_string();
    assert!(
        phg.starts_with("HTTP/1.1 404"),
        ".phg source must never be served: {phg}"
    );
    // `/` falls through to the program (the FixtureTransport program serves it 200).
    let root = String::from_utf8_lossy(&fx.sent[2]).to_string();
    assert!(root.starts_with("HTTP/1.1 200"), "fallthrough: {root}");
}

// ── DEC-363: the response-header injection guard (P1 SECURITY) ────────────────────────────────────
//
// Reproduced before the fix: `withHeader("X-User", <CRLF payload>)` serialized a response whose
// `Content-Length: 2` still described "ok" while ~30 further bytes followed — an injected header, an
// early head terminator and a second body. A request-smuggling / desync primitive, not just splitting.
//
// The guard lives in the phorj PRELUDE, so these run through the real language surface on BOTH Rust
// backends; the transpiled PHP leg faults with the identical message by construction (one policy, one
// wording), which the byte-identity gate covers.

/// A program that builds a response through `surface` and prints the serialized head.
fn injection_prog(surface: &str) -> String {
    format!(
        r#"package Main;
import Core.Output;
import Core.Http;
import Core.Http.Response;
import Core.Http.Cookie;
import Core.Bytes;
import Core.Runtime.Entry;
import Core.Runtime.EntryKind;

#[Entry(kind: EntryKind.Cli)]
function main(): int {{
    string evil = Bytes.toString(b"x\x0d\x0aX-Injected: yes\x0d\x0a\x0d\x0a<html>pwned</html>") ?? "";
    string nul = Bytes.toString(b"a\x00b") ?? "";
    Response r = {surface};
    string head = Bytes.toString(r.serialize()) ?? "";
    Output.printLine(head);
    return 0;
}}
"#
    )
}

fn faults_on_both_backends(surface: &str) {
    let src = injection_prog(surface);
    for (leg, res) in [
        ("vm", phorj::cli::cmd_run(&src)),
        ("tree-walker", phorj::cli::cmd_treewalk(&src)),
    ] {
        match res {
            Ok(out) => panic!("{leg}: injection was NOT refused for {surface}; got:\n{out}"),
            Err(e) => assert!(
                e.contains("contains a forbidden character"),
                "{leg}: wrong diagnostic for {surface}: {e}"
            ),
        }
    }
}

#[test]
fn dec363_withheader_value_crlf_is_refused() {
    faults_on_both_backends(r#"Response.text(200, "ok").withHeader("X-User", evil)"#);
}

#[test]
fn dec363_withheader_name_crlf_is_refused() {
    // The NAME is equally unvalidated — an evil name injects its own line.
    faults_on_both_backends(r#"Response.text(200, "ok").withHeader(evil, "v")"#);
}

#[test]
fn dec363_withheader_name_with_a_colon_is_refused() {
    // A `:` in a name would forge a second header on the same line.
    faults_on_both_backends(r#"Response.text(200, "ok").withHeader("X:Forged", "v")"#);
}

#[test]
fn dec363_nul_is_refused_in_a_header_value() {
    // Ruled extra 1: NUL joins the rejected set on BOTH sides (PHP's own `header()` rejects it).
    faults_on_both_backends(r#"Response.text(200, "ok").withHeader("X-User", nul)"#);
}

#[test]
fn dec363_cookie_name_value_and_path_are_all_refused() {
    // The guard sits on the Cookie CONSTRUCTOR, so all three string fields AND all four builders
    // (`path`/`secure`/`httpOnly`/`partitioned`, which each re-construct) are covered at once.
    faults_on_both_backends(r#"Response.text(200, "ok").withCookie(new Cookie(evil, "v", "/"))"#);
    faults_on_both_backends(r#"Response.text(200, "ok").withCookie(new Cookie("sid", evil, "/"))"#);
    faults_on_both_backends(r#"Response.text(200, "ok").withCookie(new Cookie("sid", "v", evil))"#);
}

#[test]
fn dec363_a_cookie_builder_cannot_smuggle_a_bad_path() {
    // `path()` re-constructs, so the constructor guard catches it — this is why the chokepoint is the
    // constructor rather than `render()`.
    faults_on_both_backends(
        r#"Response.text(200, "ok").withCookie(new Cookie("sid", "v", "/").path(evil))"#,
    );
}

#[test]
fn dec363_a_clean_response_still_serializes_and_does_not_split() {
    // The other half: the guard must not break ordinary headers or cookies, and the serialized head
    // must contain exactly one terminator with no injected line.
    let src = injection_prog(
        r#"Response.text(200, "ok").withHeader("X-User", "alice").withCookie(new Cookie("sid", "abc", "/"))"#,
    );
    let out = phorj::cli::cmd_run(&src).expect("a clean response must still serialize");
    assert!(out.contains("X-User: alice"), "{out}");
    assert!(out.contains("Set-Cookie: sid=abc"), "{out}");
    assert!(
        !out.contains("X-Injected"),
        "no injected header may appear: {out}"
    );
    assert_eq!(
        phorj::cli::cmd_treewalk(&src).expect("interp"),
        out,
        "interp ≡ VM"
    );
}

// ── DEC-331 S3.3a — `Http.serve(cfg, handler)` ──────────────────────────────────────────────────
// D5's one handler model: the handler is a CLOSURE passed to `Http.serve` inside a `Web` entry, not
// a magic-named top-level function. `respond`/`handle` still work until S3.3c retires them (Q1,
// developer-ruled 2026-08-22), so this must pass ALONGSIDE the two tests above, not instead of them.
//
// The entry is a closure FACTORY, nothing more: `Http.serve` registers the closure and RETURNS, and
// `serve_program` drives the same loop it always has. That keeps the transport, keep-alive,
// static-file interception and the `(Value, String)` stdout contract untouched.
const HTTP_SERVE_PROGRAM: &str = r#"
package Main;
import Core.Runtime.Entry; import Core.Runtime.EntryKind;
import Core.Http;
import Core.Http.Request;
import Core.Http.Response;
import Core.Http.ServeConfig;
#[Entry(kind: EntryKind.Web)] function web() -> void {
  Http.serve(new ServeConfig(), function(Request req) -> Response {
    if (req.path == "/") {
      return Response.text(200, "home");
    }
    return Response.text(404, "missing");
  });
}
#[Entry(kind: EntryKind.Cli)] function main() -> void { }
"#;

/// DEC-331 D5 / S3.3a — the executable spec for `Http.serve(cfg, handler)`, now green.
///
/// BOTH backends run it and must agree byte for byte. Serve is Invariant-14 quarantined, so
/// `tests/differential.rs` never sees this program: this assertion is the only thing standing
/// between the two legs and a silent divergence on the web path.
#[test]
fn http_serve_closure_handler_is_servable() {
    for (backend, fac) in [
        ("interp", ifac(&checked(HTTP_SERVE_PROGRAM))),
        ("vm", vfac(HTTP_SERVE_PROGRAM)),
    ] {
        let mut fx = FixtureTransport::new(vec![
            b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n".to_vec(),
            b"GET /missing HTTP/1.1\r\nHost: localhost\r\n\r\n".to_vec(),
            b"not a request".to_vec(),
        ]);
        serve(&fac, &mut fx, false).expect("serve loop completes");

        assert_eq!(fx.sent.len(), 3, "{backend}: one response per request");
        assert_eq!(fx.sent[0], http("HTTP/1.1 200 OK", "home"), "{backend}");
        assert_eq!(
            fx.sent[1],
            http("HTTP/1.1 404 Not Found", "missing"),
            "{backend}"
        );
        // Malformed -> 400, from the SAME phorj-side bridge the legacy `handle` path uses. Keeping
        // that policy in the prelude rather than in Rust is what makes it identical on both legs.
        assert_eq!(
            fx.sent[2],
            http("HTTP/1.1 400 Bad Request", "Bad Request"),
            "{backend}"
        );
    }
}

/// A web entry whose handler CAPTURES a counter and also mutates a program STATIC, reporting both.
/// The two must behave DIFFERENTLY across requests, and that difference IS the ruled per-request
/// semantics (plan section 3b): captures persist because they live in the closure value; statics
/// re-seed because they live in the machine, and a fresh machine is built per request.
const HTTP_SERVE_STATE_PROGRAM: &str = r#"
package Main;
import Core.Runtime.Entry; import Core.Runtime.EntryKind;
import Core.Http;
import Core.Http.Request;
import Core.Http.Response;
import Core.Http.ServeConfig;
class Counter { public mutable int n; constructor() { this.n = 0; } }
class Seen { static mutable int total = 0; }
#[Entry(kind: EntryKind.Web)] function web(): void {
  Counter c = new Counter();
  Http.serve(new ServeConfig(), function(Request req): Response {
    c.n = c.n + 1;
    Seen.total = Seen.total + 1;
    return Response.text(200, "captured={c.n} static={Seen.total}");
  });
}
#[Entry(kind: EntryKind.Cli)] function main(): void { }
"#;

/// The per-request state contract, pinned on BOTH backends.
///
/// This is the assertion a future refactor that reuses one interpreter/VM across requests would
/// break — and nothing else would notice, because serve sits outside the byte-identity differential.
#[test]
fn captures_persist_across_requests_while_statics_reseed() {
    for (backend, fac) in [
        ("interp", ifac(&checked(HTTP_SERVE_STATE_PROGRAM))),
        ("vm", vfac(HTTP_SERVE_STATE_PROGRAM)),
    ] {
        let req = b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n".to_vec();
        let mut fx = FixtureTransport::new(vec![req.clone(), req.clone(), req]);
        serve(&fac, &mut fx, false).expect("serve loop completes");

        assert_eq!(fx.sent.len(), 3, "{backend}");
        assert_eq!(
            fx.sent[0],
            http("HTTP/1.1 200 OK", "captured=1 static=1"),
            "{backend}"
        );
        assert_eq!(
            fx.sent[1],
            http("HTTP/1.1 200 OK", "captured=2 static=1"),
            "{backend}: the capture must COUNT UP and the static must RE-SEED"
        );
        assert_eq!(
            fx.sent[2],
            http("HTTP/1.1 200 OK", "captured=3 static=1"),
            "{backend}"
        );
    }
}

/// A web entry that never calls `Http.serve` registers nothing. That is a STARTUP error naming the
/// missing call — not an empty handler that 500s every request with something opaque.
#[test]
fn a_web_entry_that_never_calls_serve_is_a_named_startup_error() {
    let src = r#"
package Main;
import Core.Runtime.Entry; import Core.Runtime.EntryKind;
import Core.Http;
#[Entry(kind: EntryKind.Web)] function web(): void { }
#[Entry(kind: EntryKind.Cli)] function main(): void { }
"#;
    let prog = phorj::cli::parse_checked_program(src).expect("program type-checks");
    // `HandlerFactory` is a boxed `dyn Fn`, so it is not `Debug` and `expect_err` cannot be used.
    let msg = match phorj::serve::web_interp_factory(Arc::new(prog)) {
        Ok(_) => panic!("a web entry that registers nothing must not produce a factory"),
        Err(d) => format!("{d:?}"),
    };
    assert!(
        msg.contains("Http.serve"),
        "the error must name the missing call: {msg}"
    );
    // S3.3c: with the named-`respond` fallback gone this is the ONLY way a program can fail to be
    // servable, so it carries a code that `phg explain` can reach.
    assert!(
        msg.contains("E-SERVE-NO-HANDLER"),
        "the error must carry its code: {msg}"
    );
}

/// Invariant 14, tier 2 — `Http.serve` has NO faithful idiomatic PHP mapping (PHP is *served by* a
/// web server rather than being one), so `phg transpile` REFUSES a program that calls it.
///
/// This landed with S3.3a and not later, and the reason is a rule rather than a preference: before
/// this slice an `Http.serve` program did not type-check, so the transpiler never saw one. The moment
/// it checks clean, "checks clean AND transpiles to something" is precisely the tier-3 silent
/// semantic downgrade the ladder forbids. `E-TRANSPILE-SERVE` was already RULED (spec D7, register
/// DEC-331) and was listed in `docs/plans/product-driven-gap-programme.plan.md` as documented-but-
/// unbuilt; this is the build.
#[test]
fn transpiling_a_program_that_calls_http_serve_is_refused() {
    let prog = phorj::cli::parse_program(HTTP_SERVE_PROGRAM).expect("parses");
    let err = phorj::cli::transpile_program(&prog, HTTP_SERVE_PROGRAM)
        .expect_err("a program calling Http.serve must not transpile");
    assert!(
        err.contains("E-TRANSPILE-SERVE"),
        "the refusal must carry the ruled code: {err}"
    );
}

/// A program calling the RAW native under `Core.Http` — `Core.Native.Http.registerServe` — reached
/// the emitter, and the emitted PHP fatalled at runtime.
///
/// The call-keyed refusal above reads `Http.serve` and nothing else, so spelling the same
/// registration through the raw twin walked straight past it: `phg transpile` exited 0 and emitted
/// `__phorj_http_register_serve(...)`, a helper NO family defines. The native legs ran fine and the
/// PHP leg died with `Call to undefined function` (exit 255) — Invariant 1's byte-identity spine
/// broken by a program that the toolchain reported as successfully transpiled. Invariant 14 tier 2
/// demands a hard error at transpile time, never a silent divergence discovered at runtime.
///
/// The bypass was not obscure: `E-IMPORT-NATIVE-MEMBER` (`checker/program/imports.rs`) actively
/// RECOMMENDS the whole-module import spelling that reaches it.
///
/// Closed the way DEC-277 closed the same hole for the four sibling raw twins
/// (`Core.Native.{Database,Session,HttpClient,Mail}`) — a module row in `NATIVE_ONLY`, keyed on the
/// USER's import. That layer and the call-keyed layer above coexist deliberately: this one cannot
/// fire on the injected prelude, because `reject_native_only_transpile` runs on the RAW program
/// before `check_and_expand` injects anything, so the prelude's own
/// `import Core.Native.Http as NativeHttp;` is invisible to it. The companion test below pins that
/// property — it is the whole reason a module-keyed row is safe here.
#[test]
fn transpiling_a_program_that_imports_the_raw_serve_native_is_refused() {
    const RAW_NATIVE_BYPASS: &str = r#"
package Main;

import Core.Native.Http as NativeHttp;
import Core.Http;
import Core.Http.ServeConfig;

function main(): void {
  NativeHttp.registerServe(new ServeConfig(), function(bytes raw): bytes {
    return raw;
  });
}
"#;
    let prog = phorj::cli::parse_program(RAW_NATIVE_BYPASS).expect("parses");
    let err = phorj::cli::transpile_program(&prog, RAW_NATIVE_BYPASS)
        .expect_err("a program importing the raw serve native must not transpile");
    assert!(
        err.contains("E-TRANSPILE-SERVE"),
        "the refusal must carry the same ruled code as the friendly spelling: {err}"
    );
}

/// THE SPELLING THE FIRST FIX MISSED, and the one a milestone panel had to find: **no raw import at
/// all.** The injected `http_request_prelude` fragment declares `import Core.Native.Http as
/// NativeHttp;`, and until DEC-459 prelude imports were PROGRAM-WIDE once injected, so `NativeHttp`
/// was in scope for every `import Core.Http;` program: the program below type-checked clean,
/// `phg transpile` exited 0, both native legs ran, and the PHP leg died with `Call to undefined
/// function __phorj_http_register_serve()` at exit 255. A by-name containment arm in `ladder.rs`
/// refused it as `E-TRANSPILE-SERVE` for a while.
///
/// DEC-459 removed the leak itself: the prelude's alias is isolated under a spelling no user token can
/// take, so `NativeHttp` here is simply an UNKNOWN IDENTIFIER — for `phg check` and for `transpile`
/// alike — and the containment arm is gone with it. (`tests/prelude_isolation.rs` covers the other
/// three leaked qualifiers and the alias collisions.)
#[test]
fn a_leaked_prelude_alias_is_no_longer_in_user_scope() {
    const LEAKED_ALIAS_BYPASS: &str = r#"
package Main;

import Core.Runtime.Entry;
import Core.Runtime.EntryKind;
import Core.Http;
import Core.Http.ServeConfig;

#[Entry(kind: EntryKind.Cli)]
function main(): void {
  NativeHttp.registerServe(new ServeConfig(), function(bytes raw): bytes { return raw; });
}
"#;
    let err = phorj::cli::cmd_check(LEAKED_ALIAS_BYPASS)
        .expect_err("`NativeHttp` must not resolve in user code");
    assert!(
        err.contains("E-UNKNOWN-IDENT") && err.contains("`NativeHttp`"),
        "the leaked qualifier must be an unknown identifier, got: {err}"
    );
    let prog = phorj::cli::parse_program(LEAKED_ALIAS_BYPASS).expect("parses");
    let err = phorj::cli::transpile_program(&prog, LEAKED_ALIAS_BYPASS)
        .expect_err("and it must not transpile either");
    assert!(err.contains("E-UNKNOWN-IDENT"), "{err}");
}

/// THE FALSE-POSITIVE GUARD FOR THE MODULE-KEYED LAYER above — the one that would have made the row
/// unsafe if it were wrong.
///
/// The injected HTTP preludes import `Core.Native.Http as NativeHttp` themselves
/// (`src/cli/http_request_prelude.rs`), and the serve prelude's `Http.serve` body CALLS
/// `NativeHttp.registerServe`. So a module-keyed refusal that ran AFTER prelude injection would
/// reject every `import Core.Http;` program in the repo — the whole `examples/web/*` corpus, i.e.
/// exactly the Invariant-1 corpus the refusal exists to protect.
///
/// It does not, because both refusal sites run pre-expansion (`cmd_transpile` refuses the
/// `lex_parse` output; `transpile_program` refuses before `check_and_expand`). This test pins that
/// ordering from the outside: an ordinary `Core.Http` program that never registers a handler must
/// still transpile. If someone moves the gate after expansion, this goes red rather than the
/// breakage reaching the corpus.
#[test]
fn an_ordinary_core_http_program_still_transpiles_after_the_module_keyed_refusal() {
    const FRIENDLY_HTTP_PROGRAM: &str = r#"
package Main;

import Core.Http;
import Core.Http.Request;
import Core.Http.Response;
import Core.Output;

function reply(Request req): Response {
  return Response.text(200, req.path);
}

function main(): void {
  Output.printLine("ok");
}
"#;
    let prog = phorj::cli::parse_program(FRIENDLY_HTTP_PROGRAM).expect("parses");
    let php = phorj::cli::transpile_program(&prog, FRIENDLY_HTTP_PROGRAM)
        .expect("a Core.Http program that never registers a handler must still transpile");
    assert!(
        php.contains("class Request"),
        "the injected Core.Http surface must still be emitted: {}",
        &php[..php.len().min(200)]
    );
}

/// THE FALSE-POSITIVE GUARD, and the reason the refusal is keyed on the CALL rather than on the
/// `Core.Http` import or on the `Web` entry kind.
///
/// Both of the obvious cheaper keys are wrong here, and each was checked rather than assumed:
///   * the injected `class Http` is present in EVERY `import Core.Http;` program, so an import-keyed
///     refusal would reject the five shipped `examples/web/*`;
///   * `#[Entry(kind: EntryKind.Web)]` programs transpile clean TODAY — verified at the time on
///     `examples/web/core-http.phg` (since converted to the `examples/web/core-http/` project by
///     S3.3d) and `examples/web/handler.phg` — so an entry-kind-keyed refusal
///     would break them too, notwithstanding the spec sentence claiming that is "already the rule".
///
/// Either mistake breaks the example byte-identity glob, which is Invariant 1's corpus enforcement.
///
/// REPLACED by DEC-455.12 (S3.3d). This slot used to hold
/// `a_legacy_web_program_that_never_calls_http_serve_still_transpiles`, which pinned that a legacy
/// `(Request): Response` web entry kept transpiling so the shipped `examples/web/*` stayed green.
/// The narrowing makes that condition unreachable — such a program no longer type-checks, and the
/// corpus contains none. Rather than delete the slot, it now guards the state that REPLACED it: no
/// shipped example may reintroduce the retired shape. That is the regression this directory actually
/// needs, and nothing else in the suite asserts it.
#[test]
fn no_shipped_example_declares_the_retired_web_entry_shape() {
    let mut checked = 0usize;
    let mut offenders = Vec::new();
    let mut stack = vec![std::path::PathBuf::from("examples")];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).expect("examples/ is readable") {
            let path = e.expect("dir entry").path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().and_then(|x| x.to_str()) == Some("phg") {
                checked += 1;
                let src = std::fs::read_to_string(&path).expect("example is readable");
                // The retired shape is a `Web` ENTRY whose own signature takes a Request. Read the
                // parameter list of the first `function` after the attribute and nothing else — a
                // naive window would flag the `function(Request req)` CLOSURE that every correct D5
                // entry passes to `Http.serve`, i.e. it would fire on precisely the migrated shape it
                // is meant to bless (it did, on all four, before this was narrowed).
                // Split on the attribute AT LINE START. Matching it anywhere flags PROSE that merely
                // quotes it — `examples/web/handler.phg`'s migration note does exactly that, and this
                // test named it as an offender until the anchor was added.
                for w in src.split("\n#[Entry(kind: EntryKind.Web)]").skip(1) {
                    let Some(after_fn) = w.split_once("function ") else {
                        continue;
                    };
                    let Some((params, _)) = after_fn.1.split_once(')') else {
                        continue;
                    };
                    let Some((_, params)) = params.split_once('(') else {
                        continue;
                    };
                    if params.contains("Request") {
                        offenders.push(path.display().to_string());
                    }
                }
            }
        }
    }
    // Zero-denominator guard: a green run over no files proves nothing (the DEC-191 no-op-glob lesson).
    assert!(
        checked > 100,
        "expected the whole example corpus, walked only {checked} files — the walk is broken"
    );
    assert!(
        offenders.is_empty(),
        "these examples declare the RETIRED `(Request): Response` web entry: {offenders:?}"
    );
}

/// DEC-455.12 (S3.3d) — every `examples/web/*/serve.phg` REGISTERS a handler, SERVES a real request,
/// and is transpile-quarantined.
///
/// These files are gated by NOTHING else, and that is structural rather than accidental:
/// `collect_phg` returns early from any directory containing `src/`, so both differential globs skip
/// a project wholesale, and `find_main_phg` only ever picks a file named `main.phg`. A `serve.phg`
/// sitting at a project root is therefore invisible to the byte-identity corpus BY DESIGN — serve is
/// Invariant-14 quarantined and has no PHP leg to compare against. Without this test the shipped
/// serve demos could rot silently, which is exactly the DEC-191 failure shape.
#[test]
fn every_example_serve_phg_registers_serves_and_is_transpile_quarantined() {
    let mut found = 0usize;
    let mut responses: Vec<(&str, Vec<u8>)> = Vec::new();
    let mut dirs: Vec<_> = std::fs::read_dir("examples/web")
        .expect("examples/web is readable")
        .map(|e| e.expect("dir entry").path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    for dir in dirs {
        let entry = dir.join("serve.phg");
        if !entry.is_file() {
            continue;
        }
        found += 1;
        let label = entry.display().to_string();
        let unit = phorj::loader::load(&entry).unwrap_or_else(|e| panic!("load {label}: {e}"));
        // `loader::load` does NOT inject preludes — `unit.program` still has no `Core.Http`, so a
        // factory built from it dies with `undefined variable Http` at the `Http.serve` call. The
        // expansion chokepoint is what every backend goes through (Invariant 5), and it is what
        // `cli::treewalk_program` uses on the very same unit. The REIFIED form is the VM's — going
        // through `check_and_expand_reified` + the VM factory is Invariant 6: a vm-compile path that
        // skips it hides a VM≠tree-walker divergence off the differential's CLI path.
        let expanded = phorj::cli::check_and_expand(&unit.program, &unit.diag_src)
            .unwrap_or_else(|e| panic!("{label}: must type-check: {e}"));
        let (rprog, reified) = phorj::cli::check_and_expand_reified(&unit.program, &unit.diag_src)
            .unwrap_or_else(|e| panic!("{label}: must type-check (reified): {e}"));

        // BOTH BACKENDS. `phg serve`'s DEFAULT is the VM; asserting only the interpreter would leave
        // the shipped demos' real backend uncertified, and a multi-package unit is precisely the
        // layout the inline single-source serve tests never exercise.
        for (backend, fac) in [
            (
                "interp",
                phorj::serve::web_interp_factory(Arc::new(expanded))
                    .unwrap_or_else(|e| panic!("{label} [interp]: must register a handler: {e}")),
            ),
            (
                "vm",
                phorj::serve::web_vm_factory(Arc::new(rprog), Arc::new(reified))
                    .unwrap_or_else(|e| panic!("{label} [vm]: must register a handler: {e}")),
            ),
        ] {
            // It actually answers a request, over the deterministic in-memory transport.
            let mut fx =
                FixtureTransport::new(vec![b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n".to_vec()]);
            serve(&fac, &mut fx, false)
                .unwrap_or_else(|e| panic!("{label} [{backend}]: serve loop failed: {e}"));
            assert_eq!(
                fx.sent.len(),
                1,
                "{label} [{backend}]: exactly one response per request"
            );
            assert!(
                fx.sent[0].starts_with(b"HTTP/1.1 "),
                "{label} [{backend}]: not an HTTP/1.1 wire form: {:?}",
                String::from_utf8_lossy(&fx.sent[0])
                    .chars()
                    .take(60)
                    .collect::<String>()
            );
            responses.push((backend, fx.sent[0].clone()));
        }
        // Invariant 1 on the serve path: the two legs must agree BYTE for byte.
        assert_eq!(
            responses[0].1, responses[1].1,
            "{label}: interp and VM disagree on the served response"
        );
        responses.clear();

        // 3. It is quarantined from the PHP leg — this is what lets the logic in `src/` keep ITS leg.
        match phorj::cli::transpile_program(&unit.program, &unit.diag_src) {
            Ok(_) => {
                panic!("{label}: a file calling `Http.serve` must NOT transpile (Invariant 14)")
            }
            Err(e) => assert!(
                e.to_string().contains("E-TRANSPILE-SERVE"),
                "{label}: must be refused by the LADDER code, not incidentally: {e}"
            ),
        }
    }
    // Zero-denominator guard — if the layout changes and no serve.phg is found, fail loudly rather
    // than pass over an empty set.
    assert!(
        found >= 3,
        "expected the shipped serve demos under examples/web/*/serve.phg, found {found}"
    );
}

/// A web entry that REGISTERS and then FAULTS must not poison the thread for the next program.
///
/// This is the failure mode `web_handlers::register_on_this_thread`'s `reset()` exists for, written
/// down because a defensive line with no test is indistinguishable from a bandaid. The path: the
/// entry calls `Http.serve` (slot now full), then faults, so the run returns `Err` and the slot is
/// never TAKEN. Without the `reset`, the very next registration on that thread hits "called twice"
/// and a perfectly good program refuses to serve — with an error blaming a second `Http.serve` call
/// that does not exist in its source.
///
/// Deleting the `reset()` turns this test red; nothing else in the suite notices, which is how the
/// line came to be uncovered in the first place.
#[test]
fn a_faulting_web_entry_does_not_poison_the_next_registration_on_this_thread() {
    let poisoner = r#"
package Main;
import Core.Runtime.Entry; import Core.Runtime.EntryKind;
import Core.Http;
import Core.Http.Request;
import Core.Http.Response;
import Core.Http.ServeConfig;
import Core.Abort.panic;
#[Entry(kind: EntryKind.Web)] function web(): void {
  Http.serve(new ServeConfig(), function(Request req): Response {
    return Response.text(200, "never reached");
  });
  panic("boom");
}
#[Entry(kind: EntryKind.Cli)] function main(): void { }
"#;
    let prog = phorj::cli::parse_checked_program(poisoner).expect("type-checks");
    assert!(
        phorj::serve::web_interp_factory(Arc::new(prog)).is_err(),
        "an entry that faults after registering must fail at startup"
    );

    // SAME THREAD, immediately after: a clean program must still register and serve.
    let mut fx = FixtureTransport::new(vec![b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n".to_vec()]);
    serve(&ifac(&checked(HTTP_SERVE_PROGRAM)), &mut fx, false).expect("serve loop completes");
    assert_eq!(fx.sent[0], http("HTTP/1.1 200 OK", "home"));
}

/// S3.3c — the RETIRED `(Request): Response` web entry is refused at startup with a NAMED code and a
/// migration message.
///
/// Without this shape check the failure is an ARITY FAULT: `web_entry_name` resolves the legacy entry
/// (S3.3b deliberately kept that shape legal for `kind: Web`), the factory calls it with no arguments,
/// and the user's "startup error" is an opaque complaint about parameter counts on a program that was
/// perfectly well-formed one release ago. Every pre-D5 serve program takes this path exactly once, so
/// it is the single most-read diagnostic of the whole retirement.
#[test]
fn a_legacy_request_response_web_entry_is_refused_with_the_migration_code() {
    // DEC-455.12 (S3.3d): the CHECKER now rejects this shape (`E-ENTRY-SIG`, with a migration hint),
    // so a real program can no longer reach the runtime check below — `parse_checked_program` would
    // fail here, which is why the program is built UNCHECKED. The runtime check is kept as
    // defence-in-depth and this test is what keeps it covered: an untested defensive branch is how a
    // panic ships. The user-facing diagnostic of the retirement is now the checker's, asserted by
    // `checker::tests::entry_point::web_entry_no_longer_accepts_the_legacy_request_response_shape`.
    let prog =
        phorj::cli::parse_program(HTTP_HANDLE_PROGRAM).expect("the legacy shape still PARSES");
    // `HandlerFactory` is a boxed `dyn Fn`, so it is not `Debug` and `expect_err` cannot be used.
    let msg = match phorj::serve::web_interp_factory(Arc::new(prog)) {
        Ok(_) => panic!("the retired `(Request): Response` web entry must not produce a factory"),
        Err(d) => format!("{d:?}"),
    };
    assert!(
        msg.contains("E-SERVE-NO-HANDLER"),
        "the refusal must carry the code so `phg explain` can reach it: {msg}"
    );
    assert!(
        msg.contains("Http.serve"),
        "the refusal must name what to migrate TO: {msg}"
    );
}

/// Panel C10/F4 (RFC 9112 §6.3): a request whose `Content-Length` is not a valid non-negative integer
/// is answered `400 Bad Request` and the connection is CLOSED — never served as a body-less `200`
/// (`abc`, `-1` and a 24-digit value all got `200` before). Single-threaded transport path.
#[test]
fn malformed_content_length_gets_400_and_the_connection_closes() {
    use phorj::serve::TcpTransport;
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::time::Duration;

    let prog = program();
    let mut t = TcpTransport::bind("127.0.0.1:0").expect("bind ephemeral port");
    t.set_timeout(Some(Duration::from_secs(5)));
    let addr = t.local_addr().expect("addr");
    std::thread::spawn(move || {
        let _ = serve(&ifac(&prog), &mut t, false);
    });
    for bad in ["abc", "-1", "123456789012345678901234"] {
        let mut s = TcpStream::connect(addr).expect("connect");
        s.set_read_timeout(Some(Duration::from_secs(5)))
            .expect("client timeout");
        s.write_all(
            format!("POST / HTTP/1.1\r\nHost: x\r\nContent-Length: {bad}\r\n\r\n").as_bytes(),
        )
        .expect("write");
        let mut resp = Vec::new();
        s.read_to_end(&mut resp)
            .expect("read until the server closes");
        let text = String::from_utf8_lossy(&resp);
        assert!(
            text.starts_with("HTTP/1.1 400 "),
            "`Content-Length: {bad}` must be a 400, got:\n{text}"
        );
        assert!(text.contains("Connection: close"), "{text}");
    }
}

/// The same RFC 9112 §6.3 refusal on the multi-worker POOL path (the default `phg serve`), whose
/// per-connection loop is a separate framing site from the single-threaded transport above.
#[test]
fn malformed_content_length_gets_400_on_the_pool() {
    use phorj::serve::serve_pool;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};

    let prog = program();
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
    let addr = listener.local_addr().expect("addr");
    std::thread::spawn(move || {
        let _ = serve_pool(listener, ifac(&prog), None, false, 2);
    });
    let mut s = TcpStream::connect(addr).expect("connect");
    s.write_all(b"POST / HTTP/1.1\r\nHost: x\r\nContent-Length: -1\r\n\r\n")
        .expect("write");
    let mut resp = Vec::new();
    s.read_to_end(&mut resp)
        .expect("read until the worker closes");
    let text = String::from_utf8_lossy(&resp);
    assert!(text.starts_with("HTTP/1.1 400 "), "{text}");
    assert!(text.contains("Connection: close"), "{text}");
}
