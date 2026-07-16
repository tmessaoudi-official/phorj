//! The injected `Core.*` virtual modules: prelude sources, the CORE_MODULES registry
//! (UA-L2 — one row per module), and import-gated injection.

use super::*;

/// Type-check + de-alias an already-parsed program (the gate, minus lex/parse). De-aliases so every
/// backend sees alias-free types (aliases are front-end sugar; the checker validated them, including
/// cycles + built-in shadowing). Non-fatal warnings (the lint channel, M3 S2.5) render to stderr and
/// never gate the build. `diag_src` is the source used to render error carets — the single file for a
/// loose program, or `""` for a merged multi-file unit (where no single source aligns, so diagnostics
/// print message + position without a source line).
/// The canonical `Core.Json` value model, injected (below) when a program imports `Core.Json`. A
/// recursive enum over the JSON shapes; `Int`/`Float` are distinct (PHP-faithful, design-locked).
pub(super) const JSON_PRELUDE: &str = "enum Json { Null(), Bool(bool value), Int(int value), \
     Float(float value), String(string value), Array(List<Json> items), Object(Map<string, Json> entries) }";

/// The canonical `RoundingMode` enum, injected (below) when a program imports `Core.Decimal`
/// (M-NUM S2). Zero-payload variants — constructed `new HalfUp()` and matched `HalfUp()`, the
/// project's zero-payload variant convention — read by `Decimal.div`/`Decimal.round` via the
/// variant name. The seven modes mirror `value::RoundMode`. (Same [[core-json-and-injected-types]]
/// injected-type pattern as `Json`.)
pub(super) const ROUNDING_MODE_PRELUDE: &str =
    "enum RoundingMode { HalfUp(), HalfDown(), HalfEven(), Up(), Down(), Ceiling(), Floor() }";

/// True if the program imports the module `module` (e.g. `["Core", "Http"]`) either as a whole
/// (`import Core.Http`) OR via a **member-import** of one of its types, one segment deeper
/// (`import Core.Http.Router`). Import-redesign S2: a member-import must also pull in the injected
/// prelude, since the leaf type it names is one of that prelude's classes/enums.
pub(super) fn imports_module_or_member(prog: &Program, module: &[&str]) -> bool {
    prog.items.iter().any(|it| {
        matches!(it, crate::ast::Item::Import { path, .. }
            if (path.len() == module.len() || path.len() == module.len() + 1)
                && path.iter().zip(module).all(|(a, b)| a == b))
    })
}

/// The canonical `Core.Option<T>` value model (DEC-182, Wave B foundation), injected (below) when a
/// program imports `Core.Option`. The opt-in rich absence type — distinct from the built-in `T?`
/// (lightweight built-in absence + what stdlib returns); interconvert explicitly, never implicitly.
/// The FIRST *generic* injected enum: `T` is checked as `Ty::Param` (the inject chain runs before
/// `check_resolutions`) then erased by the downstream `erase_generics` — identical discipline to a
/// user-declared `enum Option<T>`. Matches the canonical shape in `examples/guide/generic-enums.phg`.
pub(super) const OPTION_PRELUDE: &str = "enum Option<T> { None, Some(T value) }";

/// The canonical `Core.Result<T, E>` value model (DEC-182, Wave B foundation), injected (below) when
/// a program imports `Core.Result`. Error-as-value: `Success(T)` or `Failure(E)`, where the error
/// payload `E` is a user enum. Pairs with the built-in `Error` marker + typed multi-catch; faults
/// stay uncatchable (bugs only). A generic injected enum like [`OPTION_PRELUDE`] — `T`/`E` are
/// erased downstream. Matches the canonical shape in `examples/guide/generic-enums.phg`.
pub(super) const RESULT_PRELUDE: &str = "enum Result<T, E> { Success(T value), Failure(E error) }";

/// The canonical `Core.Http` types, injected (below) when a program imports `Core.Http` (M6 W1 →
/// stdlib). The portable handler model — `handle(Request): Response` — at the value level: `Request`
/// and `Response` are immutable values; `Request.parse(bytes) -> Request?` and `resp.serialize()`
/// round-trip the HTTP/1.1 wire form. The bodies reuse `Core.Bytes`/`Core.String` (so the prelude also
/// imports them), so this is the same proven logic as `examples/web/handler.phg`, promoted to the
/// stdlib behind the static-method API (slice B0). Flows through every backend as ordinary classes.
/// `Core.Debug` (DEC-238) — the beautiful dumper. ONE function carrying both products (developer-
/// ruled): `Debug.dump(x)` renders deeply (the versioned v1 format in `native/debug.rs`), PRINTS,
/// and returns `Dumped<T>` — `.value()` is the pass-through, `.text()` the captured rendering.
/// `Debug.dd(x)` (dump + exit 1) and `Runtime.exit` land in slice 2. Nothing in the wind: only
/// reachable through `import Core.Debug`.
pub(super) const DEBUG_PRELUDE: &str = r#"
import Core.DebugSys;
import Core.Output;
import Core.Runtime;

// The dump result: BOTH the pass-through value and the rendering, explicitly.
class Dumped<T> {
  constructor(private T v, private string s) {}
  function value(): T { return this.v; }
  function text(): string { return this.s; }
}

class Debug {
  // Render + PRINT + carry: `int t = Debug.dump(price).value() * qty;` flows on;
  // `string snap = Debug.dump(cfg).text();` captures (already printed).
  static function dump<T>(T v): Dumped<T> {
    string s = DebugSys.render(v);
    Output.printLine(s);
    return new Dumped(v, s);
  }
  // dump-and-die (the debugging convention): print the rendering, then a CLEAN exit 1 (deliberate
  // abort — never a stack trace; that's `panic`'s job).
  static function dd<T>(T v): never {
    discard Debug.dump(v);
    Runtime.exit(1);
  }
}
"#;

/// `Core.Session` (W3, TOP-20 #3 blocker) — HTTP sessions for `phg serve`, on top of the
/// `Core.Http` `Request`/`Response` value types. THROW-FREE surface (in-memory store ops are
/// total). Security defaults ON — better than PHP's opt-in ini flags: the cookie is
/// `HttpOnly; SameSite=Lax; Path=/`; ids are 128-bit OS-entropy hex; an expired/unknown cookie id
/// silently gets a FRESH empty session (never resurrected); `regenerate()` (session-fixation
/// defense) is first-class. Values are strings (structured data goes through `Core.Json` — PHP's
/// serialized `$_SESSION` does the same under the hood). Native-only (`E-TRANSPILE-SESSION`).
pub(super) const SESSION_PRELUDE: &str = r#"
import Core.SessionSys;
import Core.Http;
import Core.Http.Request;
import Core.Http.Response;
import Core.String;
import Core.List;

class Session {
  private mutable string sid;
  private constructor(string s) { this.sid = s; }
  // Start (or resume) the session named by the request's `phorjsid` cookie — 30-minute idle TTL,
  // touched on every access (the gc_maxlifetime shape).
  static function start(Request req): Session {
    return Session.startWithTtl(req, 1800);
  }
  static function startWithTtl(Request req, int ttlSeconds): Session {
    string cand = Session.cookieSid(req);
    return new Session(SessionSys.acquire(cand, ttlSeconds));
  }
  private static function cookieSid(Request req): string {
    string? cookies = req.header("Cookie");
    if (var c = cookies) {
      List<string> parts = String.split(c, ";");
      for (string part in parts) {
        string p = String.trim(part);
        if (String.startsWith(p, "phorjsid=")) { return String.removePrefix(p, "phorjsid="); }
      }
    }
    return "";
  }
  function id(): string { return this.sid; }
  function get(string key): string? { return SessionSys.get(this.sid, key); }
  function set(string key, string value): void { discard SessionSys.set(this.sid, key, value); }
  function remove(string key): void { discard SessionSys.remove(this.sid, key); }
  // Sorted (deterministic) key listing.
  function keys(): List<string> { return SessionSys.keys(this.sid); }
  function destroy(): void { discard SessionSys.destroy(this.sid); }
  // The session-fixation defense: a FRESH id carrying the same data; the old id is dead
  // immediately. Call it on every privilege change (login/logout).
  function regenerate(): Session {
    this.sid = SessionSys.regenerate(this.sid);
    return this;
  }
  // Attach the session cookie to a response — HttpOnly + SameSite=Lax + Path=/ (secure defaults;
  // add `; Secure` yourself when serving over TLS).
  function apply(Response r): Response {
    return r.withHeader("Set-Cookie", "phorjsid={this.sid}; HttpOnly; SameSite=Lax; Path=/");
  }
}
"#;

