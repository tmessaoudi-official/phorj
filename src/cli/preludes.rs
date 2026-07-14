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
    return new Request(method, path, body, lines, []);
  }
}
class Response {
  constructor(public int status, public bytes body, public List<string> headerLines) {}
  static function text(int status, string body): Response {
    return new Response(status, Bytes.fromString(body), ["Content-Type: text/plain"]);
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
    Router sub = builder(new Router([], []));
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
    mutable List<string> out = Router.idStrs([]);
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
/// and read only through `expose()` — the `value` field is private, and a `Secret` instance is not a
/// `string`, so printing/interpolating it is a clean type error (the primary, loud guarantee; no
/// runtime `***`). Reuses the generic-class machinery (`Box<T>`) wholesale — no new `Op`/`Value`/`Ty`.
/// Injected by [`inject_core_modules`] via the `Core.Secret` registry row — a no-op unless
/// `Core.Secret` is imported and no `Secret` class is already declared. The transpiler adds `final`
/// + `#[\SensitiveParameter]` for this class by name.
pub(super) const SECRET_PRELUDE: &str =
    "class Secret<T> { constructor(private T value) {} function expose(): T { return this.value; } }";

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

// Prelude-local result carrier (NOT Core.Result — see the native docs on injection order).
enum DbResult<T> { Ok(T value), Err(string message) }

class DbError implements Error {
  constructor(public string message) {}
  // `throw` is a statement, not an expression, so it cannot be a `match` arm value directly. This
  // `never`-returning helper lets a `DbResult.Err(e)` arm raise a catchable `DbError` as an expression
  // (`DbResult.Err(e) => DbError.fail(e)`) — a call to a `never` function types as the bottom type, unifying
  // with the `Success` arm's value type.
  static function fail(string message): never throws DbError { throw new DbError(message); }
}

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
}

class Statement {
  constructor(private DbHandle raw) {}
  function bind(string | int | float | bool value): Statement throws DbError {
    return match (DbSys.bind(this.raw, value)) { DbResult.Ok(h) => new Statement(h), DbResult.Err(e) => DbError.fail(e)? };
  }
  function bindNamed(string name, string | int | float | bool value): Statement throws DbError {
    return match (DbSys.bindNamed(this.raw, name, value)) { DbResult.Ok(h) => new Statement(h), DbResult.Err(e) => DbError.fail(e)? };
  }
  function exec(): int throws DbError {
    return match (DbSys.exec(this.raw)) { DbResult.Ok(n) => n, DbResult.Err(e) => DbError.fail(e)? };
  }
  function query(): List<Row> throws DbError {
    return match (DbSys.query(this.raw)) { DbResult.Ok(rows) => Statement.wrapRows(rows), DbResult.Err(e) => DbError.fail(e)? };
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

class Db {
  // DEC-221: opening a connection can fail, so the constructor itself declares `throws DbError` and
  // opens directly — `new Db(dsn)` (fail-fast, exactly like PHP's `new PDO`). No static factory. The
  // handle is COMPUTED in the body (not a promoted param), so the field is `mutable` (set once here).
  private mutable DbHandle raw;
  constructor(string dsn) throws DbError {
    this.raw = match (DbSys.connect(dsn)) { DbResult.Ok(h) => h, DbResult.Err(e) => DbError.fail(e)? };
  }
  function prepare(string sql): Statement throws DbError {
    return match (DbSys.prepare(this.raw, sql)) { DbResult.Ok(h) => new Statement(h), DbResult.Err(e) => DbError.fail(e)? };
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
        module: &["Core", "Secret"],
        qualifier: "Secret",
        src: Some(SECRET_PRELUDE),
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
    // `Core.Db` (DEC-208) — the enhanced-PDO surface classes. MUST precede `Core.DbSys` (its natives)
    // so its `import Core.DbSys` triggers the natives being in scope (the Http→Regex ordering rule).
    VirtualModule {
        module: &["Core", "Db"],
        qualifier: "Db",
        src: Some(DB_PRELUDE),
        respond_bridge: None,
        member_gated: true,
        bare_types: &["Db", "Statement", "Row", "DbError", "DbHandle"],
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
