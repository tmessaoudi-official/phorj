//! The `Core.Http` prelude source (split from `preludes.rs` per Invariant 13 — content unchanged).

/// The canonical `Core.Http` types, injected (below) when a program imports `Core.Http` (M6 W1 →
/// stdlib). The portable handler model — `handle(Request): Response` — at the value level: `Request`
/// and `Response` are immutable values; `Request.parse(bytes) -> Request?` and `resp.serialize()`
/// round-trip the HTTP/1.1 wire form. The bodies reuse `Core.Bytes`/`Core.String` (so the prelude also
/// imports them), so this is the same proven logic as `examples/web/handler.phg`, promoted to the
/// stdlib behind the static-method API (slice B0). Flows through every backend as ordinary classes.
pub(crate) const HTTP_PRELUDE: &str = r#"
import Core.Bytes;
import Core.String;
import Core.List;
import Core.Regex;
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
    // DEC-331 slice 2: route params land in the MUTABLE attributes bag (PSR-7 convention; the
    // one documented mutable bag) — `req.param(name)` reads them back. NB this mutates the
    // caller's request (Rc-shared handle) — recorded deviation from the old `withParams` copy.
    mutable int pi = 0;
    int pn = List.length(params);
    while (pi + 1 < pn) {
      req.attributes.set(params[pi], params[pi + 1]);
      pi += 2;
    }
    var composed = Router.compose(this.mws, chosen.handler);
    return composed(req);
  }
}
"#;