pub(super) const HTTP_PRELUDE: &str = r#"
import Core.Bytes;
import Core.String;
import Core.List;
import Core.Regex;
class Request {
  constructor(public string method, public string path, public bytes body, private List<string> headerLines, private List<string> attrs) {}
  function header(string name): string? {
    for (string line in this.headerLines) {
      if (String.contains(line, ":")) {
        List<string> kv = String.splitOnce(line, ":");
        string key = String.trim(kv[0]);
        if (key == name) { return String.trim(kv[1]); }
      }
    }
    return null;
  }
  function param(string name): string? {
    mutable int i = 0;
    int n = List.length(this.attrs);
    while (i + 1 < n) {
      if (this.attrs[i] == name) { return this.attrs[i + 1]; }
      i += 2;
    }
    return null;
  }
  function withParams(List<string> p): Request {
    return new Request(this.method, this.path, this.body, this.headerLines, p);
  }
  static function parse(bytes raw): Request? {
    int sep = Bytes.find(raw, b"\x0d\x0a\x0d\x0a") ?? -1;
    if (sep < 0) { return null; }
    bytes headBytes = Bytes.slice(raw, 0, sep);
    bytes body = Bytes.slice(raw, sep + 4, Bytes.length(raw));
    string head = Bytes.toString(headBytes) ?? "";
    string nl = Bytes.toString(b"\x0d\x0a") ?? "";
    List<string> lines = String.split(head, nl);
    string requestLine = lines[0];
    List<string> rl = String.split(requestLine, " ");
    string method = rl[0];
    string path = rl[1];
    return new Request(method, path, body, lines, new List<string>());
  }
}
class Response {
  constructor(public int status, public bytes body, public List<string> headerLines) {}
  static function text(int status, string body): Response {
    return new Response(status, Bytes.fromString(body), ["Content-Type: text/plain"]);
  }
  // DEC-220 S2 — ergonomic, IMMUTABLE, chainable builders (the browser-bound sink). `html`/`json`
  // are 200-status constructors setting a sensible Content-Type; chain `.status(n)` to change it.
  static function html(string body): Response {
    return new Response(200, Bytes.fromString(body), ["Content-Type: text/html; charset=utf-8"]);
  }
  static function json(string body): Response {
    return new Response(200, Bytes.fromString(body), ["Content-Type: application/json"]);
  }
  // Each returns a NEW Response (headers-before-body is structural — Response is a value, so PHP's
  // "headers already sent" is impossible). `status` renames the field-free way to set the code;
  // `withHeader`/`withCookie` append a header line (immutable, like `Router.route`).
  function status(int newStatus): Response {
    return new Response(newStatus, this.body, this.headerLines);
  }
  function withHeader(string name, string value): Response {
    return new Response(this.status, this.body, List.concat(this.headerLines, ["{name}: {value}"]));
  }
  function withCookie(string name, string value): Response {
    return new Response(this.status, this.body, List.concat(this.headerLines, ["Set-Cookie: {name}={value}"]));
  }
  static function reason(int s): string {
    return if (s == 200) { "OK" }
      else { if (s == 400) { "Bad Request" }
      else { if (s == 404) { "Not Found" }
      else { "Internal Server Error" } } };
  }
  function serialize(): bytes {
    string nl = Bytes.toString(b"\x0d\x0a") ?? "";
    string reason = Response.reason(this.status);
    int st = this.status;
    string statusLine = "HTTP/1.1 {st} {reason}";
    int bodyLen = Bytes.length(this.body);
    string userHeaders = String.join(this.headerLines, nl);
    string head = "{statusLine}{nl}Content-Length: {bodyLen}{nl}{userHeaders}{nl}{nl}";
    return Bytes.concat(Bytes.fromString(head), this.body);
  }
}
class Route {
  constructor(public string method, public string pattern, public (Request) -> Response handler) {}
}
class Router {
  // `table` = the registered routes; `mws` = middleware applied (outermost-first) to every matched
  // handler. Middleware is `(Request, next) -> Response`: it may call `next(req)` to continue the
  // chain, or short-circuit (e.g. return 401 without calling `next`).
  constructor(private List<Route> table, private List<(Request, (Request) -> Response) -> Response> mws) {}
  function route(string method, string pattern, (Request) -> Response handler): Router {
    return new Router(List.concat(this.table, [new Route(method, pattern, handler)]), this.mws);
  }
  // Append a middleware (applies to every route this router handles). Chainable, immutable.
  function use((Request, (Request) -> Response) -> Response mw): Router {
    return new Router(this.table, List.concat(this.mws, [mw]));
  }
  // Mount a sub-router under `prefix`: run `build` on a fresh empty router, then merge each sub-route
  // with `prefix` prepended to its pattern and the sub-router's own middleware composed around its
  // handler (so group-scoped middleware applies). The parent's `use` middleware still applies on top
  // in `handle`.
  function group(string prefix, (Router) -> Router build): Router {
    var builder = build;
    Router sub = builder(new Router(new List<Route>(), new List<(Request, (Request) -> Response) -> Response>()));
    mutable List<Route> merged = this.table;
    for (Route r in sub.table) {
      var h = r.handler;
      var wrapped = Router.compose(sub.mws, h);
      merged = List.concat(merged, [new Route(r.method, prefix + r.pattern, wrapped)]);
    }
    return new Router(merged, this.mws);
  }
  // Fold a middleware list around a handler: first-registered runs OUTERMOST. Each step builds a
  // `function(req) => mw(req, prev)` closure capturing the middleware and the previously-wrapped handler.
  static function compose(List<(Request, (Request) -> Response) -> Response> mws, (Request) -> Response handler): (Request) -> Response {
    mutable var h = handler;
    mutable int i = List.length(mws) - 1;
    while (i >= 0) {
      var mw = mws[i];
      var prev = h;
      h = function(Request req) -> Response { return mw(req, prev); };
      i -= 1;
    }
    return h;
  }
  static function idStrs(List<string> xs): List<string> { return xs; }
  // A pattern segment is a parameter iff it is `{...}`. The inner text is `name` (bare) or
  // `name:regex` (constrained); the regex must match the WHOLE path segment.
  static function isParam(string seg): bool {
    return String.startsWith(seg, "\{") && String.endsWith(seg, "\}");
  }
  static function paramInner(string seg): string {
    // Drop only the OUTER braces (substring 1..len-1) — a constraint regex may itself contain braces
    // (`\d{4}`), so stripping every `{`/`}` would corrupt it. `substring(s, 1, -1)` = bytes[1..len-1]
    // on both backends and PHP `substr($s, 1, -1)`.
    return String.substring(seg, 1, -1);
  }
  static function paramName(string seg): string {
    string inner = Router.paramInner(seg);
    if (String.contains(inner, ":")) { List<string> kv = String.splitOnce(inner, ":"); return kv[0]; }
    return inner;
  }
  // A constrained segment matches its path component iff the (whole-segment-anchored) regex matches.
  static function constraintOk(string seg, string component): bool {
    string inner = Router.paramInner(seg);
    if (String.contains(inner, ":")) {
      List<string> kv = String.splitOnce(inner, ":");
      var re = Regex.compile("^(?:" + kv[1] + ")$");
      return Regex.matches(re, component);
    }
    return true; // a bare `{name}` matches any component
  }
  // Specificity score (higher = more specific), or -1 for no match. A literal segment scores 2, a
  // matching CONSTRAINED param scores 1, a bare param scores 0 — so literal > constrained > param.
  // A constrained param whose component fails its regex makes the whole route not match.
  static function segScore(string pattern, string path): int {
    List<string> ps = String.split(pattern, "/");
    List<string> xs = String.split(path, "/");
    if (List.length(ps) != List.length(xs)) { return -1; }
    mutable int score = 0;
    mutable int i = 0;
    int n = List.length(ps);
    while (i < n) {
      string p = ps[i];
      if (Router.isParam(p)) {
        if (!Router.constraintOk(p, xs[i])) { return -1; }
        if (String.contains(Router.paramInner(p), ":")) { score += 1; }
      } else {
        if (p != xs[i]) { return -1; }
        score += 2;
      }
      i += 1;
    }
    return score;
  }
  static function extractParams(string pattern, string path): List<string> {
    List<string> ps = String.split(pattern, "/");
    List<string> xs = String.split(path, "/");
    mutable List<string> out = Router.idStrs(new List<string>());
    mutable int i = 0;
    int n = List.length(ps);
    while (i < n) {
      string p = ps[i];
      if (Router.isParam(p)) {
        out = List.concat(out, [Router.paramName(p), xs[i]]);
      }
      i += 1;
    }
    return out;
  }
  function handle(Request req): Response {
    mutable int best = -1;
    mutable int bestScore = -1;
    mutable int idx = 0;
    for (Route r in this.table) {
      if (r.method == req.method) {
        int sc = Router.segScore(r.pattern, req.path);
        if (sc > bestScore) { best = idx; bestScore = sc; }
      }
      idx += 1;
    }
    if (best < 0) { return Response.text(404, "Not Found: {req.method} {req.path}"); }
    Route chosen = this.table[best];
    List<string> params = Router.extractParams(chosen.pattern, req.path);
    var composed = Router.compose(this.mws, chosen.handler);
    return composed(req.withParams(params));
  }
}
"#;

/// The `phg serve` bridge: the runtime's `respond(bytes): bytes` entry, synthesized to wrap a
/// user-defined `handle(Request): Response` (closes Batch-1 C). Injected only when `Core.Http` is
/// imported, a `handle` exists, and the user hasn't written their own `respond`. A malformed request
/// (parse returns null) becomes a 400 — HTTP policy lives here in Phorj, not in the Rust runtime.
pub(super) const HTTP_RESPOND_BRIDGE: &str = r#"
function respond(bytes raw): bytes {
  if (var req = Request.parse(raw)) {
    return handle(req).serialize();
  }
  return Response.text(400, "Bad Request").serialize();
}
"#;

/// The opaque compiled-`Regex` value model, injected when a program imports `Core.Regex` (Fork A,
/// `docs/specs/2026-06-28-core-regex-design.md`). A `Regex` value is built only by `Regex.compile`
/// (which validates via the `regex` crate); the `pattern` field is the **bare** pattern. It is public
/// so the transpiled `__phorj_regex_*` global helpers can read `$re->pattern` to build the
/// `/u`-delimited PHP `preg_*` form. Injected by [`inject_core_modules`] via the `Core.Regex`
/// registry row — a no-op unless `Core.Regex` is imported and no `Regex` class is already declared.
pub(super) const REGEX_PRELUDE: &str = "class Regex { constructor(public string pattern) {} }";

/// The `Secret<T>` opaque-wrapper type, injected when a program imports `Core.Secret` (Fork B,
/// `docs/specs/2026-06-28-secret-type-design.md`). A `Secret<T>` value is constructed `new Secret(x)`
/// `Core.Mail` (DEC-223) — the native mailer prelude, a TWIN of `Core.Db`: prelude classes wrap the
/// `Core.MailSys` natives, errors flow through the prelude-local `MailResult<T>` + a `<<Kind>>`-parsing
/// `MailError.fail`, and the transport credential is a `Core.Secret`. Native-only (`E-TRANSPILE-MAIL`
/// — see the pipeline ladder gate); every symbol import-gated (nothing in the wind). Surface notes
/// realized under bounded autonomy (developer to confirm, recorded in C-decisions DEC-230): the spec's
/// `new SmtpConfig(host, port, user, Secret pw)` 4-arg form is realized as the static factory
/// `SmtpConfig.withAuth(...)` (phorj has NO constructor default params / overloading — gap flagged in
/// KNOWN_ISSUES), and `new SendmailTransport()` path override is `SendmailTransport.at(path)`.
pub(super) const MAIL_PRELUDE: &str = r#"
import Core.MailSys;
import Core.String;
import Core.List;
// `Core.Secret` provides the opaque credential wrapper for `SmtpConfig.withAuth` (the Db.withPassword
// discipline): the SMTP password never sits in plaintext in user code and is never retained by the
// transport (only a redacted `smtp://host:port` description is stored).
import Core.Secret;

// Prelude-local result carrier (NOT Core.Result — see the Core.Db native docs on injection order).
enum MailResult<T> { Ok(T value), Err(string message) }

open class MailError implements Error {
  constructor(public string message) {}
  // The single classification point (the `DbError.fail` mechanism): natives tag failures with a
  // `<<Kind>>` marker; this strips it and throws the matching TYPED subtype, so
  // `catch (AuthFailed e)` is precise while `catch (MailError e)` still catches everything.
  static function fail(string message): never throws MailError {
    if (String.startsWith(message, "<<ConnectionFailed>>")) { throw new ConnectionFailed(String.removePrefix(message, "<<ConnectionFailed>>")); }
    if (String.startsWith(message, "<<AuthFailed>>")) { throw new AuthFailed(String.removePrefix(message, "<<AuthFailed>>")); }
    if (String.startsWith(message, "<<RecipientRejected>>")) { throw new RecipientRejected(String.removePrefix(message, "<<RecipientRejected>>")); }
    if (String.startsWith(message, "<<TlsError>>")) { throw new TlsError(String.removePrefix(message, "<<TlsError>>")); }
    if (String.startsWith(message, "<<InvalidAddress>>")) { throw new InvalidAddress(String.removePrefix(message, "<<InvalidAddress>>")); }
    if (String.startsWith(message, "<<MessageBuildFailed>>")) { throw new MessageBuildFailed(String.removePrefix(message, "<<MessageBuildFailed>>")); }
    if (String.startsWith(message, "<<Timeout>>")) { throw new MailTimeout(String.removePrefix(message, "<<Timeout>>")); }
    if (String.startsWith(message, "<<Io>>")) { throw new MailIo(String.removePrefix(message, "<<Io>>")); }
    throw new MailError(message);
  }
}

// Typed error taxonomy (spec §5, shaped like DbError's). `MailTimeout`/`MailIo` carry the Mail prefix
// because bare `Timeout` already belongs to Core.Db's taxonomy (two injected classes may not collide).
class ConnectionFailed extends MailError { constructor(string message) { parent.constructor(message); } }
class AuthFailed extends MailError { constructor(string message) { parent.constructor(message); } }
class RecipientRejected extends MailError { constructor(string message) { parent.constructor(message); } }
class TlsError extends MailError { constructor(string message) { parent.constructor(message); } }
class InvalidAddress extends MailError { constructor(string message) { parent.constructor(message); } }
class MessageBuildFailed extends MailError { constructor(string message) { parent.constructor(message); } }
class MailTimeout extends MailError { constructor(string message) { parent.constructor(message); } }
class MailIo extends MailError { constructor(string message) { parent.constructor(message); } }

// A typed, injection-safe address (spec §4): validated AT CONSTRUCTION (DEC-221 throwing ctor), so an
// `Address` value is valid-by-construction everywhere downstream — raw-header injection (the #1 PHP
// mail() footgun) is structurally impossible.
class Address {
  constructor(public string email, public string name) throws MailError {
    match (MailSys.addressCheck(email, name)) { MailResult.Ok(_) => Address.ok(), MailResult.Err(e) => MailError.fail(e)? };
  }
  // The name-less form (`Address.of("a@b.c")`) — phorj has no ctor default params.
  static function of(string email): Address throws MailError { return new Address(email, "")?; }
  private static function ok(): void {}
}

// An attachment source: a filesystem path (`fromFile`) or in-memory bytes (`fromBytes`). `mime` may be
// "" for `fromFile` (guessed from the extension; pass it explicitly for anything exotic).
class Attachment {
  private constructor(public string path, public string name, public string mime, public bytes? data) {}
  static function fromFile(string path): Attachment { return new Attachment(path, "", "", null); }
  static function fromFileTyped(string path, string mime): Attachment { return new Attachment(path, "", mime, null); }
  static function fromBytes(string name, string mime, bytes data): Attachment { return new Attachment("", name, mime, data); }
}

// Transport configs — plain data carriers `new Mailer(...)` dispatches on (match-over-union).
class SmtpConfig {
  // DEC-236 ctor defaults realize the spec's 4-arg form directly: `new SmtpConfig(host, port)` is
  // unauthenticated; `new SmtpConfig(host, port, user, secret)` authenticates. `withAuth` stays as
  // a thin compatibility alias. DEC-265: `tls` selects the TLS mode when authenticated — `"auto"`
  // (default: implicit TLS on port 465, STARTTLS-required otherwise), `"starttls"` (force STARTTLS-
  // required), or `"implicit"` (force TLS-from-connect). Any unrecognized value fails SAFE to the
  // required-TLS path — a typo can never downgrade to plaintext. `allowInsecureAuth = true` is the
  // explicit, loud opt-out permitting authenticated plaintext on a trusted network — the ONLY way to
  // express credentials-without-guaranteed-TLS (misuse-resistant surface, DEC-272). (A typed `SmtpTls`
  // enum is deferred: ctor default params must be literal constants, which an enum value is not.)
  constructor(public string host, public int port, public string user = "", public Secret<string>? password = null, public string tls = "auto", public bool allowInsecureAuth = false) {}
  static function withAuth(string host, int port, string user, Secret<string> password): SmtpConfig {
    return new SmtpConfig(host, port, user, password);
  }
}
class SendmailTransport {
  // `new SendmailTransport()` = the platform default (/usr/sbin/sendmail); pass a path to override.
  constructor(public string path = "") {}
  static function at(string path): SendmailTransport { return new SendmailTransport(path); }
}
class FileTransport { constructor(public string dir) {} }
class NullTransport { constructor() {} }

