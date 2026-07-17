//!
//! DEC-273 wave 2: colocated with the `uri` extension. Compiled UNCONDITIONALLY (the
//! `CORE_MODULES` const array references it on every build; on a gated build the disabled-import
//! gate rejects the import before this prelude could matter).

pub const PRELUDE: &str = r#"
import Core.Native.Uri as NativeUri;
import Core.String;

open class UriError implements Error {
  constructor(public string message) {}
  static function fail(string message): never throws UriError {
    if (message == "The port is out of range") { throw new UriPortOutOfRangeError(message); }
    if (message == "The specified base URI must be absolute") { throw new UriBaseNotAbsoluteError(message); }
    if (message == "The specified scheme is malformed") { throw new UriBadSchemeError(message); }
    if (message == "The specified userinfo is malformed") { throw new UriBadUserInfoError(message); }
    if (message == "The specified host is malformed") { throw new UriBadHostError(message); }
    if (message == "The specified port is malformed") { throw new UriBadPortError(message); }
    if (message == "The specified path is malformed") { throw new UriBadPathError(message); }
    if (message == "The specified query is malformed") { throw new UriBadQueryError(message); }
    if (message == "The specified fragment is malformed") { throw new UriBadFragmentError(message); }
    throw new UriMalformedError(message);
  }
}
class UriMalformedError extends UriError { constructor(string message) { parent.constructor(message); } }
class UriBadSchemeError extends UriError { constructor(string message) { parent.constructor(message); } }
class UriBadUserInfoError extends UriError { constructor(string message) { parent.constructor(message); } }
class UriBadHostError extends UriError { constructor(string message) { parent.constructor(message); } }
class UriBadPortError extends UriError { constructor(string message) { parent.constructor(message); } }
class UriPortOutOfRangeError extends UriError { constructor(string message) { parent.constructor(message); } }
class UriBadPathError extends UriError { constructor(string message) { parent.constructor(message); } }
class UriBadQueryError extends UriError { constructor(string message) { parent.constructor(message); } }
class UriBadFragmentError extends UriError { constructor(string message) { parent.constructor(message); } }
class UriBaseNotAbsoluteError extends UriError { constructor(string message) { parent.constructor(message); } }

class Uri {
  private constructor(public string raw) {}
  static function parse(string s): Uri throws UriError { return Uri.wrap(NativeUri.parse(s))?; }
  static function wrap(string r): Uri throws UriError {
    if (String.startsWith(r, "<<E>>")) { return UriError.fail(String.removePrefix(r, "<<E>>"))?; }
    return new Uri(r);
  }
  function scheme(): string? { return NativeUri.scheme(this.raw); }
  function rawScheme(): string? { return NativeUri.rawScheme(this.raw); }
  function userInfo(): string? { return NativeUri.userInfo(this.raw); }
  function rawUserInfo(): string? { return NativeUri.rawUserInfo(this.raw); }
  function username(): string? { return NativeUri.username(this.raw); }
  function rawUsername(): string? { return NativeUri.rawUsername(this.raw); }
  function password(): string? { return NativeUri.password(this.raw); }
  function rawPassword(): string? { return NativeUri.rawPassword(this.raw); }
  function host(): string? { return NativeUri.host(this.raw); }
  function rawHost(): string? { return NativeUri.rawHost(this.raw); }
  function port(): int? { return NativeUri.port(this.raw); }
  function path(): string { return NativeUri.path(this.raw); }
  function rawPath(): string { return NativeUri.rawPath(this.raw); }
  function query(): string? { return NativeUri.query(this.raw); }
  function rawQuery(): string? { return NativeUri.rawQuery(this.raw); }
  function fragment(): string? { return NativeUri.fragment(this.raw); }
  function rawFragment(): string? { return NativeUri.rawFragment(this.raw); }
  function withScheme(string? scheme): Uri throws UriError { return Uri.wrap(NativeUri.withScheme(this.raw, scheme))?; }
  function withUserInfo(string? userInfo): Uri throws UriError { return Uri.wrap(NativeUri.withUserInfo(this.raw, userInfo))?; }
  function withHost(string? host): Uri throws UriError { return Uri.wrap(NativeUri.withHost(this.raw, host))?; }
  function withPort(int? port): Uri throws UriError { return Uri.wrap(NativeUri.withPort(this.raw, port))?; }
  function withPath(string path): Uri throws UriError { return Uri.wrap(NativeUri.withPath(this.raw, path))?; }
  function withQuery(string? query): Uri throws UriError { return Uri.wrap(NativeUri.withQuery(this.raw, query))?; }
  function withFragment(string? fragment): Uri throws UriError { return Uri.wrap(NativeUri.withFragment(this.raw, fragment))?; }
  function resolve(string reference): Uri throws UriError { return Uri.wrap(NativeUri.resolve(this.raw, reference))?; }
  function equals(Uri other): bool { return NativeUri.equals(this.raw, other.raw, false); }
  function equalsIncludingFragment(Uri other): bool { return NativeUri.equals(this.raw, other.raw, true); }
  function toString(): string { return NativeUri.toText(this.raw); }
  function toRawString(): string { return this.raw; }
  // Percent-encoding statics (DEC-279 — the former `Core.Url` module, merged here). Pure string
  // transforms over the `Core.Native.Uri` percent-encoding natives; byte-identical to PHP
  // `urlencode`/`rawurlencode`/`urldecode`/`rawurldecode`. Decoders yield `null` on invalid-UTF-8
  // output. `encodeComponent`/`decodeComponent` (RFC 3986, space ⇒ `%20`) dropped the old
  // `UriComponent` infix — the `Uri.` qualifier already says it.
  static function encodeForm(string s): string { return NativeUri.encodeForm(s); }
  static function encodeComponent(string s): string { return NativeUri.encodeComponent(s); }
  static function decodeForm(string s): string? { return NativeUri.decodeForm(s); }
  static function decodeComponent(string s): string? { return NativeUri.decodeComponent(s); }
}
"#;
