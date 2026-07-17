//! `Core.HttpClientModule` (W3-2, TOP-20 #2 blocker) — the sync HTTP client prelude (the Core.DatabaseModule/Mail
//! architecture). Taxonomy names are prefixed where a bare name is already taken by another injected
//! taxonomy (`HttpTimeoutError`/`HttpTlsError`/`HttpConnectionFailedError` — `TimeoutError`/`TlsError`/
//! `ConnectionFailedError`/`ConnectionError` belong to Core.DatabaseModule / Core.Mail; injected-class dedup would
//! silently CAPTURE the other module's class — the cross-prelude collision smell recorded in
//! KNOWN_ISSUES). Native-only (`E-TRANSPILE-HTTPCLIENT`).
//!
//! DEC-273 wave 3: colocated with the `http_client` extension. Compiled UNCONDITIONALLY (the
//! `CORE_MODULES` const array references it on every build; the disabled-import gate rejects the
//! import on gated builds before the prelude matters).

pub const PRELUDE: &str = r#"
import Core.Native.HttpClient as NativeHttpClient;
import Core.String;
import Core.List;
import Core.Bytes;

// Prelude-local result carrier (NOT Core.Result — the Core.DatabaseModule injection-order rationale).
enum HcResult<T> { Ok(T value), Err(string message) }

open class HttpClientError implements Error {
  constructor(public string message) {}
  // The single classification point (the DatabaseError.fail mechanism): `<<Kind>>` marker → typed subtype.
  static function fail(string message): never throws HttpClientError {
    if (String.startsWith(message, "<<InvalidUrlError>>")) { throw new InvalidUrlError(String.removePrefix(message, "<<InvalidUrlError>>")); }
    if (String.startsWith(message, "<<ConnectionFailedError>>")) { throw new HttpConnectionFailedError(String.removePrefix(message, "<<ConnectionFailedError>>")); }
    if (String.startsWith(message, "<<TimeoutError>>")) { throw new HttpTimeoutError(String.removePrefix(message, "<<TimeoutError>>")); }
    if (String.startsWith(message, "<<TlsError>>")) { throw new HttpTlsError(String.removePrefix(message, "<<TlsError>>")); }
    if (String.startsWith(message, "<<ProtocolError>>")) { throw new ProtocolError(String.removePrefix(message, "<<ProtocolError>>")); }
    if (String.startsWith(message, "<<TooManyRedirectsError>>")) { throw new TooManyRedirectsError(String.removePrefix(message, "<<TooManyRedirectsError>>")); }
    if (String.startsWith(message, "<<TooLargeError>>")) { throw new TooLargeError(String.removePrefix(message, "<<TooLargeError>>")); }
    if (String.startsWith(message, "<<BlockedAddressError>>")) { throw new BlockedAddressError(String.removePrefix(message, "<<BlockedAddressError>>")); }
    throw new HttpClientError(message);
  }
}

class InvalidUrlError extends HttpClientError { constructor(string message) { parent.constructor(message); } }
class HttpConnectionFailedError extends HttpClientError { constructor(string message) { parent.constructor(message); } }
class HttpTimeoutError extends HttpClientError { constructor(string message) { parent.constructor(message); } }
class HttpTlsError extends HttpClientError { constructor(string message) { parent.constructor(message); } }
class ProtocolError extends HttpClientError { constructor(string message) { parent.constructor(message); } }
class TooManyRedirectsError extends HttpClientError { constructor(string message) { parent.constructor(message); } }
class TooLargeError extends HttpClientError { constructor(string message) { parent.constructor(message); } }
// DEC-270 SSRF guard: the URL resolved to a private/link-local/metadata address the client refuses by
// default. Pass `.allowPrivateHosts(true)` to permit private ranges deliberately (loopback is allowed).
class BlockedAddressError extends HttpClientError { constructor(string message) { parent.constructor(message); } }

// A completed response: status, headers (names lowercased), body as text or bytes. Inert data
// behind an opaque handle — reading it never re-touches the network.
class HttpResponse {
  constructor(private HttpClientHandle raw) {}
  function status(): int throws HttpClientError {
    return match (NativeHttpClient.status(this.raw)) { HcResult.Ok(v) => v, HcResult.Err(e) => HttpClientError.fail(e)? };
  }
  // The named header's value, or null when absent (names are case-insensitive).
  function header(string name): string? throws HttpClientError {
    return match (NativeHttpClient.header(this.raw, name)) { HcResult.Ok(v) => v, HcResult.Err(e) => HttpClientError.fail(e)? };
  }
  function headerNames(): List<string> throws HttpClientError {
    return match (NativeHttpClient.headerNames(this.raw)) { HcResult.Ok(v) => v, HcResult.Err(e) => HttpClientError.fail(e)? };
  }
  // The body as UTF-8 text (a non-UTF-8 body is a clean ProtocolError steering to bodyBytes()).
  function body(): string throws HttpClientError {
    return match (NativeHttpClient.bodyText(this.raw)) { HcResult.Ok(v) => v, HcResult.Err(e) => HttpClientError.fail(e)? };
  }
  function bodyBytes(): bytes throws HttpClientError {
    return match (NativeHttpClient.bodyBytes(this.raw)) { HcResult.Ok(v) => v, HcResult.Err(e) => HttpClientError.fail(e)? };
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
    HttpClientHandle h = match (NativeHttpClient.request(method, url, headerNames, headerValues, body, this.timeoutMs, this.maxRedirects, this.allowPrivate)) { HcResult.Ok(v) => v, HcResult.Err(e) => HttpClientError.fail(e)? };
    return new HttpResponse(h);
  }
}
"#;