// The message builder (spec §4): chainable, accumulating `to`/`cc`/`bcc` on repeat calls. `.html`
// auto-derives a plaintext alternative (multipart/alternative); `.text` overrides it. Attachments:
// `.attach` (multipart/mixed) and `.attachInline(cid, …)` (multipart/related, referenced `cid:<cid>`).
class Email {
  private mutable MailHandle raw;
  constructor() {
    this.raw = MailSys.emailNew();
  }
  function from(Address a): Email throws MailError {
    this.raw = match (MailSys.from(this.raw, a.email, a.name)) { MailResult.Ok(h) => h, MailResult.Err(e) => MailError.fail(e)? };
    return this;
  }
  function replyTo(Address a): Email throws MailError {
    this.raw = match (MailSys.replyTo(this.raw, a.email, a.name)) { MailResult.Ok(h) => h, MailResult.Err(e) => MailError.fail(e)? };
    return this;
  }
  function to(Address a): Email throws MailError {
    this.raw = match (MailSys.to(this.raw, a.email, a.name)) { MailResult.Ok(h) => h, MailResult.Err(e) => MailError.fail(e)? };
    return this;
  }
  function cc(Address a): Email throws MailError {
    this.raw = match (MailSys.cc(this.raw, a.email, a.name)) { MailResult.Ok(h) => h, MailResult.Err(e) => MailError.fail(e)? };
    return this;
  }
  function bcc(Address a): Email throws MailError {
    this.raw = match (MailSys.bcc(this.raw, a.email, a.name)) { MailResult.Ok(h) => h, MailResult.Err(e) => MailError.fail(e)? };
    return this;
  }
  function subject(string s): Email throws MailError {
    this.raw = match (MailSys.subject(this.raw, s)) { MailResult.Ok(h) => h, MailResult.Err(e) => MailError.fail(e)? };
    return this;
  }
  function text(string body): Email throws MailError {
    this.raw = match (MailSys.text(this.raw, body)) { MailResult.Ok(h) => h, MailResult.Err(e) => MailError.fail(e)? };
    return this;
  }
  function html(string body): Email throws MailError {
    this.raw = match (MailSys.html(this.raw, body)) { MailResult.Ok(h) => h, MailResult.Err(e) => MailError.fail(e)? };
    return this;
  }
  function attach(Attachment a): Email throws MailError {
    if (var d = a.data) {
      this.raw = match (MailSys.attachBytes(this.raw, a.name, a.mime, d)) { MailResult.Ok(h) => h, MailResult.Err(e) => MailError.fail(e)? };
    } else {
      this.raw = match (MailSys.attachFile(this.raw, a.path, a.mime)) { MailResult.Ok(h) => h, MailResult.Err(e) => MailError.fail(e)? };
    }
    return this;
  }
  function attachInline(string cid, Attachment a): Email throws MailError {
    if (var d = a.data) {
      this.raw = match (MailSys.attachInlineBytes(this.raw, cid, a.mime, d)) { MailResult.Ok(h) => h, MailResult.Err(e) => MailError.fail(e)? };
    } else {
      this.raw = match (MailSys.attachInline(this.raw, cid, a.path, a.mime)) { MailResult.Ok(h) => h, MailResult.Err(e) => MailError.fail(e)? };
    }
    return this;
  }
  function handle(): MailHandle { return this.raw; }
}

// The mailer (spec §3): one of four transports behind the same `send`/`sendAll` surface. TLS on SMTP
// is STARTTLS-opportunistic (used when the server offers it — Mailpit-style no-TLS fakers still
// work); credentials only via `SmtpConfig.withAuth` + `Secret`.
class Mailer {
  private mutable MailHandle raw;
  constructor(SmtpConfig | SendmailTransport | FileTransport | NullTransport transport) throws MailError {
    this.raw = match (transport) {
      SmtpConfig s => Mailer.connectSmtp(s)?,
      SendmailTransport s => match (MailSys.sendmail(s.path)) { MailResult.Ok(h) => h, MailResult.Err(e) => MailError.fail(e)? },
      FileTransport f => match (MailSys.fileTransport(f.dir)) { MailResult.Ok(h) => h, MailResult.Err(e) => MailError.fail(e)? },
      NullTransport n => match (MailSys.nullTransport()) { MailResult.Ok(h) => h, MailResult.Err(e) => MailError.fail(e)? }
    };
  }
  private static function connectSmtp(SmtpConfig cfg): MailHandle throws MailError {
    mutable string pw = "";
    if (var s = cfg.password) { pw = s.expose(); }
    return match (MailSys.smtp(cfg.host, cfg.port, cfg.user, pw, cfg.tls, cfg.allowInsecureAuth)) { MailResult.Ok(h) => h, MailResult.Err(e) => MailError.fail(e)? };
  }
  // Arm DKIM signing (RSA key PEM as a `Secret`) for every subsequent send on this mailer.
  function dkim(string domain, string selector, Secret<string> privateKeyPem): Mailer throws MailError {
    match (MailSys.dkim(this.raw, domain, selector, privateKeyPem.expose())) { MailResult.Ok(_) => Mailer.ok(), MailResult.Err(e) => MailError.fail(e)? };
    return this;
  }
  function send(Email e): void throws MailError {
    match (MailSys.send(this.raw, e.handle())) { MailResult.Ok(_) => Mailer.ok(), MailResult.Err(e2) => MailError.fail(e2)? };
  }
  // Batch over one reused transport connection. Fail-fast: the first failure aborts with that
  // message's typed error (the count already delivered is in the message). Returns the sent count.
  function sendAll(List<Email> emails): int throws MailError {
    mutable List<MailHandle> handles = new List<MailHandle>();
    for (Email e in emails) { handles = List.append(handles, e.handle()); }
    return match (MailSys.sendAll(this.raw, handles)) { MailResult.Ok(n) => n, MailResult.Err(e) => MailError.fail(e)? };
  }
  private static function ok(): void {}
}
"#;

/// `Core.Fs` (W3, TOP-20 #5 blocker) — the TYPED filesystem prelude: every failure is a catchable
/// `FsError` subtype (contrast the older `Core.File`, whose write/delete failures are uncatchable
/// hard faults — its deprecation is a queued adjudication; this module is purely additive).
/// Listings are SORTED (determinism). Std-only, always compiled (no feature gate). The taxonomy is
/// Fs-PREFIXED throughout (`FsNotFound`, not `NotFound` — a bare generic name would CAPTURE
/// user-space classes via the injected-type discipline; caught live when `examples/web/server.phg`'s
/// own `NotFound` class collided).
pub(super) const FS_PRELUDE: &str = r#"
import Core.FsSys;
import Core.String;
import Core.List;

// Prelude-local result carrier (NOT Core.Result — the Core.Db injection-order rationale).
enum FsResult<T> { Ok(T value), Err(string message) }

open class FsError implements Error {
  constructor(public string message) {}
  static function fail(string message): never throws FsError {
    if (String.startsWith(message, "<<NotFound>>")) { throw new FsNotFound(String.removePrefix(message, "<<NotFound>>")); }
    if (String.startsWith(message, "<<PermissionDenied>>")) { throw new FsPermissionDenied(String.removePrefix(message, "<<PermissionDenied>>")); }
    if (String.startsWith(message, "<<AlreadyExists>>")) { throw new FsAlreadyExists(String.removePrefix(message, "<<AlreadyExists>>")); }
    if (String.startsWith(message, "<<NotADirectory>>")) { throw new FsNotADirectory(String.removePrefix(message, "<<NotADirectory>>")); }
    if (String.startsWith(message, "<<IsADirectory>>")) { throw new FsIsADirectory(String.removePrefix(message, "<<IsADirectory>>")); }
    if (String.startsWith(message, "<<DirNotEmpty>>")) { throw new FsDirNotEmpty(String.removePrefix(message, "<<DirNotEmpty>>")); }
    if (String.startsWith(message, "<<FsIo>>")) { throw new FsIo(String.removePrefix(message, "<<FsIo>>")); }
    throw new FsError(message);
  }
}

class FsNotFound extends FsError { constructor(string message) { parent.constructor(message); } }
class FsPermissionDenied extends FsError { constructor(string message) { parent.constructor(message); } }
class FsAlreadyExists extends FsError { constructor(string message) { parent.constructor(message); } }
class FsNotADirectory extends FsError { constructor(string message) { parent.constructor(message); } }
class FsIsADirectory extends FsError { constructor(string message) { parent.constructor(message); } }
class FsDirNotEmpty extends FsError { constructor(string message) { parent.constructor(message); } }
class FsIo extends FsError { constructor(string message) { parent.constructor(message); } }

// The typed filesystem surface (static module functions — filesystem state is ambient, so an
// instance would carry nothing; the SORTED listings + typed errors are the value).
class Fs {
  static function readText(string path): string throws FsError {
    return match (FsSys.readText(path)) { FsResult.Ok(v) => v, FsResult.Err(e) => FsError.fail(e)? };
  }
  static function readBytes(string path): bytes throws FsError {
    return match (FsSys.readBytes(path)) { FsResult.Ok(v) => v, FsResult.Err(e) => FsError.fail(e)? };
  }
  static function writeText(string path, string contents): void throws FsError {
    match (FsSys.writeText(path, contents)) { FsResult.Ok(_) => Fs.ok(), FsResult.Err(e) => FsError.fail(e)? };
  }
  static function writeBytes(string path, bytes contents): void throws FsError {
    match (FsSys.writeBytes(path, contents)) { FsResult.Ok(_) => Fs.ok(), FsResult.Err(e) => FsError.fail(e)? };
  }
  static function appendText(string path, string contents): void throws FsError {
    match (FsSys.appendText(path, contents)) { FsResult.Ok(_) => Fs.ok(), FsResult.Err(e) => FsError.fail(e)? };
  }
  static function copy(string from, string to): void throws FsError {
    match (FsSys.copy(from, to)) { FsResult.Ok(_) => Fs.ok(), FsResult.Err(e) => FsError.fail(e)? };
  }
  static function move(string from, string to): void throws FsError {
    match (FsSys.move(from, to)) { FsResult.Ok(_) => Fs.ok(), FsResult.Err(e) => FsError.fail(e)? };
  }
  static function delete(string path): void throws FsError {
    match (FsSys.delete(path)) { FsResult.Ok(_) => Fs.ok(), FsResult.Err(e) => FsError.fail(e)? };
  }
  static function size(string path): int throws FsError {
    return match (FsSys.size(path)) { FsResult.Ok(v) => v, FsResult.Err(e) => FsError.fail(e)? };
  }
  static function exists(string path): bool throws FsError {
    return match (FsSys.exists(path)) { FsResult.Ok(v) => v, FsResult.Err(e) => FsError.fail(e)? };
  }
  static function isFile(string path): bool throws FsError {
    return match (FsSys.isFile(path)) { FsResult.Ok(v) => v, FsResult.Err(e) => FsError.fail(e)? };
  }
  static function isDir(string path): bool throws FsError {
    return match (FsSys.isDir(path)) { FsResult.Ok(v) => v, FsResult.Err(e) => FsError.fail(e)? };
  }
  // Recursive create (mkdir -p semantics); removeDir removes ONE EMPTY dir; removeDirAll is the
  // loud recursive delete (refuses "/", "." and "..").
  static function createDir(string path): void throws FsError {
    match (FsSys.createDir(path)) { FsResult.Ok(_) => Fs.ok(), FsResult.Err(e) => FsError.fail(e)? };
  }
  static function removeDir(string path): void throws FsError {
    match (FsSys.removeDir(path)) { FsResult.Ok(_) => Fs.ok(), FsResult.Err(e) => FsError.fail(e)? };
  }
  static function removeDirAll(string path): void throws FsError {
    match (FsSys.removeDirAll(path)) { FsResult.Ok(_) => Fs.ok(), FsResult.Err(e) => FsError.fail(e)? };
  }
  // Entry NAMES of one directory, sorted; walk = every FILE under a root as sorted relative paths.
  static function listDir(string path): List<string> throws FsError {
    return match (FsSys.listDir(path)) { FsResult.Ok(v) => v, FsResult.Err(e) => FsError.fail(e)? };
  }
  static function walk(string root): List<string> throws FsError {
    return match (FsSys.walk(root)) { FsResult.Ok(v) => v, FsResult.Err(e) => FsError.fail(e)? };
  }
  static function tempDir(): string throws FsError {
    return match (FsSys.tempDir()) { FsResult.Ok(v) => v, FsResult.Err(e) => FsError.fail(e)? };
  }
  private static function ok(): void {}
}
"#;

/// `Core.HttpClient` (W3-2, TOP-20 #2 blocker) — the sync HTTP client prelude (the Core.Db/Mail
/// architecture). Taxonomy names are prefixed where a bare name is already taken by another injected
/// taxonomy (`HttpTimeout`/`HttpTlsError`/`HttpConnectionFailed` — `Timeout`/`TlsError`/
/// `ConnectionFailed`/`ConnectionError` belong to Core.Db / Core.Mail; injected-class dedup would
/// silently CAPTURE the other module's class — the cross-prelude collision smell recorded in
/// KNOWN_ISSUES). Native-only (`E-TRANSPILE-HTTPCLIENT`).
pub(super) const HTTP_CLIENT_PRELUDE: &str = r#"
import Core.HttpClientSys;
import Core.String;
import Core.List;
import Core.Bytes;

