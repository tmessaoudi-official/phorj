//! `Core.SessionModule` (W3, TOP-20 #3 blocker) — HTTP sessions for `phg serve`, on top of the
//! `Core.Http` `Request`/`Response` value types. THROW-FREE surface (in-memory store ops are
//! total). Security defaults ON — better than PHP's opt-in ini flags: the cookie is
//! `HttpOnly; SameSite=Lax; Path=/`; ids are 128-bit OS-entropy hex; an expired/unknown cookie id
//! silently gets a FRESH empty session (never resurrected); `regenerate()` (session-fixation
//! defense) is first-class. Values are strings (structured data goes through `Core.Json` — PHP's
//! serialized `$_SESSION` does the same under the hood). Native-only (`E-TRANSPILE-SESSION`).
//!
//! DEC-273 wave 3: colocated with the `session` extension. Compiled UNCONDITIONALLY (the
//! `CORE_MODULES` const array references it on every build; the disabled-import gate rejects the
//! import on gated builds before the prelude matters).

pub const PRELUDE: &str = r#"
import Core.Native.Session as NativeSession;
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
    return new Session(NativeSession.acquire(cand, ttlSeconds));
  }
  private static function cookieSid(Request req): string {
    // DEC-331 slice 2: read through the rich Request's cookie bag (names case-SENSITIVE,
    // values split on the FIRST `=` — byte-identical to the old hand-rolled Cookie-header scan).
    return req.cookies.get("phorjsid") ?? "";
  }
  function id(): string { return this.sid; }
  function get(string key): string? { return NativeSession.get(this.sid, key); }
  function set(string key, string value): void { discard NativeSession.set(this.sid, key, value); }
  function remove(string key): void { discard NativeSession.remove(this.sid, key); }
  // Sorted (deterministic) key listing.
  function keys(): List<string> { return NativeSession.keys(this.sid); }
  function destroy(): void { discard NativeSession.destroy(this.sid); }
  // The session-fixation defense: a FRESH id carrying the same data; the old id is dead
  // immediately. Call it on every privilege change (login/logout).
  function regenerate(): Session {
    this.sid = NativeSession.regenerate(this.sid);
    return this;
  }
  // Attach the session cookie to a response — HttpOnly + SameSite=Lax + Path=/ (secure defaults;
  // add `; Secure` yourself when serving over TLS).
  function apply(Response r): Response {
    Cookie c = new Cookie("phorjsid", this.sid).secure(false);
    return r.withCookie(c);
  }
}
"#;
