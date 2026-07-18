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

// DEC-273 wave 2: the debug prelude source moved to `crate::ext::debug_prelude` (colocation).

/// `Core.Input` (DEC-281) — the stdin module, `Core.Output`'s twin: piped/redirected data
/// (`cat file | phg run s.phg`, `phg run s.phg < file`) becomes readable. Impure (quarantined
/// from the byte-identity differential like `Core.Process`; validated by `tests/stdin.rs` on both
/// backends under the override seam) but FULLY transpilable — the PHP legs read `php://stdin`
/// via the CLI `STDIN` constant. Under `phg serve`, stdin is disabled before workers run (web
/// input is the `Request`): reads behave as an already-exhausted pipe. `readLine` strips the
/// trailing newline and returns `null` at EOF; `lines()` is a DEC-257 `Iterator<string>` —
/// foreach-able, hydrating one line per pull.
pub(super) const INPUT_PRELUDE: &str = r#"
import Core.Native.Input as NativeInput;
import Core.IteratorModule;

class Input {
  static function readAll(): string { return NativeInput.readAll(); }
  static function readAllBytes(): bytes { return NativeInput.readAllBytes(); }
  static function readLine(): string? { return NativeInput.readLine(); }
  static function isInteractive(): bool { return NativeInput.isInteractive(); }
  static function lines(): InputLines { return new InputLines(); }
}

// The pull-iterator over stdin lines (DEC-257 protocol): `hasNext()` reads one line ahead and
// caches it; `next()` hands it over, or FAULTS "iterator exhausted" past the end (the misuse
// contract — foreach never triggers it).
class InputLines implements Iterator<string> {
  private mutable string? ahead;
  constructor() {}
  function hasNext(): bool {
    if (var cached = this.ahead) { return true; }
    string? l = NativeInput.readLine();
    if (var line = l) {
      this.ahead = line;
      return true;
    }
    return false;
  }
  function next(): string {
    bool has = this.hasNext();
    if (has) {
      if (var l = this.ahead) {
        this.ahead = null;
        return l;
      }
    }
    panic("iterator exhausted");
  }
}
"#;

// DEC-273 wave 3: the session prelude source moved to `crate::ext::session_prelude` (colocation).

