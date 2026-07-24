//! The DEC-331 slice-2 rich `Request` prelude source (spec `docs/specs/2026-07-23-rich-request.md`),
//! injected as part of `Core.Http` (second `srcs` fragment beside `http_prelude.rs` — Inv-13 split).
//!
//! Design (panel-certified plan, SLICE-STATE 2026-07-24):
//!   * bags are pure-phorj classes over native-parsed data → they transpile as class shape for free;
//!   * `Request.parse` is the EAGER-validating wire constructor (D8a's ruled default): null on any
//!     malformed/oversize input, so the untouched `respond` bridge 400s it — parse NEVER faults;
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
  // The EAGER wire constructor: null = malformed or oversize (the respond bridge's 400), NEVER a
  // fault. Also the single path fake/withers rebuild through (one parsing story).
  static function parse(bytes raw): Request? {
    string nl = Bytes.toString(b"\x0d\x0a") ?? "";
    int sep = Bytes.find(raw, b"\x0d\x0a\x0d\x0a") ?? -1;
    if (sep < 0) { return null; }
    bytes bodyBytes = Bytes.slice(raw, sep + 4, Bytes.length(raw));
    string head = Bytes.toString(Bytes.slice(raw, 0, sep)) ?? "";
    List<string> lines = String.split(head, nl);
    List<string> rl = String.split(lines[0], " ");
    if (List.length(rl) < 2) { return null; }
    string method = rl[0];
    string target = rl[1];
    // Body stash decision is native-side, single-sourced with the caps: -2 oversize, -1 inline.
    int stash = NativeHttp.stashBody(bodyBytes);
    if (stash == -2) { return null; }
    bytes inline = if (stash >= 0) { b"" } else { bodyBytes };
    RequestBody body = new RequestBody(inline, stash);
    // Header lines (everything after the request line) → lowercased-key bag.
    List<string> headerLines = List.slice(lines, 1, List.length(lines));
    Map<string, List<string>> headerMap = Request.headerPairs(headerLines);
    HeaderBag headers = new HeaderBag(headerMap);
    // Split the target into decoded path + query bag.
    mutable string path = target;
    mutable string queryString = "";
    if (var q = String.indexOf(target, "?")) {
      path = String.substring(target, 0, q);
      queryString = String.substring(target, q + 1, String.length(target));
    }
    ParamBag query = new ParamBag(NativeHttp.parseQuery(queryString));
    // Cookies: every `cookie` header, pairs split on `;`, FIRST `=` only, names case-SENSITIVE.
    ParamBag cookies = new ParamBag(Request.cookiePairs(new HeaderBag(headerMap)));
    // Form + files by content type (urlencoded + multipart, D8c/D8d).
    string contentType = headers.get("content-type") ?? "";
    mutable Map<string, List<string>> formMap = NativeHttp.parseQuery("");
    mutable List<UploadedFile> fileItems = new List<UploadedFile>();
    mutable List<string> fileFields = new List<string>();
    if (String.startsWith(contentType, "application/x-www-form-urlencoded")) {
      formMap = NativeHttp.parseQuery(Bytes.toString(bodyBytes) ?? "");
    }
    // An EMPTY body with a multipart content-type parses to empty form/files — there is no body
    // to be malformed, and the fake/wither builder passes through this state legitimately
    // (`withHeader("content-type", …)` before `withBody(…)`). Recorded build decision.
    if (String.startsWith(contentType, "multipart/form-data") && Bytes.length(bodyBytes) > 0) {
      string boundary = Request.boundaryOf(contentType);
      if (boundary == "") { return null; }
      if (var parts = NativeHttp.parseMultipart(bodyBytes, boundary)) {
        // Field parts fold into the form bag structurally (body order = first-wins order; values
        // verbatim — multipart field values are NOT urlencoded). File parts go through the same
        // native stash decision as the whole body (per-part spill above the threshold).
        formMap = Request.multipartFields(parts);
        for (MultipartPart p in parts) {
          if (p.fileName != "") {
            int fh = NativeHttp.stashBody(p.content);
            if (fh == -2) { return null; }
            bytes finline = if (fh >= 0) { b"" } else { p.content };
            UploadedFile upfile = new UploadedFile(p.fileName, Bytes.length(p.content), p.contentType, finline, fh);
            fileItems = List.concat(fileItems, [upfile]);
            fileFields = List.concat(fileFields, [p.name]);
          }
        }
      } else {
        return null;
      }
    }
    return new Request(
      method, NativeHttp.decodePath(path), query, headers, cookies,
      new ParamBag(formMap), new FileBag(fileItems, fileFields), body,
      new AttrBag(new Map<string, string>()),
      target, headerLines, bodyBytes
    );
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
    string nl = Bytes.toString(b"\x0d\x0a") ?? "";
    string joined = String.join(headerLines, nl);
    string head = if (joined == "") { "{method} {target} HTTP/1.1{nl}{nl}" }
      else { "{method} {target} HTTP/1.1{nl}{joined}{nl}{nl}" };
    if (var req = Request.parse(Bytes.concat(Bytes.fromString(head), body))) { return req; }
    panic("rebuilt request no longer parses (fake/withHeader/withBody produced a malformed request)");
  }
  private static function headerPairs(List<string> lines): Map<string, List<string>> {
    mutable Map<string, List<string>> out = new Map<string, List<string>>();
    for (string line in lines) {
      if (String.contains(line, ":")) {
        List<string> kv = String.splitOnce(line, ":");
        string key = String.lowerCase(String.trim(kv[0]));
        string value = String.trim(kv[1]);
        List<string> prev = Map.get(out, key) ?? new List<string>();
        out[key] = List.concat(prev, [value]);
      }
    }
    return out;
  }
  private static function cookiePairs(HeaderBag headers): Map<string, List<string>> {
    mutable Map<string, List<string>> out = new Map<string, List<string>>();
    for (string line in headers.getAll("cookie")) {
      for (string piece in String.split(line, ";")) {
        string p = String.trim(piece);
        if (p == "") { continue; }
        mutable string k = p;
        mutable string v = "";
        if (var eq = String.indexOf(p, "=")) {
          k = String.substring(p, 0, eq);
          v = String.substring(p, eq + 1, String.length(p));
        }
        List<string> prev = Map.get(out, k) ?? new List<string>();
        out[k] = List.concat(prev, [v]);
      }
    }
    return out;
  }
  private static function multipartFields(List<MultipartPart> parts): Map<string, List<string>> {
    mutable Map<string, List<string>> out = new Map<string, List<string>>();
    for (MultipartPart p in parts) {
      if (p.fileName == "") {
        List<string> prev = Map.get(out, p.name) ?? new List<string>();
        out[p.name] = List.concat(prev, [Bytes.toString(p.content) ?? ""]);
      }
    }
    return out;
  }
  private static function boundaryOf(string contentType): string {
    if (var b = String.indexOf(contentType, "boundary=")) {
      string rest = String.substring(contentType, b + 9, String.length(contentType));
      if (String.startsWith(rest, "\"")) {
        string inner = String.substring(rest, 1, String.length(rest));
        if (var q = String.indexOf(inner, "\"")) { return String.substring(inner, 0, q); }
        return "";
      }
      if (var semi = String.indexOf(rest, ";")) { return String.trim(String.substring(rest, 0, semi)); }
      return String.trim(rest);
    }
    return "";
  }
}
"#;