// Prelude-local result carrier (NOT Core.Result — the Core.Db injection-order rationale).
enum HcResult<T> { Ok(T value), Err(string message) }

open class HttpClientError implements Error {
  constructor(public string message) {}
  // The single classification point (the DbError.fail mechanism): `<<Kind>>` marker → typed subtype.
  static function fail(string message): never throws HttpClientError {
    if (String.startsWith(message, "<<InvalidUrl>>")) { throw new InvalidUrl(String.removePrefix(message, "<<InvalidUrl>>")); }
    if (String.startsWith(message, "<<ConnectionFailed>>")) { throw new HttpConnectionFailed(String.removePrefix(message, "<<ConnectionFailed>>")); }
    if (String.startsWith(message, "<<Timeout>>")) { throw new HttpTimeout(String.removePrefix(message, "<<Timeout>>")); }
    if (String.startsWith(message, "<<TlsError>>")) { throw new HttpTlsError(String.removePrefix(message, "<<TlsError>>")); }
    if (String.startsWith(message, "<<ProtocolError>>")) { throw new ProtocolError(String.removePrefix(message, "<<ProtocolError>>")); }
    if (String.startsWith(message, "<<TooManyRedirects>>")) { throw new TooManyRedirects(String.removePrefix(message, "<<TooManyRedirects>>")); }
    if (String.startsWith(message, "<<TooLarge>>")) { throw new TooLarge(String.removePrefix(message, "<<TooLarge>>")); }
    if (String.startsWith(message, "<<BlockedAddress>>")) { throw new BlockedAddress(String.removePrefix(message, "<<BlockedAddress>>")); }
    throw new HttpClientError(message);
  }
}

class InvalidUrl extends HttpClientError { constructor(string message) { parent.constructor(message); } }
class HttpConnectionFailed extends HttpClientError { constructor(string message) { parent.constructor(message); } }
class HttpTimeout extends HttpClientError { constructor(string message) { parent.constructor(message); } }
class HttpTlsError extends HttpClientError { constructor(string message) { parent.constructor(message); } }
class ProtocolError extends HttpClientError { constructor(string message) { parent.constructor(message); } }
class TooManyRedirects extends HttpClientError { constructor(string message) { parent.constructor(message); } }
class TooLarge extends HttpClientError { constructor(string message) { parent.constructor(message); } }
// DEC-270 SSRF guard: the URL resolved to a private/link-local/metadata address the client refuses by
// default. Pass `.allowPrivateHosts(true)` to permit private ranges deliberately (loopback is allowed).
class BlockedAddress extends HttpClientError { constructor(string message) { parent.constructor(message); } }

// A completed response: status, headers (names lowercased), body as text or bytes. Inert data
// behind an opaque handle — reading it never re-touches the network.
class HttpResponse {
  constructor(private HcHandle raw) {}
  function status(): int throws HttpClientError {
    return match (HttpClientSys.status(this.raw)) { HcResult.Ok(v) => v, HcResult.Err(e) => HttpClientError.fail(e)? };
  }
  // The named header's value, or null when absent (names are case-insensitive).
  function header(string name): string? throws HttpClientError {
    return match (HttpClientSys.header(this.raw, name)) { HcResult.Ok(v) => v, HcResult.Err(e) => HttpClientError.fail(e)? };
  }
  function headerNames(): List<string> throws HttpClientError {
    return match (HttpClientSys.headerNames(this.raw)) { HcResult.Ok(v) => v, HcResult.Err(e) => HttpClientError.fail(e)? };
  }
  // The body as UTF-8 text (a non-UTF-8 body is a clean ProtocolError steering to bodyBytes()).
  function body(): string throws HttpClientError {
    return match (HttpClientSys.bodyText(this.raw)) { HcResult.Ok(v) => v, HcResult.Err(e) => HttpClientError.fail(e)? };
  }
  function bodyBytes(): bytes throws HttpClientError {
    return match (HttpClientSys.bodyBytes(this.raw)) { HcResult.Ok(v) => v, HcResult.Err(e) => HttpClientError.fail(e)? };
  }
}

// The client (instance-based, chainable config): 30 s timeout + 5 redirects by default; TLS via
// bundled Mozilla roots; response size capped (64 MB); header CR/LF injection rejected at the gate;
// URL userinfo rejected (send credentials in a header). NOT in v1 (documented): HTTP/2, keep-alive
// pooling, proxies, cookies.
class HttpClient {
  public mutable int timeoutMs;
  public mutable int maxRedirects;
  // DEC-270: SSRF guard defaults to ON (false = private/link-local/metadata destinations refused;
  // loopback is always allowed). Opt in to reach private ranges deliberately (internal APIs, sidecars
  // beyond loopback) with `.allowPrivateHosts(true)`.
  public mutable bool allowPrivate;
  constructor() {
    this.timeoutMs = 30000;
    this.maxRedirects = 5;
    this.allowPrivate = false;
  }
  function timeout(int ms): HttpClient { this.timeoutMs = ms; return this; }
  function redirects(int n): HttpClient { this.maxRedirects = n; return this; }
  function allowPrivateHosts(bool v): HttpClient { this.allowPrivate = v; return this; }
  function get(string url): HttpResponse throws HttpClientError {
    return this.send("GET", url, new List<string>(), new List<string>(), Bytes.fromString(""))?;
  }
  function post(string url, string contentType, string body): HttpResponse throws HttpClientError {
    return this.send("POST", url, ["Content-Type"], [contentType], Bytes.fromString(body))?;
  }
  function put(string url, string contentType, string body): HttpResponse throws HttpClientError {
    return this.send("PUT", url, ["Content-Type"], [contentType], Bytes.fromString(body))?;
  }
  function delete(string url): HttpResponse throws HttpClientError {
    return this.send("DELETE", url, new List<string>(), new List<string>(), Bytes.fromString(""))?;
  }
  // The general form: any method, parallel header name/value lists, a bytes body.
  function send(string method, string url, List<string> headerNames, List<string> headerValues, bytes body): HttpResponse throws HttpClientError {
    HcHandle h = match (HttpClientSys.request(method, url, headerNames, headerValues, body, this.timeoutMs, this.maxRedirects, this.allowPrivate)) { HcResult.Ok(v) => v, HcResult.Err(e) => HttpClientError.fail(e)? };
    return new HttpResponse(h);
  }
}
"#;

/// and read only through `expose()` — the `value` field is private, and a `Secret` instance is not a
/// `string`, so printing/interpolating it is a clean type error (the primary, loud guarantee; no
/// runtime `***`). Reuses the generic-class machinery (`Box<T>`) wholesale — no new `Op`/`Value`/`Ty`.
/// Injected by [`inject_core_modules`] via the `Core.Secret` registry row — a no-op unless
/// `Core.Secret` is imported and no `Secret` class is already declared. The transpiler adds `final`
/// + `#[\SensitiveParameter]` for this class by name.
pub(super) const SECRET_PRELUDE: &str =
    "class Secret<T> { constructor(private T value) {} function expose(): T { return this.value; } }";

/// `Core.Uri` (DEC-240) — one immutable RFC 3986 `Uri` class with the typed `UriError` taxonomy.
/// The instance state is a single validated RAW string; every accessor/wither/operation calls a
/// `Core.UriSys` native over it (`src/native/uri/`), whose Rust kernel is pinned byte-for-byte to
/// the transpile twin — PHP 8.5's always-on `Uri\Rfc3986\Uri` (probe record:
/// `docs/research/2026-07-16-uri-twin-probes.md`) — so byte-identity holds with NO ladder
/// quarantine. Fallible natives return the new raw form or a `<<E>>`-sentinel message (`<` is
/// malformed anywhere in a URI, so the sentinel is collision-free); `UriError.fail` classifies
/// the message into the per-component taxonomy (richer than PHP's single `InvalidUriException`,
/// while the MESSAGES stay twin-identical). Getters are the NORMALIZED view (lowercased
/// scheme/host, dot-segments removed, unreserved percent-escapes decoded); the `raw*` family
/// returns the form as written.
pub(super) const URI_PRELUDE: &str = r#"
import Core.UriSys;
import Core.String;

open class UriError implements Error {
  constructor(public string message) {}
  static function fail(string message): never throws UriError {
    if (message == "The port is out of range") { throw new UriPortOutOfRange(message); }
    if (message == "The specified base URI must be absolute") { throw new UriBaseNotAbsolute(message); }
    if (message == "The specified scheme is malformed") { throw new UriBadScheme(message); }
    if (message == "The specified userinfo is malformed") { throw new UriBadUserInfo(message); }
    if (message == "The specified host is malformed") { throw new UriBadHost(message); }
    if (message == "The specified port is malformed") { throw new UriBadPort(message); }
    if (message == "The specified path is malformed") { throw new UriBadPath(message); }
    if (message == "The specified query is malformed") { throw new UriBadQuery(message); }
    if (message == "The specified fragment is malformed") { throw new UriBadFragment(message); }
    throw new UriMalformed(message);
  }
}
class UriMalformed extends UriError { constructor(string message) { parent.constructor(message); } }
class UriBadScheme extends UriError { constructor(string message) { parent.constructor(message); } }
class UriBadUserInfo extends UriError { constructor(string message) { parent.constructor(message); } }
class UriBadHost extends UriError { constructor(string message) { parent.constructor(message); } }
class UriBadPort extends UriError { constructor(string message) { parent.constructor(message); } }
class UriPortOutOfRange extends UriError { constructor(string message) { parent.constructor(message); } }
class UriBadPath extends UriError { constructor(string message) { parent.constructor(message); } }
class UriBadQuery extends UriError { constructor(string message) { parent.constructor(message); } }
class UriBadFragment extends UriError { constructor(string message) { parent.constructor(message); } }
class UriBaseNotAbsolute extends UriError { constructor(string message) { parent.constructor(message); } }

class Uri {
  private constructor(public string raw) {}
  static function parse(string s): Uri throws UriError { return Uri.wrap(UriSys.parse(s))?; }
  static function wrap(string r): Uri throws UriError {
    if (String.startsWith(r, "<<E>>")) { return UriError.fail(String.removePrefix(r, "<<E>>"))?; }
    return new Uri(r);
  }
  function scheme(): string? { return UriSys.scheme(this.raw); }
  function rawScheme(): string? { return UriSys.rawScheme(this.raw); }
  function userInfo(): string? { return UriSys.userInfo(this.raw); }
  function rawUserInfo(): string? { return UriSys.rawUserInfo(this.raw); }
  function username(): string? { return UriSys.username(this.raw); }
  function rawUsername(): string? { return UriSys.rawUsername(this.raw); }
  function password(): string? { return UriSys.password(this.raw); }
  function rawPassword(): string? { return UriSys.rawPassword(this.raw); }
  function host(): string? { return UriSys.host(this.raw); }
  function rawHost(): string? { return UriSys.rawHost(this.raw); }
  function port(): int? { return UriSys.port(this.raw); }
  function path(): string { return UriSys.path(this.raw); }
  function rawPath(): string { return UriSys.rawPath(this.raw); }
  function query(): string? { return UriSys.query(this.raw); }
  function rawQuery(): string? { return UriSys.rawQuery(this.raw); }
  function fragment(): string? { return UriSys.fragment(this.raw); }
  function rawFragment(): string? { return UriSys.rawFragment(this.raw); }
  function withScheme(string? scheme): Uri throws UriError { return Uri.wrap(UriSys.withScheme(this.raw, scheme))?; }
  function withUserInfo(string? userInfo): Uri throws UriError { return Uri.wrap(UriSys.withUserInfo(this.raw, userInfo))?; }
  function withHost(string? host): Uri throws UriError { return Uri.wrap(UriSys.withHost(this.raw, host))?; }
  function withPort(int? port): Uri throws UriError { return Uri.wrap(UriSys.withPort(this.raw, port))?; }
  function withPath(string path): Uri throws UriError { return Uri.wrap(UriSys.withPath(this.raw, path))?; }
  function withQuery(string? query): Uri throws UriError { return Uri.wrap(UriSys.withQuery(this.raw, query))?; }
  function withFragment(string? fragment): Uri throws UriError { return Uri.wrap(UriSys.withFragment(this.raw, fragment))?; }
  function resolve(string reference): Uri throws UriError { return Uri.wrap(UriSys.resolve(this.raw, reference))?; }
  function equals(Uri other): bool { return UriSys.equals(this.raw, other.raw, false); }
  function equalsIncludingFragment(Uri other): bool { return UriSys.equals(this.raw, other.raw, true); }
  function toString(): string { return UriSys.toText(this.raw); }
  function toRawString(): string { return this.raw; }
}
"#;

