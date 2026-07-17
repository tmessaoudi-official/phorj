//! The `Secret<T>` opaque-wrapper type, injected when a program imports `Core.Secret` (Fork B,
//! `docs/specs/2026-06-28-secret-type-design.md`). A `Secret<T>` value is constructed `new Secret(x)`
//! `Core.Mail` (DEC-223) — the native mailer prelude, a TWIN of `Core.DatabaseModule`: prelude classes wrap the
//! `Core.Native.Mail` natives, errors flow through the prelude-local `MailResult<T>` + a `<<Kind>>`-parsing
//! `MailError.fail`, and the transport credential is a `Core.Secret`. Native-only (`E-TRANSPILE-MAIL`
//! — see the pipeline ladder gate); every symbol import-gated (nothing in the wind). Surface notes
//! realized under bounded autonomy (developer to confirm, recorded in C-decisions DEC-230): the spec's
//! `new SmtpConfig(host, port, user, Secret pw)` 4-arg form is realized as the static factory
//! `SmtpConfig.withAuth(...)` (phorj has NO constructor default params / overloading — gap flagged in
//! KNOWN_ISSUES), and `new SendmailTransport()` path override is `SendmailTransport.at(path)`.
//!
//! DEC-273 wave 3: colocated with the `mail` extension. Compiled UNCONDITIONALLY (the
//! `CORE_MODULES` const array references it on every build; the disabled-import gate rejects the
//! import on gated builds before the prelude matters).

pub const PRELUDE: &str = r#"
import Core.Native.Mail as NativeMail;
import Core.String;
import Core.List;
// `Core.Secret` provides the opaque credential wrapper for `SmtpConfig.withAuth` (the Database.withPassword
// discipline): the SMTP password never sits in plaintext in user code and is never retained by the
// transport (only a redacted `smtp://host:port` description is stored).
import Core.Secret;

// Prelude-local result carrier (NOT Core.Result — see the Core.DatabaseModule native docs on injection order).
enum MailResult<T> { Ok(T value), Err(string message) }

open class MailError implements Error {
  constructor(public string message) {}
  // The single classification point (the `DatabaseError.fail` mechanism): natives tag failures with a
  // `<<Kind>>` marker; this strips it and throws the matching TYPED subtype, so
  // `catch (AuthFailedError e)` is precise while `catch (MailError e)` still catches everything.
  static function fail(string message): never throws MailError {
    if (String.startsWith(message, "<<ConnectionFailedError>>")) { throw new ConnectionFailedError(String.removePrefix(message, "<<ConnectionFailedError>>")); }
    if (String.startsWith(message, "<<AuthFailedError>>")) { throw new AuthFailedError(String.removePrefix(message, "<<AuthFailedError>>")); }
    if (String.startsWith(message, "<<RecipientRejectedError>>")) { throw new RecipientRejectedError(String.removePrefix(message, "<<RecipientRejectedError>>")); }
    if (String.startsWith(message, "<<TlsError>>")) { throw new TlsError(String.removePrefix(message, "<<TlsError>>")); }
    if (String.startsWith(message, "<<InvalidAddressError>>")) { throw new InvalidAddressError(String.removePrefix(message, "<<InvalidAddressError>>")); }
    if (String.startsWith(message, "<<MessageBuildFailedError>>")) { throw new MessageBuildFailedError(String.removePrefix(message, "<<MessageBuildFailedError>>")); }
    if (String.startsWith(message, "<<TimeoutError>>")) { throw new MailTimeoutError(String.removePrefix(message, "<<TimeoutError>>")); }
    if (String.startsWith(message, "<<Io>>")) { throw new MailIoError(String.removePrefix(message, "<<Io>>")); }
    throw new MailError(message);
  }
}

// Typed error taxonomy (spec §5, shaped like DatabaseError's). `MailTimeoutError`/`MailIoError` carry the Mail prefix
// because bare `TimeoutError` already belongs to Core.DatabaseModule's taxonomy (two injected classes may not collide).
class ConnectionFailedError extends MailError { constructor(string message) { parent.constructor(message); } }
class AuthFailedError extends MailError { constructor(string message) { parent.constructor(message); } }
class RecipientRejectedError extends MailError { constructor(string message) { parent.constructor(message); } }
class TlsError extends MailError { constructor(string message) { parent.constructor(message); } }
class InvalidAddressError extends MailError { constructor(string message) { parent.constructor(message); } }
class MessageBuildFailedError extends MailError { constructor(string message) { parent.constructor(message); } }
class MailTimeoutError extends MailError { constructor(string message) { parent.constructor(message); } }
class MailIoError extends MailError { constructor(string message) { parent.constructor(message); } }

