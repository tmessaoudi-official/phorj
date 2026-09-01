//! The DEC-331 slice-2 rich `Request` prelude source (spec `docs/specs/UNIFIED-SPEC.md#rich-request--bags-uploads-eager-vs-lazy-parsing`),
//! injected as part of `Core.Http` (second `srcs` fragment beside `http_prelude.rs` — Inv-13 split).
//!
//! Design (panel-certified plan, SLICE-STATE 2026-07-24):
//!   * bags are pure-phorj classes over native-parsed data → they transpile as class shape for free;
//!   * `Request.parse` is the EAGER-validating wire constructor (D8a's ruled default): null on any
//!     malformed/oversize input, so the `Http.serve` bridge 400s it — parse NEVER faults;
//!   * `Request.fake` + the withers rebuild from the ORIGINAL raw target/header lines/body through
//!     the SAME parse path (one parsing story; never from decoded bags — decode is not idempotent);
//!   * withers FAULT on CR/LF in header names/values (fail-loud on a programming error — the
//!     rebuild-then-reparse path must not be an injection primitive; DEC-242 bar);
//!   * the `.get(k, default)` overload from D8d ships as `getOrDefault(k, fallback)` — phorj's
//!     E-OVERLOAD-RETURN rule forbids return-type-differing overloads (Core.Map precedent; recorded
//!     deviation);
//!   * ParamBag keys (query/form/cookies) are case-SENSITIVE; ONLY HeaderBag lowercases (D8d);
//!     cookie pairs split on the FIRST `=`; values verbatim;
//!   * `body.json()` memoizes via `private mutable` fields (observationally immutable) and calls
//!     ONLY the always-registered `Core.Native.Http.jsonParse` (feature story: flag-naming fault
//!     on a no-`json` build — the `Json` TYPE below is always injected via `import Core.Json`);
//!   * route params live in the mutable `attributes` bag (PSR-7 convention, §7 P3 — the ONE
//!     documented mutable bag); `req.param(name)` is a thin delegate.
pub(crate) const REQUEST_PRELUDE: &str = r#"
import Core.Native.Http as NativeHttp;
import Core.Json;
import Core.Map;
class ParamBag {
  constructor(private Map<string, List<string>> data) {}
  function get(string key): string? {
    if (var vs = Map.get(this.data, key)) { return vs[0]; }
    return null;
  }
  function getOrDefault(string key, string fallback): string { return this.get(key) ?? fallback; }
  function getAll(string key): List<string> {
    return Map.get(this.data, key) ?? new List<string>();
  }
  function has(string key): bool { return Map.has(this.data, key); }
  function all(): Map<string, List<string>> { return this.data; }
}
class HeaderBag {
  // Keys are stored lowercased by the parser; every lookup lowercases (case-INSENSITIVE, D8d).
  constructor(private Map<string, List<string>> data) {}
  function get(string name): string? {
    if (var vs = Map.get(this.data, String.lowerCase(name))) { return vs[0]; }
    return null;
  }
  function getOrDefault(string name, string fallback): string { return this.get(name) ?? fallback; }
  function getAll(string name): List<string> {
    return Map.get(this.data, String.lowerCase(name)) ?? new List<string>();
  }
  function has(string name): bool { return Map.has(this.data, String.lowerCase(name)); }
  function all(): Map<string, List<string>> { return this.data; }
}
// The ONE documented mutable bag (§7 P3): middleware scratch + route params (PSR-7 convention).
class AttrBag {
  constructor(private mutable Map<string, string> data) {}
  function get(string key): string? { return Map.get(this.data, key); }
  function getOrDefault(string key, string fallback): string { return Map.get(this.data, key) ?? fallback; }
  function has(string key): bool { return Map.has(this.data, key); }
  function all(): Map<string, string> { return this.data; }
  // Whole-map reassign (field-base element writes are a queued language slice).
  function set(string key, string value): void { this.data = Map.set(this.data, key, value); }
}
// Internal carrier the multipart native hand-builds — its field SET is the other half of the
// contract in `src/native/http/multipart.rs` (change BOTH or neither).
class MultipartPart {
  constructor(public string name, public string fileName, public string contentType, public bytes content) {}
}
class UploadedFile {
  constructor(public string name, public int size, public string contentType, private bytes inline, private int spillHandle) {}
  function bytes(): bytes {
    return if (this.spillHandle >= 0) { NativeHttp.readSpill(this.spillHandle) } else { this.inline };
  }
}
class FileBag {
  constructor(private List<UploadedFile> items, private List<string> fieldNames) {}
  function get(string field): UploadedFile? {
    mutable int i = 0;
    int n = List.length(this.fieldNames);
    while (i < n) {
      if (this.fieldNames[i] == field) { return this.items[i]; }
      i += 1;
    }
    return null;
  }
  function getAll(string field): List<UploadedFile> {
    mutable List<UploadedFile> out = new List<UploadedFile>();
    mutable int i = 0;
    int n = List.length(this.fieldNames);
    while (i < n) {
      if (this.fieldNames[i] == field) { out = List.concat(out, [this.items[i]]); }
      i += 1;
    }
    return out;
  }
  function has(string field): bool {
    if (var found = this.get(field)) { return true; }
    return false;
  }
}
class RequestBody {
  constructor(private bytes inline, private int spillHandle) {}
  mutable Json? cachedJson = null;
  mutable bool jsonParsed = false;
  function bytes(): bytes {
    return if (this.spillHandle >= 0) { NativeHttp.readSpill(this.spillHandle) } else { this.inline };
  }
  function text(): string { return Bytes.toString(this.bytes()) ?? ""; }
  function json(): Json? {
    if (!this.jsonParsed) {
      this.cachedJson = NativeHttp.jsonParse(this.bytes());
      this.jsonParsed = true;
    }
    return this.cachedJson;
  }
}
class Request {
  constructor(
    public string method,
    public string path,
    public ParamBag query,
    public HeaderBag headers,
    public ParamBag cookies,
    public ParamBag form,
    public FileBag files,
    public RequestBody body,
    public AttrBag attributes,
    private string rawTarget,
    private List<string> rawHeaderLines,
    private bytes rawBody
  ) {}
  // Route-param sugar over the attributes bag (Router.handle writes them there).
  function param(string name): string? { return this.attributes.get(name); }
  // ---- construction --------------------------------------------------------------------------
  // The EAGER wire constructor: null = malformed or oversize (the serve bridge's 400), NEVER a
  // fault. Also the single path fake/withers rebuild through (one parsing story).
  static function parse(bytes raw): Request? {
    // DEC-338: the entire wire→Request parse is nativized (Core.Native.Http.parseRequest) to flip the
    // `queryparse` 0.10× loss — one Rust/PHP twin builds the whole bag graph per parse instead of the
    // interpreter walking this body. Behaviour is byte-identical (null = malformed/oversize, the eager
    // D8a contract — never a fault). `fake`/withers still rebuild THROUGH here, so the one parsing
    // story is preserved. The former private helpers (headerPairs/cookiePairs/multipartFields/
    // boundaryOf) moved wholesale into that native + its PHP twin.
    return NativeHttp.parseRequest(raw);
  }
  // ---- fake + withers (the test-builder surface, §7 P2) ---------------------------------------
  static function fake(string method, string target): Request {
    return Request.rebuild(method, target, new List<string>(), b"");
  }
  function withHeader(string name, string value): Request {
    Request.guardHeaderText(name);
    Request.guardHeaderText(value);
    return Request.rebuild(this.method, this.rawTarget, List.concat(this.rawHeaderLines, ["{name}: {value}"]), this.rawBody);
  }
  function withCookie(string name, string value): Request {
    return this.withHeader("cookie", "{name}={value}");
  }
  function withBody(bytes b): Request {
    return Request.rebuild(this.method, this.rawTarget, this.rawHeaderLines, b);
  }
  // ---- internals -------------------------------------------------------------------------------
  private static function guardHeaderText(string s): void {
    string cr = Bytes.toString(b"\x0d") ?? "";
    string lf = Bytes.toString(b"\x0a") ?? "";
    if (String.contains(s, cr) || String.contains(s, lf)) {
      panic("header names and values must not contain CR or LF");
    }
  }
  private static function rebuild(string method, string target, List<string> headerLines, bytes body): Request {
    // The request line is `{method} {target} HTTP/1.1` — a CR/LF in EITHER field is a
    // request-line/header-injection primitive through the rebuild-then-reparse path (a `fake`
    // target smuggled a header past the header-only guard — 6C finding). Guard both fail-loud.
    Request.guardHeaderText(method);
    Request.guardHeaderText(target);
    string nl = Bytes.toString(b"\x0d\x0a") ?? "";
    string joined = String.join(headerLines, nl);
    string head = if (joined == "") { "{method} {target} HTTP/1.1{nl}{nl}" }
      else { "{method} {target} HTTP/1.1{nl}{joined}{nl}{nl}" };
    if (var req = Request.parse(Bytes.concat(Bytes.fromString(head), body))) { return req; }
    panic("rebuilt request no longer parses (fake/withHeader/withBody produced a malformed request)");
  }
}
"#;