/// The `Core.Time` value model (M-TIME, `docs/specs/2026-06-28-m-time-design.md`): the pure-Phorj
/// `Instant`, `Duration`, `Date`, and `DateTime` classes. Because the prelude is run through the same
/// backends and transpiler as user code, all calendar and formatting math is byte-identical by
/// construction; the only native is the clock seam (the `Core.Time` module in `src/native/time.rs`).
/// The model is UTC-only because timezones are non-deterministic and would break the byte-identity
/// spine. Calendar math uses Hinnant's truncating-division-safe civil/day conversions, which port
/// verbatim since Phorj int division truncates toward zero (PHP `intdiv`).
pub(super) const TIME_PRELUDE: &str = r#"
class Duration {
  constructor(public int ms) {}
  static function milliseconds(int n) -> Duration { return new Duration(n); }
  static function seconds(int n) -> Duration { return new Duration(n * 1000); }
  static function minutes(int n) -> Duration { return new Duration(n * 60000); }
  static function hours(int n) -> Duration { return new Duration(n * 3600000); }
  static function days(int n) -> Duration { return new Duration(n * 86400000); }
  function toMilliseconds() -> int { return this.ms; }
  function toSeconds() -> int { return this.ms / 1000; }
  function toMinutes() -> int { return this.ms / 60000; }
  function toHours() -> int { return this.ms / 3600000; }
  function toDays() -> int { return this.ms / 86400000; }
  function plus(Duration o) -> Duration { return new Duration(this.ms + o.ms); }
  function minus(Duration o) -> Duration { return new Duration(this.ms - o.ms); }
  function negate() -> Duration { return new Duration(-this.ms); }
  function isZero() -> bool { return this.ms == 0; }
  function isNegative() -> bool { return this.ms < 0; }
}
class Date {
  constructor(public int epochDay) {}
  // Howard Hinnant's days-from-civil / civil-from-days (truncating-division safe; Phorj int `/` is
  // truncate-toward-zero = PHP intdiv). `daysFromCivil`/`civil`/`pad2` are low-level building blocks
  // reused by `DateTime`; the everyday API is `of`/`year`/`month`/`day`/`addDays`/`toString`.
  static function daysFromCivil(int y, int m, int d) -> int {
    int yy = y - (if (m <= 2) { 1 } else { 0 });
    int era = (if (yy >= 0) { yy } else { yy - 399 }) / 400;
    int yoe = yy - era * 400;
    int doy = (153 * (if (m > 2) { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    int doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    return era * 146097 + doe - 719468;
  }
  static function civil(int z) -> List<int> {
    int zz = z + 719468;
    int era = (if (zz >= 0) { zz } else { zz - 146096 }) / 146097;
    int doe = zz - era * 146097;
    int yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    int y = yoe + era * 400;
    int doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    int mp = (5 * doy + 2) / 153;
    int d = doy - (153 * mp + 2) / 5 + 1;
    int m = if (mp < 10) { mp + 3 } else { mp - 9 };
    return [y + (if (m <= 2) { 1 } else { 0 }), m, d];
  }
  static function pad2(int n) -> string { return if (n < 10) { "0{n}" } else { "{n}" }; }
  // Zero-pad a non-negative year to 4 digits (ISO `YYYY`); proleptic negatives are emitted verbatim.
  static function pad4(int n) -> string {
    return if (n < 0) { "{n}" } else { if (n < 10) { "000{n}" } else { if (n < 100) { "00{n}" } else { if (n < 1000) { "0{n}" } else { "{n}" } } } };
  }
  static function of(int y, int m, int d) -> Date { return new Date(Date.daysFromCivil(y, m, d)); }
  static function ofEpochDay(int d) -> Date { return new Date(d); }
  function year() -> int { return Date.civil(this.epochDay)[0]; }
  function month() -> int { return Date.civil(this.epochDay)[1]; }
  function day() -> int { return Date.civil(this.epochDay)[2]; }
  function addDays(int n) -> Date { return new Date(this.epochDay + n); }
  function minusDays(int n) -> Date { return new Date(this.epochDay - n); }
  function daysUntil(Date o) -> int { return o.epochDay - this.epochDay; }
  // 1=Mon … 7=Sun (ISO-8601). epochDay 0 = 1970-01-01 = Thursday.
  function dayOfWeek() -> int {
    int w = (this.epochDay + 3) % 7;
    return (if (w < 0) { w + 7 } else { w }) + 1;
  }
  function isLeapYear() -> bool {
    int y = this.year();
    return (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0);
  }
  function isBefore(Date o) -> bool { return this.epochDay < o.epochDay; }
  function isAfter(Date o) -> bool { return this.epochDay > o.epochDay; }
  function compareTo(Date o) -> int {
    return if (this.epochDay < o.epochDay) { -1 } else { if (this.epochDay > o.epochDay) { 1 } else { 0 } };
  }
  function toString() -> string {
    List<int> c = Date.civil(this.epochDay);
    return "{Date.pad4(c[0])}-{Date.pad2(c[1])}-{Date.pad2(c[2])}";
  }
}
class Instant {
  constructor(public int ms) {}
  static function ofEpochMilliseconds(int m) -> Instant { return new Instant(m); }
  static function ofEpochSeconds(int s) -> Instant { return new Instant(s * 1000); }
  static function now() -> Instant { return new Instant(Time.nowMilliseconds()); }
  function epochMilliseconds() -> int { return this.ms; }
  function epochSeconds() -> int { return this.ms / 1000; }
  function plus(Duration d) -> Instant { return new Instant(this.ms + d.ms); }
  function minus(Duration d) -> Instant { return new Instant(this.ms - d.ms); }
  function durationSince(Instant o) -> Duration { return new Duration(this.ms - o.ms); }
  function isBefore(Instant o) -> bool { return this.ms < o.ms; }
  function isAfter(Instant o) -> bool { return this.ms > o.ms; }
  function compareTo(Instant o) -> int {
    return if (this.ms < o.ms) { -1 } else { if (this.ms > o.ms) { 1 } else { 0 } };
  }
  // Civil-date view (UTC, day-resolution): floor-divide milliseconds by a day (floor, not truncate, so a
  // pre-1970 instant maps to the right civil day).
  function toDate() -> Date {
    int day = if (this.ms >= 0) { this.ms / 86400000 } else { (this.ms - 86399999) / 86400000 };
    return Date.ofEpochDay(day);
  }
  // ── civil (wall-time) view, UTC ──────────────────────────────────────────────────────────────
  // An `Instant` is also the human date-time: it exposes year/month/day/hour/minute/second/milliseconds and
  // an ISO-8601 string. (No separate `DateTime` class — that name collides with PHP's built-in, and
  // `Instant` already IS the point in time; fields are derived on demand.) `ofCivil` builds an instant
  // from broken-down UTC fields.
  static function ofCivil(int y, int mo, int d, int h, int mi, int s) -> Instant {
    int day = Date.daysFromCivil(y, mo, d);
    return new Instant(day * 86400000 + h * 3600000 + mi * 60000 + s * 1000);
  }
  // Milliseconds within the current UTC day, always in [0, 86399999] (uses the floored epoch-day).
  function millisecondsOfDay() -> int {
    int day = if (this.ms >= 0) { this.ms / 86400000 } else { (this.ms - 86399999) / 86400000 };
    return this.ms - day * 86400000;
  }
  function year() -> int { return this.toDate().year(); }
  function month() -> int { return this.toDate().month(); }
  function day() -> int { return this.toDate().day(); }
  function dayOfWeek() -> int { return this.toDate().dayOfWeek(); }
  function hour() -> int { return this.millisecondsOfDay() / 3600000; }
  function minute() -> int { return (this.millisecondsOfDay() / 60000) % 60; }
  function second() -> int { return (this.millisecondsOfDay() / 1000) % 60; }
  function milliseconds() -> int { return this.millisecondsOfDay() % 1000; }
  // ISO-8601 UTC: `YYYY-MM-DDTHH:MM:SSZ` (always `Z`; second-resolution, sub-second dropped). For any
  // other layout, interpolate the accessors directly (Phorj has first-class string interpolation).
  function toIso() -> string {
    List<int> c = Date.civil(this.toDate().epochDay);
    string date = "{Date.pad4(c[0])}-{Date.pad2(c[1])}-{Date.pad2(c[2])}";
    string time = "{Date.pad2(this.hour())}:{Date.pad2(this.minute())}:{Date.pad2(this.second())}";
    return "{date}T{time}Z";
  }
}
"#;

/// A virtual `Core.*` module: its import path, its optional injected prelude source, how it gates
/// (whole-module-only vs also member-imports), and the injected member-type names that must be
/// import-qualified (the `module_of` contribution). UA-L2 (registry-unification): the single source
/// for BOTH the prelude-injection fold ([`inject_core_modules`]) AND the injected-type discipline
/// ([`core_module_of`]) — so a new Core module (Db, HTTP expansions) is ONE row here, not edits in
/// the eight `inject_*_prelude` fns plus the hand-synced `module_of` match this replaced.
pub(super) struct VirtualModule {
    /// The import path segments, e.g. `["Core", "Http"]`. Gates injection; also the qualifier root.
    module: &'static [&'static str],
    /// The `module_of` return value for this row's `bare_types` (the dotted module below `Core.`),
    /// e.g. `"Http"`, `"Time"`, `"Runtime.Integer"`. Only meaningful when `bare_types` is non-empty.
    qualifier: &'static str,
    /// The prelude source to inject when the module is imported; `None` for attribute-only modules
    /// (`Core.DI`/`Core.Runtime*`) that contribute to `module_of` but inject no enum/class prelude.
    src: Option<&'static str>,
    /// The conditionally-injected `respond` serve-bridge source (Http only) — appended when the
    /// program defines `handle` and no `respond`. The one honest residual special-case.
    respond_bridge: Option<&'static str>,
    /// `true` → a member-import (`import Core.Http.Router`) also pulls the prelude in
    /// ([`imports_module_or_member`]); `false` → only a whole-module import (`import Core.Json`).
    member_gated: bool,
    /// Injected member-type names that `module_of` maps to `qualifier` — seeded EXPLICITLY to the
    /// pre-UA-L2 `module_of` set. NB: kept separate from the prelude's own declared names (the
    /// shadow-check derives those from the parsed source) — e.g. `Core.Time` injects `DateTime` too,
    /// but `DateTime` is deliberately NOT in `module_of` (see KNOWN_ISSUES). Fusing the two lists
    /// would silently change gating; `DateTime` is the proof they diverge.
    bare_types: &'static [&'static str],
}

/// `Core.Db` (DEC-208) — the enhanced-PDO surface: `Db`/`Statement`/`Row`/`DbError` phorj-source classes
/// wrapping the opaque `DbHandle` and the internal `Core.DbSys` natives. Each method calls a
/// `DbSys.*` native (which returns `Result<T, string>` — never a hard fault) and `match`es it, throwing
/// a catchable `DbError` on `Failure` (DEC-208 error-mechanism = prelude-wrapper; a phorj-source `throw`
/// is a real `Op::Throw`, byte-identical across both backends). `import Core.Db` transitively imports
/// `Core.DbSys` (the natives) + `Core.Result` (the carrier), so this module runs BEFORE them.
pub(super) const DB_PRELUDE: &str = r#"
import Core.DbSys;
import Core.List;
import Core.String;
// `Core.Map` is imported for the `queryMap<K,V>` hydration helpers the `desugar_db` pass generates
// into a `Core.Db` program (they build the result `Map` via `Map.set`); like the `Core.List` import
// above it makes the module's ops available to the generated helpers (and, as with `List`, to user
// code under an `import Core.Db`).
import Core.Map;
// `Core.Secret` (Fork B) provides the opaque `Secret<T>` credential wrapper used by the `Db.withPassword`
// factory (DEC-208 slice G): a connection password is passed as a `Secret<string>` so it never sits in
// plaintext in user code, and — because the driver parses it out of the DSN and retains only a redacted
// DSN — is masked in every connect error / log. Secret is registered before Db (see CORE_MODULES order),
// so this transitive import injects the class here exactly as List/String/Map above.
import Core.Secret;

// Prelude-local result carrier (NOT Core.Result — see the native docs on injection order).
enum DbResult<T> { Ok(T value), Err(string message) }

// Column NAMING STRATEGY (DEC-208 slice B2): the per-query mapping between DB column names and phorj
// field names, passed to `Statement.namingStrategy(...)`. Zero-payload variants (construct with
// `new Naming.SnakeToCamel()`, like `RoundingMode`). Member-gated (`import Core.Db.Naming;`) — nothing
// in the wind. The strategy is resolved AT COMPILE TIME by the `desugar_db` pass, so this type is only
// ever an argument literal; it carries no runtime state.
enum Naming { Exact(), SnakeToCamel() }

open class DbError implements Error {
  constructor(public string message) {}
  // `throw` is a statement, not an expression, so it cannot be a `match` arm value directly. This
  // `never`-returning helper lets a `DbResult.Err(e)` arm raise a catchable exception as an expression
  // (`DbResult.Err(e) => DbError.fail(e)`) — a call to a `never` function types as the bottom type,
  // unifying with the success arm's value type.
  //
  // It is ALSO the single classification point (DEC-208 slice C, spec §6): the native tags a driver
  // error with a `<<Kind>>` marker prefix (`src/native/db.rs` `err_kind`), and `fail` strips the marker
  // and throws the matching TYPED subtype. Because every Row/Statement/Db method — including the S2
  // `queryInto` hydration helpers — funnels its `DbResult.Err` through here, they all yield the precise
  // `catch (UniqueViolation e)` type with zero change at the call sites. An untagged message (a logic /
  // usage error, e.g. mixed bind styles, or a plain SQLite failure) throws the base `DbError`.
  static function fail(string message): never throws DbError {
    if (String.startsWith(message, "<<UniqueViolation>>")) { throw new UniqueViolation(String.removePrefix(message, "<<UniqueViolation>>")); }
    if (String.startsWith(message, "<<ConstraintViolation>>")) { throw new ConstraintViolation(String.removePrefix(message, "<<ConstraintViolation>>")); }
    if (String.startsWith(message, "<<ConnectionError>>")) { throw new ConnectionError(String.removePrefix(message, "<<ConnectionError>>")); }
    if (String.startsWith(message, "<<SerializationFailure>>")) { throw new SerializationFailure(String.removePrefix(message, "<<SerializationFailure>>")); }
    if (String.startsWith(message, "<<Timeout>>")) { throw new Timeout(String.removePrefix(message, "<<Timeout>>")); }
    if (String.startsWith(message, "<<SyntaxError>>")) { throw new SyntaxError(String.removePrefix(message, "<<SyntaxError>>")); }
    throw new DbError(message);
  }
}

// Typed error taxonomy (DEC-208 slice C, spec §6). Each `extends DbError`, so `catch (DbError e)`
// still catches EVERY DB error while `catch (UniqueViolation e)` catches exactly one kind. The native
// maps rusqlite (extended) result codes to the marker `DbError.fail` reads. `SerializationFailure` is
// the transient class `retry` targets (SQLite `SQLITE_BUSY`/`SQLITE_LOCKED`) — it is the spec's
// `Deadlock` under a single name. `Timeout` is part of the taxonomy contract; SQLite has no source for
// it yet (it arrives with query `.timeout(ms)`, slice D), so it is currently only ever caught, not thrown.
class UniqueViolation extends DbError { constructor(string message) { parent.constructor(message); } }
class ConstraintViolation extends DbError { constructor(string message) { parent.constructor(message); } }
class ConnectionError extends DbError { constructor(string message) { parent.constructor(message); } }
class SerializationFailure extends DbError { constructor(string message) { parent.constructor(message); } }
class Timeout extends DbError { constructor(string message) { parent.constructor(message); } }
class SyntaxError extends DbError { constructor(string message) { parent.constructor(message); } }

class Row {
  constructor(private DbHandle raw) {}
  function getInt(string column): int throws DbError {
    return match (DbSys.getInt(this.raw, column)) { DbResult.Ok(v) => v, DbResult.Err(e) => DbError.fail(e)? };
  }
  function getString(string column): string throws DbError {
    return match (DbSys.getString(this.raw, column)) { DbResult.Ok(v) => v, DbResult.Err(e) => DbError.fail(e)? };
  }
  function getFloat(string column): float throws DbError {
    return match (DbSys.getFloat(this.raw, column)) { DbResult.Ok(v) => v, DbResult.Err(e) => DbError.fail(e)? };
  }
  function getBool(string column): bool throws DbError {
    return match (DbSys.getBool(this.raw, column)) { DbResult.Ok(v) => v, DbResult.Err(e) => DbError.fail(e)? };
  }
  // Nullable accessors (DEC-208 S2): a SQL NULL yields `null` rather than throwing; a wrong non-null
  // storage type still throws. Used by the dynamic path and by the `queryInto` hydration of `T?` fields.
  function getIntOrNull(string column): int? throws DbError {
    return match (DbSys.getIntOrNull(this.raw, column)) { DbResult.Ok(v) => v, DbResult.Err(e) => DbError.fail(e)? };
  }
  function getStringOrNull(string column): string? throws DbError {
    return match (DbSys.getStringOrNull(this.raw, column)) { DbResult.Ok(v) => v, DbResult.Err(e) => DbError.fail(e)? };
  }
  function getFloatOrNull(string column): float? throws DbError {
    return match (DbSys.getFloatOrNull(this.raw, column)) { DbResult.Ok(v) => v, DbResult.Err(e) => DbError.fail(e)? };
  }
  function getBoolOrNull(string column): bool? throws DbError {
    return match (DbSys.getBoolOrNull(this.raw, column)) { DbResult.Ok(v) => v, DbResult.Err(e) => DbError.fail(e)? };
  }
  // Decimal accessors (DEC-208 slice E): a `decimal`/`decimal?` hydration field maps its column here —
  // exact money (a TEXT column is parsed exactly; never through float). Used by the dynamic path and by
  // the `queryInto` hydration of a `decimal` field (via `desugar_db`'s `accessor_for`).
  function getDecimal(string column): decimal throws DbError {
    return match (DbSys.getDecimal(this.raw, column)) { DbResult.Ok(v) => v, DbResult.Err(e) => DbError.fail(e)? };
  }
  function getDecimalOrNull(string column): decimal? throws DbError {
    return match (DbSys.getDecimalOrNull(this.raw, column)) { DbResult.Ok(v) => v, DbResult.Err(e) => DbError.fail(e)? };
  }
  // Typed ARRAY-column accessors (DEC-208 slice K) — a Postgres `int[]`/`text[]`/`float8[]`/`bool[]`
  // column reads as a typed `List<scalar>`. STRICT: a non-array column, a wrong element type, or a
  // NULL element throws a catchable DbError (filter NULL elements in SQL: `array_remove(col, NULL)`);
  // the `OrNull` forms admit a whole-array SQL NULL. Numeric/decimal arrays: select `col::text[]` and
  // read `getStringList` (the slice-E decimal-as-text discipline, element form).
  function getIntList(string column): List<int> throws DbError {
    return match (DbSys.getIntList(this.raw, column)) { DbResult.Ok(v) => v, DbResult.Err(e) => DbError.fail(e)? };
  }
  function getStringList(string column): List<string> throws DbError {
    return match (DbSys.getStringList(this.raw, column)) { DbResult.Ok(v) => v, DbResult.Err(e) => DbError.fail(e)? };
  }
  function getFloatList(string column): List<float> throws DbError {
    return match (DbSys.getFloatList(this.raw, column)) { DbResult.Ok(v) => v, DbResult.Err(e) => DbError.fail(e)? };
  }
  function getBoolList(string column): List<bool> throws DbError {
    return match (DbSys.getBoolList(this.raw, column)) { DbResult.Ok(v) => v, DbResult.Err(e) => DbError.fail(e)? };
  }
  function getIntListOrNull(string column): List<int>? throws DbError {
    return match (DbSys.getIntListOrNull(this.raw, column)) { DbResult.Ok(v) => v, DbResult.Err(e) => DbError.fail(e)? };
  }
  function getStringListOrNull(string column): List<string>? throws DbError {
    return match (DbSys.getStringListOrNull(this.raw, column)) { DbResult.Ok(v) => v, DbResult.Err(e) => DbError.fail(e)? };
  }
  function getFloatListOrNull(string column): List<float>? throws DbError {
    return match (DbSys.getFloatListOrNull(this.raw, column)) { DbResult.Ok(v) => v, DbResult.Err(e) => DbError.fail(e)? };
  }
  function getBoolListOrNull(string column): List<bool>? throws DbError {
    return match (DbSys.getBoolListOrNull(this.raw, column)) { DbResult.Ok(v) => v, DbResult.Err(e) => DbError.fail(e)? };
  }
  // Column introspection (DEC-208 slice B) — the desugared `queryScalar`/`queryMap`/nested-hydration
  // helpers use these. `columnNames` is selection-ordered; `isNull` tests a column for SQL NULL.
  function columnNames(): List<string> throws DbError {
    return match (DbSys.columnNames(this.raw)) { DbResult.Ok(v) => v, DbResult.Err(e) => DbError.fail(e)? };
  }
  function isNull(string column): bool throws DbError {
    return match (DbSys.isNull(this.raw, column)) { DbResult.Ok(v) => v, DbResult.Err(e) => DbError.fail(e)? };
  }
}

class Statement {
  constructor(private DbHandle raw) {}
  function bind(string | int | float | bool value): Statement throws DbError {
    return match (DbSys.bind(this.raw, value)) { DbResult.Ok(h) => new Statement(h), DbResult.Err(e) => DbError.fail(e)? };
  }
  function bindNamed(string name, string | int | float | bool value): Statement throws DbError {
    return match (DbSys.bindNamed(this.raw, name, value)) { DbResult.Ok(h) => new Statement(h), DbResult.Err(e) => DbError.fail(e)? };
  }
  // Typed IN-list bind (DEC-208 slice D, spec §2): occupies one positional `?` slot (left-to-right
  // with bind()) that expands to `(?,?,…)` — one placeholder per value — at execute time; an empty list
  // becomes `(NULL)` (a never-true IN). Strictly safer than PDO (which cannot bind an array to IN).
  // Generic over the element type (a `List<int>`/`List<string>`/… all bind); a non-scalar element is a
  // runtime DbError (an invariant `List<bindable>` union cannot accept a homogeneous list argument).
  function bindList<T>(List<T> values): Statement throws DbError {
    return match (DbSys.bindList(this.raw, values)) { DbResult.Ok(h) => new Statement(h), DbResult.Err(e) => DbError.fail(e)? };
  }
  function exec(): int throws DbError {
    return match (DbSys.exec(this.raw)) { DbResult.Ok(n) => n, DbResult.Err(e) => DbError.fail(e)? };
  }
  // Bulk write (DEC-208 slice D, spec §4): prepare ONCE, execute for each row of positional binds,
  // inside one savepoint (atomic + far faster than a loop). `rows` carries ALL binds (do not also call
  // bind()/bindNamed()). Returns the total affected rows. Generic over the row element type (same
  // reason as bindList); a non-scalar bind value is a runtime DbError.
  function executeMany<T>(List<List<T>> rows): int throws DbError {
    return match (DbSys.executeMany(this.raw, rows)) { DbResult.Ok(n) => n, DbResult.Err(e) => DbError.fail(e)? };
  }
  // Exec an INSERT and return the auto-generated rowid / PK (DEC-208 slice D, spec §4) — exec + the
  // connection's last insert id in one call. (Db.lastInsertId() reads the same value standalone.)
  function execReturningId(): int throws DbError {
    return match (DbSys.execReturningId(this.raw)) { DbResult.Ok(id) => id, DbResult.Err(e) => DbError.fail(e)? };
  }
  // Column naming strategy (DEC-208 slice B2, spec §3) — chainable, per query:
  // `stmt.namingStrategy(new Naming.SnakeToCamel()).queryInto()` maps a `userName` field from a
  // `user_name` column. This method is a compile-time marker realized as a runtime NO-OP (returns
  // `this` unchanged): the `desugar_db` pass reads the strategy from the call chain at COMPILE TIME and
  // bakes the transformed column-name literals straight into the generated `getX("user_name")` calls
  // (zero runtime cost). It exists so the chain type-checks; the argument must be a `new Naming.X()`
  // literal (a runtime value is rejected, `E-DB-NAMING-NOT-CONST` — the strategy cannot vary at run
  // time). Applies only to by-field-name hydration (`queryInto`/`queryOneInto`, and a `queryMap` entity
  // value); `queryScalar`/scalar map values read by column position and ignore it. NOTE: the strategy is
  // read from the query call's OWN chain, so keep it in one expression — break it into a stored
  // `Statement s = stmt.namingStrategy(...); s.queryInto();` and the query reverts to `Exact` (a missing
  // column then faults loudly at run time, never silently wrong).
  function namingStrategy(Naming strategy): Statement { return this; }
  function query(): List<Row> throws DbError {
    return match (DbSys.query(this.raw)) { DbResult.Ok(rows) => Statement.wrapRows(rows), DbResult.Err(e) => DbError.fail(e)? };
  }
  // Streaming (DEC-208 item H): run the query and deliver rows ONE AT A TIME via `RowStream.next()`
  // (`null` = exhausted) instead of materializing a `List<Row>` in user code. The typed form
  // `stmt.streamInto<T>()` (desugar_db) wraps this in a `DbStream<T>` that hydrates each row only
  // when pulled — early exit skips the remaining rows' hydration entirely.
  function stream(): RowStream throws DbError {
    return match (DbSys.stream(this.raw)) { DbResult.Ok(h) => new RowStream(h), DbResult.Err(e) => DbError.fail(e)? };
  }
  private static function wrapRows(List<DbHandle> rows): List<Row> {
    mutable List<Row> out = new List<Row>();
    mutable int i = 0;
    int n = List.length(rows);
    while (i < n) {
      out = List.append(out, new Row(rows[i]));
      i = i + 1;
    }
    return out;
  }
}

// A row-at-a-time result cursor (DEC-208 item H) — the untyped streaming surface. `next()` yields the
// next `Row`, or `null` when the result set is exhausted; iterate with
// `while (var r = s.next()?) { … }` (a null-condition binding ends the loop).
class RowStream {
  constructor(private DbHandle raw) {}
  function next(): Row? throws DbError {
    DbHandle? h = match (DbSys.streamNext(this.raw)) { DbResult.Ok(v) => v, DbResult.Err(e) => DbError.fail(e)? };
    if (var handle = h) { return new Row(handle); }
    return null;
  }
}

// The TYPED streaming surface (DEC-208 item H): a lazy, hydrate-on-pull stream of `T` built by the
// `stmt.streamInto<T>()` desugar (which supplies the per-class hydration closure). `next()` pulls one
// row and hydrates it into `T` (same strict by-name mapping as `queryInto`), or `null` at the end —
// rows never pulled are never hydrated.
class DbStream<T> {
  constructor(private RowStream rows, private (Row) => T throws DbError hydrate) {}
  function next(): T? throws DbError {
    Row? r = this.rows.next()?;
    if (var row = r) {
      (Row) => T throws DbError f = this.hydrate;
      T v = f(row)?;
      return v;
    }
    return null;
  }
}

class Db {
  // DEC-221: opening a connection can fail, so the constructor itself declares `throws DbError` and
  // opens directly — `new Db(dsn)` (fail-fast, exactly like PHP's `new PDO`). No static factory. The
  // handle is COMPUTED in the body (not a promoted param), so the field is `mutable` (set once here).
  private mutable DbHandle raw;
  constructor(string dsn) throws DbError {
    this.raw = match (DbSys.connect(dsn)) { DbResult.Ok(h) => h, DbResult.Err(e) => DbError.fail(e)? };
  }
  // Credential-safe connect (DEC-208 slice G, spec §1). The password is supplied as a `Core.Secret` —
  // kept out of plaintext in user code — and injected into the DSN only at the connect boundary. It is
  // NEVER retained: the driver parses it back out into its connection config and stores only a redacted
  // DSN, so a connect error prints the host but never the password (unlike PDO, which leaks the DSN in
  // exceptions). Use for a `postgres://user@host/db` DSN (no inline password); SQLite has no password,
  // so the DSN is passed through unchanged. Example:
  //   `Db db = Db.withPassword("postgres://app@db.host:5432/prod", new Secret(env));`
  static function withPassword(string dsn, Secret<string> password): Db throws DbError {
    return new Db(DbSys.dsnWithPassword(dsn, password.expose()))?;
  }
  function prepare(string sql): Statement throws DbError {
    return match (DbSys.prepare(this.raw, sql)) { DbResult.Ok(h) => new Statement(h), DbResult.Err(e) => DbError.fail(e)? };
  }