// A typed, injection-safe address (spec §4): validated AT CONSTRUCTION (DEC-221 throwing ctor), so an
// `Address` value is valid-by-construction everywhere downstream — raw-header injection (the #1 PHP
// mail() footgun) is structurally impossible.
class Address {
  constructor(public string email, public string name) throws MailError {
    match (NativeMail.addressCheck(email, name)) { MailResult.Ok(_) => Address.ok(), MailResult.Err(e) => MailError.fail(e)? };
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
    this.raw = NativeMail.emailNew();
  }
  function from(Address a): Email throws MailError {
    this.raw = match (NativeMail.from(this.raw, a.email, a.name)) { MailResult.Ok(h) => h, MailResult.Err(e) => MailError.fail(e)? };
    return this;
  }
  function replyTo(Address a): Email throws MailError {
    this.raw = match (NativeMail.replyTo(this.raw, a.email, a.name)) { MailResult.Ok(h) => h, MailResult.Err(e) => MailError.fail(e)? };
    return this;
  }
  function to(Address a): Email throws MailError {
    this.raw = match (NativeMail.to(this.raw, a.email, a.name)) { MailResult.Ok(h) => h, MailResult.Err(e) => MailError.fail(e)? };
    return this;
  }
  function cc(Address a): Email throws MailError {
    this.raw = match (NativeMail.cc(this.raw, a.email, a.name)) { MailResult.Ok(h) => h, MailResult.Err(e) => MailError.fail(e)? };
    return this;
  }
  function bcc(Address a): Email throws MailError {
    this.raw = match (NativeMail.bcc(this.raw, a.email, a.name)) { MailResult.Ok(h) => h, MailResult.Err(e) => MailError.fail(e)? };
    return this;
  }
  function subject(string s): Email throws MailError {
    this.raw = match (NativeMail.subject(this.raw, s)) { MailResult.Ok(h) => h, MailResult.Err(e) => MailError.fail(e)? };
    return this;
  }
  function text(string body): Email throws MailError {
    this.raw = match (NativeMail.text(this.raw, body)) { MailResult.Ok(h) => h, MailResult.Err(e) => MailError.fail(e)? };
    return this;
  }
  function html(string body): Email throws MailError {
    this.raw = match (NativeMail.html(this.raw, body)) { MailResult.Ok(h) => h, MailResult.Err(e) => MailError.fail(e)? };
    return this;
  }
  function attach(Attachment a): Email throws MailError {
    if (var d = a.data) {
      this.raw = match (NativeMail.attachBytes(this.raw, a.name, a.mime, d)) { MailResult.Ok(h) => h, MailResult.Err(e) => MailError.fail(e)? };
    } else {
      this.raw = match (NativeMail.attachFile(this.raw, a.path, a.mime)) { MailResult.Ok(h) => h, MailResult.Err(e) => MailError.fail(e)? };
    }
    return this;
  }
  function attachInline(string cid, Attachment a): Email throws MailError {
    if (var d = a.data) {
      this.raw = match (NativeMail.attachInlineBytes(this.raw, cid, a.mime, d)) { MailResult.Ok(h) => h, MailResult.Err(e) => MailError.fail(e)? };
    } else {
      this.raw = match (NativeMail.attachInline(this.raw, cid, a.path, a.mime)) { MailResult.Ok(h) => h, MailResult.Err(e) => MailError.fail(e)? };
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
      SendmailTransport s => match (NativeMail.sendmail(s.path)) { MailResult.Ok(h) => h, MailResult.Err(e) => MailError.fail(e)? },
      FileTransport f => match (NativeMail.fileTransport(f.dir)) { MailResult.Ok(h) => h, MailResult.Err(e) => MailError.fail(e)? },
      NullTransport n => match (NativeMail.nullTransport()) { MailResult.Ok(h) => h, MailResult.Err(e) => MailError.fail(e)? }
    };
  }
  private static function connectSmtp(SmtpConfig cfg): MailHandle throws MailError {
    mutable string pw = "";
    if (var s = cfg.password) { pw = s.expose(); }
    return match (NativeMail.smtp(cfg.host, cfg.port, cfg.user, pw, cfg.tls, cfg.allowInsecureAuth)) { MailResult.Ok(h) => h, MailResult.Err(e) => MailError.fail(e)? };
  }
  // Arm DKIM signing (RSA key PEM as a `Secret`) for every subsequent send on this mailer.
  function dkim(string domain, string selector, Secret<string> privateKeyPem): Mailer throws MailError {
    match (NativeMail.dkim(this.raw, domain, selector, privateKeyPem.expose())) { MailResult.Ok(_) => Mailer.ok(), MailResult.Err(e) => MailError.fail(e)? };
    return this;
  }
  function send(Email e): void throws MailError {
    match (NativeMail.send(this.raw, e.handle())) { MailResult.Ok(_) => Mailer.ok(), MailResult.Err(e2) => MailError.fail(e2)? };
  }
  // Batch over one reused transport connection. Fail-fast: the first failure aborts with that
  // message's typed error (the count already delivered is in the message). Returns the sent count.
  function sendAll(List<Email> emails): int throws MailError {
    mutable List<MailHandle> handles = new List<MailHandle>();
    for (Email e in emails) { handles = List.append(handles, e.handle()); }
    return match (NativeMail.sendAll(this.raw, handles)) { MailResult.Ok(n) => n, MailResult.Err(e) => MailError.fail(e)? };
  }
  private static function ok(): void {}
}
"#;