/// The canonical `Core.Http` types, injected (below) when a program imports `Core.Http` (M6 W1 →
/// stdlib). The portable handler model — `handle(Request): Response` — at the value level: `Request`
/// and `Response` are immutable values; `Request.parse(bytes) -> Request?` and `resp.serialize()`
/// round-trip the HTTP/1.1 wire form. The bodies reuse `Core.Bytes`/`Core.String` (so the prelude also
/// imports them), so this is the same proven logic as `examples/web/handler.phg`, promoted to the
/// stdlib behind the static-method API (slice B0). Flows through every backend as ordinary classes.
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
// DEC-242 — the cookie VALUE type (developer-ruled: class ONLY, no flat twin). Immutable
// builders (the Response precedent; named args don't exist, so the approved `partitioned = true`
// spelling maps to `.partitioned(true)` — disclosed in the register). Secure defaults ON,
// HttpOnly ON, SameSite Lax, Path "/" — the safe-by-default posture; `Partitioned` (CHIPS) is
// the ruled opt-in.
enum SameSite { Lax, Strict, NoValue }
class Cookie {
  constructor(
    public string name,
    public string value,
    public string cookiePath = "/",
    public bool isSecure = true,
    public bool isHttpOnly = true,
    public bool isPartitioned = false
  ) {}
  function path(string p): Cookie {
    return new Cookie(this.name, this.value, p, this.isSecure, this.isHttpOnly, this.isPartitioned);
  }
  function secure(bool b): Cookie {
    return new Cookie(this.name, this.value, this.cookiePath, b, this.isHttpOnly, this.isPartitioned);
  }
  function httpOnly(bool b): Cookie {
    return new Cookie(this.name, this.value, this.cookiePath, this.isSecure, b, this.isPartitioned);
  }
  function partitioned(bool b): Cookie {
    return new Cookie(this.name, this.value, this.cookiePath, this.isSecure, this.isHttpOnly, b);
  }
  // The Set-Cookie header VALUE, canonical attribute order: Path; Secure; HttpOnly; SameSite;
  // Partitioned. (SameSite is fixed Lax this slice — a `sameSite(SameSite)` builder joins when
  // a real Strict/None consumer lands; YAGNI over speculative surface.)
  function render(): string {
    string base = "{this.name}={this.value}; Path={this.cookiePath}";
    string s1 = if (this.isSecure) { "{base}; Secure" } else { base };
    string s2 = if (this.isHttpOnly) { "{s1}; HttpOnly" } else { s1 };
    string s3 = "{s2}; SameSite=Lax";
    return if (this.isPartitioned) { "{s3}; Partitioned" } else { s3 };
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
  function withCookie(Cookie c): Response {
    string line = c.render();
    return new Response(this.status, this.body, List.concat(this.headerLines, ["Set-Cookie: {line}"]));
  }
  function withCookies(List<Cookie> cs): Response {
    mutable Response r = this;
    for (Cookie c in cs) {
      r = r.withCookie(c);
    }
    return r;
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

// The opaque compiled-`Regex` value model, injected when a program imports `Core.Regex` (Fork A,
// `docs/specs/2026-06-28-core-regex-design.md`). A `Regex` value is built only by `Regex.compile`
// (which validates via the `regex` crate); the `pattern` field is the **bare** pattern.
// The Regex prelude doc + source moved to `crate::ext::regex_prelude` (DEC-273 colocation): the
// class carries `pattern` so the transpiled `__phorj_regex_*` helpers can read `$re->pattern`;
// injected via the `Core.Regex` registry row, a no-op unless `Core.Regex` is imported.

// DEC-273 wave 3: the mail prelude source moved to `crate::ext::mail_prelude` (colocation).

/// `Core.FileSystemModule` (W3, TOP-20 #5 blocker) — the TYPED filesystem prelude: every failure is a catchable
/// `FileSystemError` subtype (contrast the older `Core.File`, whose write/delete failures are uncatchable
/// hard faults — its deprecation is a queued adjudication; this module is purely additive).
/// Listings are SORTED (determinism). Std-only, always compiled (no feature gate). The taxonomy is
/// FileSystem-PREFIXED throughout (`FileSystemNotFoundError`, not `NotFound` — a bare generic name would CAPTURE
/// user-space classes via the injected-type discipline; caught live when `examples/web/server.phg`'s
/// own `NotFound` class collided).
pub(super) const FS_PRELUDE: &str = r#"
import Core.Native.FileSystem as NativeFileSystem;
import Core.String;
import Core.List;

// Prelude-local result carrier (NOT Core.Result — the Core.DatabaseModule injection-order rationale).
enum FileSystemResult<T> { Ok(T value), Err(string message) }

open class FileSystemError implements Error {
  constructor(public string message) {}
  static function fail(string message): never throws FileSystemError {
    if (String.startsWith(message, "<<NotFound>>")) { throw new FileSystemNotFoundError(String.removePrefix(message, "<<NotFound>>")); }
    if (String.startsWith(message, "<<PermissionDenied>>")) { throw new FileSystemPermissionDeniedError(String.removePrefix(message, "<<PermissionDenied>>")); }
    if (String.startsWith(message, "<<AlreadyExists>>")) { throw new FileSystemAlreadyExistsError(String.removePrefix(message, "<<AlreadyExists>>")); }
    if (String.startsWith(message, "<<NotADirectory>>")) { throw new FileSystemNotADirectoryError(String.removePrefix(message, "<<NotADirectory>>")); }
    if (String.startsWith(message, "<<IsADirectory>>")) { throw new FileSystemIsADirectoryError(String.removePrefix(message, "<<IsADirectory>>")); }
    if (String.startsWith(message, "<<DirNotEmpty>>")) { throw new FileSystemDirNotEmptyError(String.removePrefix(message, "<<DirNotEmpty>>")); }
    if (String.startsWith(message, "<<FileSystemIoError>>")) { throw new FileSystemIoError(String.removePrefix(message, "<<FileSystemIoError>>")); }
    throw new FileSystemError(message);
  }
}

class FileSystemNotFoundError extends FileSystemError { constructor(string message) { parent.constructor(message); } }
class FileSystemPermissionDeniedError extends FileSystemError { constructor(string message) { parent.constructor(message); } }
class FileSystemAlreadyExistsError extends FileSystemError { constructor(string message) { parent.constructor(message); } }
class FileSystemNotADirectoryError extends FileSystemError { constructor(string message) { parent.constructor(message); } }
class FileSystemIsADirectoryError extends FileSystemError { constructor(string message) { parent.constructor(message); } }
class FileSystemDirNotEmptyError extends FileSystemError { constructor(string message) { parent.constructor(message); } }
class FileSystemIoError extends FileSystemError { constructor(string message) { parent.constructor(message); } }

// The typed filesystem surface (static module functions — filesystem state is ambient, so an
// instance would carry nothing; the SORTED listings + typed errors are the value).
class FileSystem {
  static function readText(string path): string throws FileSystemError {
    return match (NativeFileSystem.readText(path)) { FileSystemResult.Ok(v) => v, FileSystemResult.Err(e) => FileSystemError.fail(e)? };
  }
  static function readBytes(string path): bytes throws FileSystemError {
    return match (NativeFileSystem.readBytes(path)) { FileSystemResult.Ok(v) => v, FileSystemResult.Err(e) => FileSystemError.fail(e)? };
  }
  static function writeText(string path, string contents): void throws FileSystemError {
    match (NativeFileSystem.writeText(path, contents)) { FileSystemResult.Ok(_) => FileSystem.ok(), FileSystemResult.Err(e) => FileSystemError.fail(e)? };
  }
  static function writeBytes(string path, bytes contents): void throws FileSystemError {
    match (NativeFileSystem.writeBytes(path, contents)) { FileSystemResult.Ok(_) => FileSystem.ok(), FileSystemResult.Err(e) => FileSystemError.fail(e)? };
  }
  static function appendText(string path, string contents): void throws FileSystemError {
    match (NativeFileSystem.appendText(path, contents)) { FileSystemResult.Ok(_) => FileSystem.ok(), FileSystemResult.Err(e) => FileSystemError.fail(e)? };
  }
  static function copy(string from, string to): void throws FileSystemError {
    match (NativeFileSystem.copy(from, to)) { FileSystemResult.Ok(_) => FileSystem.ok(), FileSystemResult.Err(e) => FileSystemError.fail(e)? };
  }
  static function move(string from, string to): void throws FileSystemError {
    match (NativeFileSystem.move(from, to)) { FileSystemResult.Ok(_) => FileSystem.ok(), FileSystemResult.Err(e) => FileSystemError.fail(e)? };
  }
  static function delete(string path): void throws FileSystemError {
    match (NativeFileSystem.delete(path)) { FileSystemResult.Ok(_) => FileSystem.ok(), FileSystemResult.Err(e) => FileSystemError.fail(e)? };
  }
  static function size(string path): int throws FileSystemError {
    return match (NativeFileSystem.size(path)) { FileSystemResult.Ok(v) => v, FileSystemResult.Err(e) => FileSystemError.fail(e)? };
  }
  static function exists(string path): bool throws FileSystemError {
    return match (NativeFileSystem.exists(path)) { FileSystemResult.Ok(v) => v, FileSystemResult.Err(e) => FileSystemError.fail(e)? };
  }
  static function isFile(string path): bool throws FileSystemError {
    return match (NativeFileSystem.isFile(path)) { FileSystemResult.Ok(v) => v, FileSystemResult.Err(e) => FileSystemError.fail(e)? };
  }
  static function isDir(string path): bool throws FileSystemError {
    return match (NativeFileSystem.isDir(path)) { FileSystemResult.Ok(v) => v, FileSystemResult.Err(e) => FileSystemError.fail(e)? };
  }
  // Recursive create (mkdir -p semantics); removeDir removes ONE EMPTY dir; removeDirAll is the
  // loud recursive delete (refuses "/", "." and "..").
  static function createDir(string path): void throws FileSystemError {
    match (NativeFileSystem.createDir(path)) { FileSystemResult.Ok(_) => FileSystem.ok(), FileSystemResult.Err(e) => FileSystemError.fail(e)? };
  }
  static function removeDir(string path): void throws FileSystemError {
    match (NativeFileSystem.removeDir(path)) { FileSystemResult.Ok(_) => FileSystem.ok(), FileSystemResult.Err(e) => FileSystemError.fail(e)? };
  }
  static function removeDirAll(string path): void throws FileSystemError {
    match (NativeFileSystem.removeDirAll(path)) { FileSystemResult.Ok(_) => FileSystem.ok(), FileSystemResult.Err(e) => FileSystemError.fail(e)? };
  }
  // Entry NAMES of one directory, sorted; walk = every FILE under a root as sorted relative paths.
  static function listDir(string path): List<string> throws FileSystemError {
    return match (NativeFileSystem.listDir(path)) { FileSystemResult.Ok(v) => v, FileSystemResult.Err(e) => FileSystemError.fail(e)? };
  }
  static function walk(string root): List<string> throws FileSystemError {
    return match (NativeFileSystem.walk(root)) { FileSystemResult.Ok(v) => v, FileSystemResult.Err(e) => FileSystemError.fail(e)? };
  }
  static function tempDir(): string throws FileSystemError {
    return match (NativeFileSystem.tempDir()) { FileSystemResult.Ok(v) => v, FileSystemResult.Err(e) => FileSystemError.fail(e)? };
  }
  private static function ok(): void {}
}
"#;

// DEC-273 wave 3: the http_client prelude source moved to `crate::ext::http_client_prelude` (colocation).

/// and read only through `expose()` — the `value` field is private, and a `Secret` instance is not a
/// `string`, so printing/interpolating it is a clean type error (the primary, loud guarantee; no
/// runtime `***`). Reuses the generic-class machinery (`Box<T>`) wholesale — no new `Op`/`Value`/`Ty`.
/// Injected by [`inject_core_modules`] via the `Core.Secret` registry row — a no-op unless
/// `Core.Secret` is imported and no `Secret` class is already declared. The transpiler adds `final`
/// + `#[\SensitiveParameter]` for this class by name.
pub(super) const SECRET_PRELUDE: &str =
    "class Secret<T> { constructor(private T value) {} function expose(): T { return this.value; } }";

/// `Core.UriModule` (DEC-240) — one immutable RFC 3986 `Uri` class with the typed `UriError` taxonomy.
/// The instance state is a single validated RAW string; every accessor/wither/operation calls a
/// `Core.Native.Uri` native over it (`src/ext/uri/`), whose Rust kernel is pinned byte-for-byte to
/// the transpile twin — PHP 8.5's always-on `Uri\Rfc3986\Uri` (probe record:
/// `docs/research/2026-07-16-uri-twin-probes.md`) — so byte-identity holds with NO ladder
/// quarantine. Fallible natives return the new raw form or a `<<E>>`-sentinel message (`<` is
/// malformed anywhere in a URI, so the sentinel is collision-free); `UriError.fail` classifies
/// the message into the per-component taxonomy (richer than PHP's single `InvalidUriException`,
/// while the MESSAGES stay twin-identical). Getters are the NORMALIZED view (lowercased
/// scheme/host, dot-segments removed, unreserved percent-escapes decoded); the `raw*` family
/// returns the form as written.
/// `Core.IteratorModule` (DEC-257) — the pull-iteration protocol interface. Implementors become
/// foreach-able (the checker desugars `for … in it` to a hasNext/next while-pull); the contract
/// for `next()` past exhaustion is a fault ("iterator exhausted") — foreach never triggers it.
pub(super) const ITERATOR_PRELUDE: &str = r#"
interface Iterator<T> {
    function hasNext(): bool;
    function next(): T;
}
"#;

// DEC-273 wave 2: the uri prelude source moved to `crate::ext::uri_prelude` (colocation).

/// The `Core.Time` value model (M-TIME, `docs/specs/2026-06-28-m-time-design.md`): the pure-Phorj
/// `Instant`, `Duration`, `Date`, and `DateTime` classes. Because the prelude is run through the same
/// backends and transpiler as user code, all calendar and formatting math is byte-identical by
/// construction; the only native is the clock seam (the `Core.Time` module in `src/native/time.rs`).
/// The model is UTC-only because timezones are non-deterministic and would break the byte-identity
/// spine. Calendar math uses Hinnant's truncating-division-safe civil/day conversions, which port
/// verbatim since Phorj int division truncates toward zero (PHP `intdiv`).
pub(super) const TIME_PRELUDE: &str = r#"
import Core.String;
import Core.List;
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
  // Parse an ISO `YYYY-MM-DD` date (the inverse of `toString`; `Date.parse(d.toString())` round-trips).
  // Returns null on a malformed input — wrong shape (not three `-`-separated parts), a non-numeric
  // part, or an out-of-range month/day. Pure Phorj over `String.split`/`parseInt` → byte-identical.
  static function parse(string s) -> Date? {
    List<string> parts = String.split(s, "-");
    if (List.length(parts) != 3) { return null; }
    if (var y = String.parseInt(parts[0])) {
      if (var m = String.parseInt(parts[1])) {
        if (var d = String.parseInt(parts[2])) {
          if (m >= 1 && m <= 12 && d >= 1 && d <= 31) { return Date.of(y, m, d); }
        }
      }
    }
    return null;
  }
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
/// ([`core_module_of`]) — so a new Core module (Database, HTTP expansions) is ONE row here, not edits in
/// the eight `inject_*_prelude` fns plus the hand-synced `module_of` match this replaced.
pub(super) struct VirtualModule {
    /// The import path segments, e.g. `["Core", "Http"]`. Gates injection; also the qualifier root.
    module: &'static [&'static str],
    /// The `module_of` return value for this row's `bare_types` (the dotted module below `Core.`),
    /// e.g. `"Http"`, `"Time"`, `"Runtime.Integer"`. Only meaningful when `bare_types` is non-empty.
    qualifier: &'static str,
    /// The prelude source to inject when the module is imported; `None` for attribute-only modules
    /// (`Core.DependencyInjection`/`Core.Runtime*`) that contribute to `module_of` but inject no enum/class prelude.
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

// DEC-273 wave 3: the db prelude source moved to `crate::ext::db_prelude` (colocation).

/// The Core-module registry, in the SAME order as the pre-UA-L2 injection chain — ORDER IS
/// LOAD-BEARING: `HTTP_PRELUDE` transitively `import Core.Regex`, and Http runs BEFORE Regex, so
/// that transitive import is what triggers `Regex`-class injection for `Router.constraintOk`. A
/// reorder that broke this would still pass most tests; `examples/web/route-constraints.phg` (a
/// regex-constrained route with no explicit `import Core.Regex`) is the regression guard.
/// DEC-282 (unused-import analysis) — the names an `import Core.…;` WHOLE-MODULE row binds into a
/// file: the qualifier's leaf plus every injected bare type (`Core.IteratorModule` binds
/// `Iterator`; `Core.Runtime` binds `Entry`; …). `None` for a path that is not a whole-module row
/// (member imports bind their own leaf and are scanned by it).
pub(crate) fn core_module_bound_names(path: &[String]) -> Option<Vec<String>> {
    let vm = CORE_MODULES.iter().find(|vm| {
        vm.module.len() == path.len() && vm.module.iter().zip(path).all(|(a, b)| a == b)
    })?;
    let mut names: Vec<String> = vec![vm
        .qualifier
        .rsplit('.')
        .next()
        .unwrap_or(vm.qualifier)
        .to_string()];
    names.extend(vm.bare_types.iter().map(|s| (*s).to_string()));
    Some(names)
}

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
    // `Core.DebugModule` (DEC-238) — the dumper prelude (std-only, always compiled).
    VirtualModule {
        module: &["Core", "DebugModule"],
        qualifier: "DebugModule",
        src: Some(crate::ext::debug_prelude::PRELUDE),
        respond_bridge: None,
        member_gated: true,
        bare_types: &["Debug", "Dumped"],
    },
    // `Core.Native.Debug` — the INTERNAL renderer native.
    VirtualModule {
        module: &["Core", "Native", "Debug"],
        qualifier: "Native.Debug",
        src: None,
        respond_bridge: None,
        member_gated: false,
        bare_types: &[],
    },
    // `Core.SessionModule` (W3, TOP-20 #3) — HTTP sessions over the Core.Http value types. MUST precede
    // `Core.Http` (its `import Core.Http` transitively injects it — the forward-fold rule).
    VirtualModule {
        module: &["Core", "SessionModule"],
        qualifier: "SessionModule",
        src: Some(crate::ext::session_prelude::PRELUDE),
        respond_bridge: None,
        member_gated: true,
        bare_types: &["Session"],
    },
    // `Core.Native.Session` — the INTERNAL session-store natives (std-only; DEC-273: gated behind the `session` feature via the ext::session extension).
    VirtualModule {
        module: &["Core", "Native", "Session"],
        qualifier: "Native.Session",
        src: None,
        respond_bridge: None,
        member_gated: false,
        bare_types: &[],
    },
    // `Core.Input` (DEC-281) — the stdin module (Output's twin); prelude over Core.Native.Input.
    VirtualModule {
        module: &["Core", "Input"],
        qualifier: "Input",
        src: Some(INPUT_PRELUDE),
        respond_bridge: None,
        member_gated: true,
        bare_types: &["Input", "InputLines"],
    },
    // `Core.Native.Input` — the INTERNAL stdin natives (std-only, always compiled).
    VirtualModule {
        module: &["Core", "Native", "Input"],
        qualifier: "Native.Input",
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
        bare_types: &[
            "Cookie", "Request", "Response", "Route", "Router", "SameSite",
        ],
    },
    VirtualModule {
        module: &["Core", "Regex"],
        qualifier: "Regex",
        src: Some(crate::ext::regex_prelude::PRELUDE),
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
    // `Core.UriModule` (DEC-240) — the RFC 3986 `Uri` class + `UriError` taxonomy over `Core.Native.Uri`.
    VirtualModule {
        module: &["Core", "UriModule"],
        qualifier: "UriModule",
        src: Some(crate::ext::uri_prelude::PRELUDE),
        respond_bridge: None,
        member_gated: true,
        bare_types: &[
            "Uri",
            "UriError",
            "UriMalformedError",
            "UriBadSchemeError",
            "UriBadUserInfoError",
            "UriBadHostError",
            "UriBadPortError",
            "UriPortOutOfRangeError",
            "UriBadPathError",
            "UriBadQueryError",
            "UriBadFragmentError",
            "UriBaseNotAbsoluteError",
        ],
    },
    // `Core.DatabaseModule` (DEC-208) — the enhanced-PDO surface classes. MUST precede `Core.Native.Database` (its natives)
    // so its `import Core.Native.Database` triggers the natives being in scope (the Http→Regex ordering rule).
    VirtualModule {
        module: &["Core", "DatabaseModule"],
        qualifier: "DatabaseModule",
        src: Some(crate::ext::db_prelude::PRELUDE),
        respond_bridge: None,
        member_gated: true,
        bare_types: &[
            "Database",
            "Statement",
            "Row",
            "DatabaseError",
            "DatabaseHandle",
            // DEC-208 item H — the streaming surfaces (untyped row cursor + typed lazy stream).
            "RowStream",
            "DatabaseStream",
            // DEC-208 slice B2 — the column naming strategy enum, member-gated so
            // `new Naming.SnakeToCamel()` resolves after `import Core.DatabaseModule.Naming;` (nothing in the wind).
            "Naming",
            // DEC-208 slice C typed taxonomy — member-gated so `catch (UniqueViolationError e)` resolves
            // in user code after `import Core.DatabaseModule.UniqueViolationError;` (nothing in the wind).
            "UniqueViolationError",
            "ConstraintViolationError",
            "ConnectionError",
            "SerializationFailureError",
            "TimeoutError",
            "SyntaxError",
        ],
    },
    // `Core.IteratorModule` (DEC-257) — THE pull-iteration protocol: any implementor is foreach-able.
    // Shape developer-ruled 2026-07-16: `hasNext()/next()` (nullable element types are sound —
    // null is never a termination signal); calling `next()` past exhaustion is a documented
    // FAULT contract ("iterator exhausted") for stdlib implementors. ROW ORDER MATTERS: this
    // row sits AFTER every prelude that itself imports Core.IteratorModule (Database's streams implement
    // it) — the injection fold walks the registry once, so a dependency row must come LATER
    // than its dependents (same rule as DbSys/Result after Database).
    VirtualModule {
        module: &["Core", "IteratorModule"],
        qualifier: "IteratorModule",
        src: Some(ITERATOR_PRELUDE),
        respond_bridge: None,
        member_gated: true,
        bare_types: &["Iterator"],
    },
    // `Core.Mail` (DEC-223) — the native-mailer prelude (twin of `Core.DatabaseModule`). MUST precede `Core.Secret`
    // (its `import Core.Secret` transitively injects it — the same forward-fold rule as Database→Secret) and
    // `Core.Native.Mail` (its natives).
    VirtualModule {
        module: &["Core", "Mail"],
        qualifier: "Mail",
        src: Some(crate::ext::mail_prelude::PRELUDE),
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
            // The typed taxonomy — member-gated so `catch (AuthFailedError e)` resolves after
            // `import Core.Mail.AuthFailedError;` (nothing in the wind).
            "ConnectionFailedError",
            "AuthFailedError",
            "RecipientRejectedError",
            "TlsError",
            "InvalidAddressError",
            "MessageBuildFailedError",
            "MailTimeoutError",
            "MailIoError",
        ],
    },
    // `Core.FileSystemModule` (W3) — the typed filesystem prelude (std-only, always compiled). Taxonomy names are
    // FileSystem-PREFIXED (a bare `NotFound` bare_type captured a user-space class — caught live).
    VirtualModule {
        module: &["Core", "FileSystemModule"],
        qualifier: "FileSystemModule",
        src: Some(FS_PRELUDE),
        respond_bridge: None,
        member_gated: true,
        bare_types: &[
            "FileSystem",
            "FileSystemError",
            "FileSystemNotFoundError",
            "FileSystemPermissionDeniedError",
            "FileSystemAlreadyExistsError",
            "FileSystemNotADirectoryError",
            "FileSystemIsADirectoryError",
            "FileSystemDirNotEmptyError",
            "FileSystemIoError",
        ],
    },
    // `Core.Native.FileSystem` — the INTERNAL filesystem natives.
    VirtualModule {
        module: &["Core", "Native", "FileSystem"],
        qualifier: "Native.FileSystem",
        src: None,
        respond_bridge: None,
        member_gated: false,
        bare_types: &[],
    },
    // `Core.HttpClientModule` (W3-2) — the sync HTTP client prelude (native-only, `http-client` feature).
    VirtualModule {
        module: &["Core", "HttpClientModule"],
        qualifier: "HttpClientModule",
        src: Some(crate::ext::http_client_prelude::PRELUDE),
        respond_bridge: None,
        member_gated: true,
        bare_types: &[
            "HttpClient",
            "HttpResponse",
            "HttpClientError",
            "HttpClientHandle",
            "InvalidUrlError",
            "HttpConnectionFailedError",
            "HttpTimeoutError",
            "HttpTlsError",
            "ProtocolError",
            "TooManyRedirectsError",
            "TooLargeError",
        ],
    },
    // `Core.Secret` (Fork B) — the opaque `Secret<T>` credential wrapper. Placed AFTER `Core.DatabaseModule` because
    // `Core.DatabaseModule`'s `import Core.Secret` (for the `Database.withPassword(dsn, Secret<string>)` factory, DEC-208
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
    // `Core.Native.Database` — the INTERNAL DB natives (open/prepare/bind/query/exec/get*) the `Core.DatabaseModule` prelude
    // wraps. Native-only (no prelude); a distinct qualifier so a prelude `class Database` never collides with
    // the native leaf. Feature-gated (`database`): the natives only exist under `--features database`.
    VirtualModule {
        module: &["Core", "Native", "Database"],
        qualifier: "Native.Database",
        src: None,
        respond_bridge: None,
        member_gated: false,
        bare_types: &[],
    },
    // `Core.Native.HttpClient` — the INTERNAL HTTP-client natives (`http-client` feature).
    VirtualModule {
        module: &["Core", "Native", "HttpClient"],
        qualifier: "Native.HttpClient",
        src: None,
        respond_bridge: None,
        member_gated: false,
        bare_types: &[],
    },
    // `Core.Native.Mail` — the INTERNAL mailer natives the `Core.Mail` prelude wraps (the `Core.Native.Database`
    // twin, DEC-223). Feature-gated (`mail`): the natives only exist under `--features mail`.
    VirtualModule {
        module: &["Core", "Native", "Mail"],
        qualifier: "Native.Mail",
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
        // DEC-191 addendum: `#[Entry]` is import-gated (wind rule) — `import Core.Runtime.Entry;`
        // exactly like the UncheckedOverflow precedent one row up.
        bare_types: &["Attribute", "Entry"],
    },
    VirtualModule {
        module: &["Core", "DependencyInjection"],
        qualifier: "DependencyInjection",
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
/// `E-EXTENSION-DISABLED` diagnostic — replacing the otherwise-inevitable wall of prelude-internal
/// `E-UNKNOWN-IDENT`s (the prelude classes reference natives that do not exist in that build).
/// New gated module (e.g. `Core.Mail`) = one row here.
/// DEC-273: the gated-module table is now DERIVED from the extension registry
/// (`crate::ext::registry::EXTENSIONS` — one row per extension, each listing the Core modules it
/// provides). A new gated module = its extension row; nothing to edit here.
///
/// The dotted names of extension-provided Core modules NOT compiled into THIS build. Test
/// harnesses (the differential/example sweeps) use it to skip gated examples loudly on reduced
/// builds instead of failing on `E-EXTENSION-DISABLED` (e.g. `examples/mail/` on a build without
/// `--features mail`).
pub fn unavailable_gated_modules() -> Vec<String> {
    crate::ext::registry::disabled()
        .flat_map(|e| e.modules.iter().map(|m| (*m).to_string()))
        .collect()
}

/// If the program imports a feature-gated Core module whose feature is compiled out, the diagnostic
/// to abort with (checked on the RAW program, before any prelude injection).
/// True when the dotted import path targets `module` — the module itself or one of its members
/// (`Core.Mail` and `Core.Mail.SmtpConfig` both target `Core.Mail`; `Core.Mailer` does NOT —
/// the `.` boundary is what separates a member from an unrelated longer name). Extracted so the
/// disabled-import matching has feature-independent unit coverage (the gate body itself only
/// runs on reduced builds, which the all-features CI gate never is).
fn import_targets_module(dotted: &str, module: &str) -> bool {
    dotted == module
        || (dotted.len() > module.len()
            && dotted.starts_with(module)
            && dotted.as_bytes()[module.len()] == b'.')
}

pub(super) fn unavailable_core_module(prog: &Program) -> Option<crate::diagnostic::Diagnostic> {
    use crate::ast::Item;
    for it in &prog.items {
        let Item::Import { path, span, .. } = it else {
            continue;
        };
        let dotted = path.join(".");
        for ext in crate::ext::registry::disabled() {
            for m in ext.modules {
                if import_targets_module(&dotted, m) {
                    return Some(
                        crate::diagnostic::Diagnostic::new(
                            crate::diagnostic::Stage::Type,
                            format!(
                                "`{m}` is provided by the `{}` extension, which is not compiled into this `phg` build",
                                ext.name
                            ),
                            span.line,
                            span.col,
                        )
                        .with_code("E-EXTENSION-DISABLED")
                        .with_hint(format!(
                            "rebuild with the `{}` cargo feature — `cargo build --features {}` (default-set extensions are absent only under `--no-default-features`); `phg extensions` lists every extension and its state",
                            ext.feature, ext.feature
                        )),
                    );
                }
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
                // DEC-257: `Core.IteratorModule` injects an INTERFACE — same same-name-shadowing
                // discipline as classes/enums (a user declaration wins over the injection).
                Item::Interface(i) => !p
                    .items
                    .iter()
                    .any(|x| matches!(x, Item::Interface(y) if y.name == i.name)),
                _ => false,
            };
            if absent {
                let mut it = it;
                if let Item::Enum(e) = &mut it {
                    e.injected = true;
                }
                if let Item::Interface(i) = &mut it {
                    i.injected = true;
                }
                prepend.push(it);
            }
        }
        // Http serve bridge (DEC-191): synthesize `respond` wrapping the program's #[Entry]
        // WEB handler (`(Request): Response`, resolved by ATTRIBUTE — the magic `handle` name is
        // retired), when no `respond` exists. The wrapper calls the entry by its actual path
        // (top-level name, or `Class.method` for a static entry).
        if let Some(bridge_src) = m.respond_bridge {
            let has_respond = p
                .items
                .iter()
                .any(|x| matches!(x, Item::Function(f) if f.name == "respond"));
            let web =
                crate::ast::entry_for(p, crate::ast::EntryRole::Web).map(|(cls, f)| match cls {
                    Some(c) => format!("{c}.{}", f.name),
                    None => f.name.clone(),
                });
            if let (Some(callee), false) = (web, has_respond) {
                let src = bridge_src.replace("handle(req)", &format!("{callee}(req)"));
                if let Ok(bridge) = lex_parse(&src) {
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

#[cfg(test)]
mod gate_tests {
    use super::import_targets_module;

    #[test]
    fn import_matching_covers_module_member_and_lookalike() {
        assert!(import_targets_module("Core.Mail", "Core.Mail"));
        assert!(import_targets_module("Core.Mail.SmtpConfig", "Core.Mail"));
        assert!(import_targets_module(
            "Core.DatabaseModule.Database",
            "Core.DatabaseModule"
        ));
        // A LONGER unrelated name must not match (the `.` boundary).
        assert!(!import_targets_module("Core.Mailer", "Core.Mail"));
        // A shorter prefix of the module is not the module.
        assert!(!import_targets_module("Core", "Core.Mail"));
        // Unrelated entirely.
        assert!(!import_targets_module("Core.Output", "Core.Mail"));
    }
}