  // --- Writes & robustness (DEC-208 slice D, spec §4/§7). ---
  // The auto-generated rowid / PK of the most recent INSERT on this connection.
  function lastInsertId(): int throws DbError {
    return match (DbSys.lastInsertId(this.raw)) { DbResult.Ok(v) => v, DbResult.Err(e) => DbError.fail(e)? };
  }
  // Arm a query timeout (ms): a bounded lock-wait (SQLite busy_timeout). Once set, a busy/locked
  // failure surfaces as `Timeout` rather than `SerializationFailure`. Chainable (returns this).
  function timeout(int ms): Db throws DbError {
    match (DbSys.timeout(this.raw, ms)) { DbResult.Ok(_) => Db.ok(), DbResult.Err(e) => DbError.fail(e)? };
    return this;
  }
  // Register a per-query observability hook (logging / metrics / slow-query). The `(string sql, int
  // ms) => void` closure fires after each query/exec with the SQL text + elapsed ms. A logging hook is
  // `void` (cannot throw a checked error), so registration never fails. Chainable (returns this).
  // NOTE: `ms` is wall-clock (non-deterministic) — do not print it raw in a byte-identity example.
  function onQuery((string, int) => void hook): Db {
    match (DbSys.onQuery(this.raw, hook)) { DbResult.Ok(_) => Db.ok(), DbResult.Err(_) => Db.ok() };
    return this;
  }

  // A void no-op, used as the success arm of the `void`-returning transaction methods below — a `match`
  // arm must be an EXPRESSION, so `DbResult.Ok(_) => Db.ok()` yields `void` cleanly (a bare `{}` block
  // is not an expression here). The `?` in the error arm makes that arm `never`, unifying to `void`.
  private static function ok(): void {}

  // --- Transactions & correctness (DEC-208 slice C, spec §5). Manual, PDO-faithful control. A nested
  // begin() opens a SAVEPOINT (composable partial rollback); commit()/rollback() release / roll back the
  // innermost level. commit()/rollback() at depth 0 are best-effort no-ops (the native guards the depth),
  // so a secondary fault can never mask the original. The closure form `db.transaction(fn)` + retry are
  // BLOCKED on phorj lambdas being unable to propagate a checked exception (see docs/KNOWN_ISSUES) —
  // recorded as a PENDING adjudication; this manual surface is what the closure form would build on. ---
  function begin(): void throws DbError {
    match (DbSys.begin(this.raw)) { DbResult.Ok(_) => Db.ok(), DbResult.Err(e) => DbError.fail(e)? };
  }
  function commit(): void throws DbError {
    match (DbSys.commit(this.raw)) { DbResult.Ok(_) => Db.ok(), DbResult.Err(e) => DbError.fail(e)? };
  }
  function rollback(): void throws DbError {
    match (DbSys.rollback(this.raw)) { DbResult.Ok(_) => Db.ok(), DbResult.Err(e) => DbError.fail(e)? };
  }
  // Best-effort rollback that NEVER throws — safe inside a `finally` (a throwing rollback there would
  // mask the original exception). The auto-rollback idiom is: `db.begin(); mutable bool ok = false;
  // try { …work…; db.commit(); ok = true; } finally { if (!ok) db.rollbackQuiet(); }` — demonstrated in
  // examples/db/transactions.phg.
  function rollbackQuiet(): void {
    match (DbSys.rollback(this.raw)) { DbResult.Ok(_) => Db.ok(), DbResult.Err(_) => Db.ok() };
  }
  // Closure-form transaction (DEC-208 slice C, spec §5 — unblocked by DEC-222 throwing closures). BEGIN,
  // run the closure, COMMIT on a normal return (returning its value), auto-ROLLBACK + re-throw the
  // ORIGINAL typed error on a throw (`DbSys.transaction` preserves the thrown value through the rollback
  // via the backend's `pending_throw`, so the caller catches the exact `DbError` the closure threw — not
  // a generic one). A NESTED `db.transaction(fn)` opens a SAVEPOINT (composable partial rollback), so
  // transactions compose. The closure is a THROWING function type — `db.transaction(function(): T throws
  // DbError { … })` — since DB work raises a checked `DbError` (a non-throwing closure is also accepted:
  // fewer-throws variance). BOTH this closure form AND the manual begin()/commit()/rollback() above are
  // supported (developer ruled BOTH).
  // DEC-249 resolved the recorded SURFACE PENDING the ambitious way: method default parameters
  // landed, so the spec's single-method shape is real — `db.transaction(fn)` runs once;
  // `db.transaction(fn, retries)` re-runs the WHOLE transaction up to `retries` extra times on the
  // transient `SerializationFailure` ONLY (SQLite SQLITE_BUSY/LOCKED — the class Serializable
  // isolation needs); any OTHER `DbError` (and an exhausted retry budget) rolls back and
  // propagates immediately. The retry loop lives HERE, not in the native, because only phorj
  // source can `catch` the TYPED error (the thrown value is backend-side `pending_throw`,
  // invisible to a native). The former distinct `transactionRetry(fn, retries)` is RETIRED.
  // NOTE (timeout): with `db.timeout(ms)` armed a transient busy is reclassified `Timeout`, not
  // `SerializationFailure` (slice D) — so it is NOT retried; leave the timeout unset when relying on retry.
  function transaction<T>(() => T throws DbError fn, int retries = 0): T throws DbError {
    mutable int attempt = 0;
    while (true) {
      try {
        return match (DbSys.transaction(this.raw, fn)) { DbResult.Ok(v) => v, DbResult.Err(e) => DbError.fail(e)? };
      } catch (SerializationFailure e) {
        if (attempt >= retries) { throw e; }
        attempt = attempt + 1;
      }
    }
  }
  // Deterministic close (spec §1): idempotent, never throws. After close(), any further use of this
  // connection (or a Statement derived from it) fails with `ConnectionError`. The `using`/`Closable`
  // sugar that would call this automatically at scope exit is DEC-203 — a separate language slice
  // (see KNOWN_ISSUES); until then, call close() explicitly (or rely on drop at program end).
  function close(): void {
    match (DbSys.close(this.raw)) { DbResult.Ok(_) => Db.ok(), DbResult.Err(_) => Db.ok() };
  }
}
"#;

/// The Core-module registry, in the SAME order as the pre-UA-L2 injection chain — ORDER IS
/// LOAD-BEARING: `HTTP_PRELUDE` transitively `import Core.Regex`, and Http runs BEFORE Regex, so
/// that transitive import is what triggers `Regex`-class injection for `Router.constraintOk`. A
/// reorder that broke this would still pass most tests; `examples/web/route-constraints.phg` (a
/// regex-constrained route with no explicit `import Core.Regex`) is the regression guard.
pub(super) const CORE_MODULES: &[VirtualModule] = &[
    VirtualModule {
        module: &["Core", "Json"],
        qualifier: "Json",
        src: Some(JSON_PRELUDE),
        respond_bridge: None,
        member_gated: false,
        bare_types: &[],
    },
    VirtualModule {
        module: &["Core", "Decimal"],
        qualifier: "Decimal",
        src: Some(ROUNDING_MODE_PRELUDE),
        respond_bridge: None,
        member_gated: true,
        bare_types: &["RoundingMode"],
    },
    VirtualModule {
        module: &["Core", "Option"],
        qualifier: "Option",
        src: Some(OPTION_PRELUDE),
        respond_bridge: None,
        member_gated: true,
        bare_types: &[],
    },
    VirtualModule {
        module: &["Core", "Result"],
        qualifier: "Result",
        src: Some(RESULT_PRELUDE),
        respond_bridge: None,
        member_gated: true,
        bare_types: &[],
    },
    // `Core.Debug` (DEC-238) — the dumper prelude (std-only, always compiled).
    VirtualModule {
        module: &["Core", "Debug"],
        qualifier: "Debug",
        src: Some(DEBUG_PRELUDE),
        respond_bridge: None,
        member_gated: true,
        bare_types: &["Debug", "Dumped"],
    },
    // `Core.DebugSys` — the INTERNAL renderer native.
    VirtualModule {
        module: &["Core", "DebugSys"],
        qualifier: "DebugSys",
        src: None,
        respond_bridge: None,
        member_gated: false,
        bare_types: &[],
    },
    // `Core.Session` (W3, TOP-20 #3) — HTTP sessions over the Core.Http value types. MUST precede
    // `Core.Http` (its `import Core.Http` transitively injects it — the forward-fold rule).
    VirtualModule {
        module: &["Core", "Session"],
        qualifier: "Session",
        src: Some(SESSION_PRELUDE),
        respond_bridge: None,
        member_gated: true,
        bare_types: &["Session"],
    },
    // `Core.SessionSys` — the INTERNAL session-store natives (std-only, always compiled).
    VirtualModule {
        module: &["Core", "SessionSys"],
        qualifier: "SessionSys",
        src: None,
        respond_bridge: None,
        member_gated: false,
        bare_types: &[],
    },
    VirtualModule {
        module: &["Core", "Http"],
        qualifier: "Http",
        src: Some(HTTP_PRELUDE),
        respond_bridge: Some(HTTP_RESPOND_BRIDGE),
        member_gated: true,
        bare_types: &["Request", "Response", "Route", "Router"],
    },
    VirtualModule {
        module: &["Core", "Regex"],
        qualifier: "Regex",
        src: Some(REGEX_PRELUDE),
        respond_bridge: None,
        member_gated: false,
        bare_types: &[],
    },
    VirtualModule {
        module: &["Core", "Time"],
        qualifier: "Time",
        src: Some(TIME_PRELUDE),
        respond_bridge: None,
        member_gated: true,
        bare_types: &["Duration", "Date", "Instant"],
    },
    // `Core.Uri` (DEC-240) — the RFC 3986 `Uri` class + `UriError` taxonomy over `Core.UriSys`.
    VirtualModule {
        module: &["Core", "Uri"],
        qualifier: "Uri",
        src: Some(URI_PRELUDE),
        respond_bridge: None,
        member_gated: true,
        bare_types: &[
            "Uri",
            "UriError",
            "UriMalformed",
            "UriBadScheme",
            "UriBadUserInfo",
            "UriBadHost",
            "UriBadPort",
            "UriPortOutOfRange",
            "UriBadPath",
            "UriBadQuery",
            "UriBadFragment",
            "UriBaseNotAbsolute",
        ],
    },
    // `Core.Db` (DEC-208) — the enhanced-PDO surface classes. MUST precede `Core.DbSys` (its natives)
    // so its `import Core.DbSys` triggers the natives being in scope (the Http→Regex ordering rule).
    VirtualModule {
        module: &["Core", "Db"],
        qualifier: "Db",
        src: Some(DB_PRELUDE),
        respond_bridge: None,
        member_gated: true,
        bare_types: &[
            "Db",
            "Statement",
            "Row",
            "DbError",
            "DbHandle",
            // DEC-208 item H — the streaming surfaces (untyped row cursor + typed lazy stream).
            "RowStream",
            "DbStream",
            // DEC-208 slice B2 — the column naming strategy enum, member-gated so
            // `new Naming.SnakeToCamel()` resolves after `import Core.Db.Naming;` (nothing in the wind).
            "Naming",
            // DEC-208 slice C typed taxonomy — member-gated so `catch (UniqueViolation e)` resolves
            // in user code after `import Core.Db.UniqueViolation;` (nothing in the wind).
            "UniqueViolation",
            "ConstraintViolation",
            "ConnectionError",
            "SerializationFailure",
            "Timeout",
            "SyntaxError",
        ],
    },
    // `Core.Mail` (DEC-223) — the native-mailer prelude (twin of `Core.Db`). MUST precede `Core.Secret`
    // (its `import Core.Secret` transitively injects it — the same forward-fold rule as Db→Secret) and
    // `Core.MailSys` (its natives).
    VirtualModule {
        module: &["Core", "Mail"],
        qualifier: "Mail",
        src: Some(MAIL_PRELUDE),
        respond_bridge: None,
        member_gated: true,
        bare_types: &[
            "Mailer",
            "Email",
            "Address",
            "Attachment",
            "MailError",
            "MailHandle",
            "SmtpConfig",
            "SendmailTransport",
            "FileTransport",
            "NullTransport",
            // The typed taxonomy — member-gated so `catch (AuthFailed e)` resolves after
            // `import Core.Mail.AuthFailed;` (nothing in the wind).
            "ConnectionFailed",
            "AuthFailed",
            "RecipientRejected",
            "TlsError",
            "InvalidAddress",
            "MessageBuildFailed",
            "MailTimeout",
            "MailIo",
        ],
    },
    // `Core.Fs` (W3) — the typed filesystem prelude (std-only, always compiled). Taxonomy names are
    // Fs-PREFIXED (a bare `NotFound` bare_type captured a user-space class — caught live).
    VirtualModule {
        module: &["Core", "Fs"],
        qualifier: "Fs",
        src: Some(FS_PRELUDE),
        respond_bridge: None,
        member_gated: true,
        bare_types: &[
            "Fs",
            "FsError",
            "FsNotFound",
            "FsPermissionDenied",
            "FsAlreadyExists",
            "FsNotADirectory",
            "FsIsADirectory",
            "FsDirNotEmpty",
            "FsIo",
        ],
    },
    // `Core.FsSys` — the INTERNAL filesystem natives.
    VirtualModule {
        module: &["Core", "FsSys"],
        qualifier: "FsSys",
        src: None,
        respond_bridge: None,
        member_gated: false,
        bare_types: &[],
    },
    // `Core.HttpClient` (W3-2) — the sync HTTP client prelude (native-only, `http-client` feature).
    VirtualModule {
        module: &["Core", "HttpClient"],
        qualifier: "HttpClient",
        src: Some(HTTP_CLIENT_PRELUDE),
        respond_bridge: None,
        member_gated: true,
        bare_types: &[
            "HttpClient",
            "HttpResponse",
            "HttpClientError",
            "HcHandle",
            "InvalidUrl",
            "HttpConnectionFailed",
            "HttpTimeout",
            "HttpTlsError",
            "ProtocolError",
            "TooManyRedirects",
            "TooLarge",
        ],
    },
    // `Core.Secret` (Fork B) — the opaque `Secret<T>` credential wrapper. Placed AFTER `Core.Db` because
    // `Core.Db`'s `import Core.Secret` (for the `Db.withPassword(dsn, Secret<string>)` factory, DEC-208
    // slice G) transitively injects it, and transitive injection only reaches modules that appear LATER
    // in this list (the same forward-fold rule as Http→Regex — an EARLIER module is never pulled by a
    // later importer). A direct `import Core.Secret;` in user code works from any position (user imports
    // seed the injected set), so this move does not affect standalone Secret programs. Nothing else
    // imports Core.Secret, so nothing needs it injected before this point.
    VirtualModule {
        module: &["Core", "Secret"],
        qualifier: "Secret",
        src: Some(SECRET_PRELUDE),
        respond_bridge: None,
        member_gated: false,
        bare_types: &[],
    },
    // `Core.DbSys` — the INTERNAL DB natives (open/prepare/bind/query/exec/get*) the `Core.Db` prelude
    // wraps. Native-only (no prelude); a distinct qualifier so a prelude `class Db` never collides with
    // the native leaf. Feature-gated (`db`): the natives only exist under `--features db`.
    VirtualModule {
        module: &["Core", "DbSys"],
        qualifier: "DbSys",
        src: None,
        respond_bridge: None,
        member_gated: false,
        bare_types: &[],
    },
    // `Core.HttpClientSys` — the INTERNAL HTTP-client natives (`http-client` feature).
    VirtualModule {
        module: &["Core", "HttpClientSys"],
        qualifier: "HttpClientSys",
        src: None,
        respond_bridge: None,
        member_gated: false,
        bare_types: &[],
    },
    // `Core.MailSys` — the INTERNAL mailer natives the `Core.Mail` prelude wraps (the `Core.DbSys`
    // twin, DEC-223). Feature-gated (`mail`): the natives only exist under `--features mail`.
    VirtualModule {
        module: &["Core", "MailSys"],
        qualifier: "MailSys",
        src: None,
        respond_bridge: None,
        member_gated: false,
        bare_types: &[],
    },
    // Attribute-only modules — no prelude to inject; they exist only to gate their `#[…]` types.
    VirtualModule {
        module: &["Core", "Runtime", "Integer"],
        qualifier: "Runtime.Integer",
        src: None,
        respond_bridge: None,
        member_gated: false,
        bare_types: &["UncheckedOverflow"],
    },
    VirtualModule {
        module: &["Core", "Runtime"],
        qualifier: "Runtime",
        src: None,
        respond_bridge: None,
        member_gated: false,
        bare_types: &["Attribute"],
    },
    VirtualModule {
        module: &["Core", "DI"],
        qualifier: "DI",
        src: None,
        respond_bridge: None,
        member_gated: false,
        bare_types: &["Injectable", "Provides", "Transient"],
    },
    // `Core.Log` (DEC-220): structured leveled logging to STDERR — native-only (no prelude), qualified
    // calls `Log.debug/info/warn/error(msg)` resolve to the `Core.Log` natives. Impure (stderr side
    // effect) ⇒ an importing program is quarantined from the byte-identity differential.
    VirtualModule {
        module: &["Core", "Log"],
        qualifier: "Log",
        src: None,
        respond_bridge: None,
        member_gated: false,
        bare_types: &[],
    },
];

/// Feature-gated Core modules: `(module path, compiled-in?, cargo feature name)`. When such a module
/// is imported on a build WITHOUT its feature, [`unavailable_core_module`] produces ONE clean
/// `E-MODULE-UNAVAILABLE` diagnostic — replacing the otherwise-inevitable wall of prelude-internal
/// `E-UNKNOWN-IDENT`s (the prelude classes reference natives that do not exist in that build).
/// New gated module (e.g. `Core.Mail`) = one row here.
const GATED_CORE_MODULES: &[(&[&str], bool, &str)] = &[
    (&["Core", "Db"], cfg!(feature = "db"), "db"),
    (&["Core", "DbSys"], cfg!(feature = "db"), "db"),
    (&["Core", "Mail"], cfg!(feature = "mail"), "mail"),
    (&["Core", "MailSys"], cfg!(feature = "mail"), "mail"),
    (
        &["Core", "HttpClient"],
        cfg!(feature = "http-client"),
        "http-client",
    ),
    (
        &["Core", "HttpClientSys"],
        cfg!(feature = "http-client"),
        "http-client",
    ),
];

/// The dotted names of feature-gated Core modules NOT compiled into THIS build. Test harnesses
/// (the differential/example sweeps) use it to skip gated examples loudly on reduced builds instead
/// of failing on `E-MODULE-UNAVAILABLE` (e.g. `examples/mail/` on a build without `--features mail`).
pub fn unavailable_gated_modules() -> Vec<String> {
    GATED_CORE_MODULES
        .iter()
        .filter(|(_, available, _)| !available)
        .map(|(m, _, _)| m.join("."))
        .collect()
}

/// If the program imports a feature-gated Core module whose feature is compiled out, the diagnostic
/// to abort with (checked on the RAW program, before any prelude injection).
pub(super) fn unavailable_core_module(prog: &Program) -> Option<crate::diagnostic::Diagnostic> {
    use crate::ast::Item;
    for it in &prog.items {
        let Item::Import { path, span, .. } = it else {
            continue;
        };
        for (module, available, feature) in GATED_CORE_MODULES {
            if *available || path.len() < module.len() {
                continue;
            }
            if path.iter().zip(module.iter()).all(|(a, b)| a == b) {
                let m = module.join(".");
                return Some(
                    crate::diagnostic::Diagnostic::new(
                        crate::diagnostic::Stage::Type,
                        format!("`{m}` is not available in this `phg` build"),
                        span.line,
                        span.col,
                    )
                    .with_code("E-MODULE-UNAVAILABLE")
                    .with_hint(format!(
                        "rebuild this `phg` with the `{feature}` cargo feature — `cargo build --features {feature}` (default-set features are absent only under `--no-default-features`)"
                    )),
                );
            }
        }
    }
    None
}

/// The injected member type → owning module qualifier (UA-L2: the registry-derived replacement for
/// the hand-synced `module_of` match). Reused by the injected-type discipline
/// (`checker::enforce_injected`) and the qualified-construction dispatch (`checker::calls`/`expr`).
pub(crate) fn core_module_of(name: &str) -> Option<&'static str> {
    CORE_MODULES
        .iter()
        .find(|m| m.bare_types.contains(&name))
        .map(|m| m.qualifier)
}

/// Inject every applicable `Core.*` prelude at the program head, in registry order. Replaces the
/// eight chained `inject_*_prelude` fns with one uniform fold (UA-L2). For each module whose import
/// is present, each prelude item is prepended only if absent (imports by path; classes/enums/fns by
/// name), injected enums are marked `injected` (qualified-variant discipline), and Http's `respond`
/// bridge is appended when the program defines `handle` and no `respond`. A no-op (borrowed) for a
/// program that imports no injected Core module — such programs stay byte-for-byte unchanged.
pub(super) fn inject_core_modules(prog: &Program) -> std::borrow::Cow<'_, Program> {
    use crate::ast::Item;
    let mut cur: std::borrow::Cow<'_, Program> = std::borrow::Cow::Borrowed(prog);
    for m in CORE_MODULES {
        let Some(src) = m.src else { continue };
        let p = cur.as_ref();
        let gated_in = if m.member_gated {
            imports_module_or_member(p, m.module)
        } else {
            p.items.iter().any(|it| {
                matches!(it, Item::Import { path, .. }
                    if path.len() == m.module.len()
                        && path.iter().zip(m.module).all(|(a, b)| a == b))
            })
        };
        if !gated_in {
            continue;
        }
        let Ok(parsed) = lex_parse(src) else {
            continue; // unreachable: registry preludes are valid
        };
        let mut prepend: Vec<Item> = Vec::new();
        for it in parsed.items {
            let absent = match &it {
                Item::Import { path, .. } => !p.items.iter().any(|x| {
                    matches!(x, Item::Import { path: xp, .. } if xp.join(".") == path.join("."))
                }),
                Item::Enum(e) => !p
                    .items
                    .iter()
                    .any(|x| matches!(x, Item::Enum(y) if y.name == e.name)),
                Item::Class(c) => !p
                    .items
                    .iter()
                    .any(|x| matches!(x, Item::Class(y) if y.name == c.name)),
                Item::Function(f) => !p
                    .items
                    .iter()
                    .any(|x| matches!(x, Item::Function(y) if y.name == f.name)),
                _ => false,
            };
            if absent {
                let mut it = it;
                if let Item::Enum(e) = &mut it {
                    e.injected = true;
                }
                prepend.push(it);
            }
        }
        // Http serve bridge: synthesize `respond` wrapping a user `handle`, when no `respond` exists.
        if let Some(bridge_src) = m.respond_bridge {
            let has_fn = |n: &str| {
                p.items
                    .iter()
                    .any(|x| matches!(x, Item::Function(f) if f.name == n))
            };
            if has_fn("handle") && !has_fn("respond") {
                if let Ok(bridge) = lex_parse(bridge_src) {
                    prepend.extend(
                        bridge
                            .items
                            .into_iter()
                            .filter(|it| matches!(it, Item::Function(f) if f.name == "respond")),
                    );
                }
            }
        }
        if prepend.is_empty() {
            continue;
        }
        let mut items = Vec::with_capacity(p.items.len() + prepend.len());
        items.extend(prepend);
        items.extend(p.items.iter().cloned());
        cur = std::borrow::Cow::Owned(Program {
            package: p.package.clone(),
            items,
            span: p.span,
        });
    }
    cur
}
