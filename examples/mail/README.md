# `Core.Mail` — the native mailer (DEC-223)

A **twin of `Core.Db`**: instance-based, typed errors, `Secret` credentials, four transports behind
one surface. Native-only — `phg transpile` rejects any `import Core.Mail` program with
`E-TRANSPILE-MAIL` (PHP's `mail()` has no SMTP auth, no TLS, and is header-injection-prone; mapping
to it would silently downgrade — THE LADDER RULE forbids that). Build with `--features mail`.

## Transports

| Transport | Construction | Use |
|---|---|---|
| SMTP (no auth) | `new Mailer(new SmtpConfig("localhost", 1025))` | Real delivery; Mailpit/MailHog fakers work unauthenticated. STARTTLS-opportunistic (no credentials at risk). |
| SMTP + auth | `new Mailer(SmtpConfig.withAuth(host, 587, user, new Secret(pw)))` | The password is a `Core.Secret` — never retained, never printed. **TLS is REQUIRED when credentials are set (DEC-265)**: implicit TLS on port 465, STARTTLS-required otherwise — a MITM that strips STARTTLS makes the connection fail rather than leak the password (PHP `mail()` has no auth/TLS at all). Choose the mode with `new SmtpConfig(host, port, user, secret, "starttls")` / `"implicit"` (default `"auto"`); the explicit, loud sixth arg `true` (`allowInsecureAuth`) is the only way to permit authenticated plaintext (trusted LAN). |
| sendmail | `new Mailer(new SendmailTransport())` / `SendmailTransport.at("/usr/sbin/sendmail")` | Local MTA pipe. |
| file | `new Mailer(new FileTransport("outbox"))` | Writes `phorj-mail-<n>.eml` — the deterministic offline test transport. |
| null | `new Mailer(new NullTransport())` | Discards (dry-run). |

## Composition

```phorj
Email e = new Email()
    .from(new Address("app@x.io", "App"))
    .to(Address.of("user@y.io"))           // .to/.cc/.bcc accumulate on repeat calls
    .replyTo(Address.of("noreply@x.io"))
    .subject("Welcome")
    .html("<h1>Hi</h1><img src=\"cid:logo\">")   // auto-derives the plaintext alternative
    .attachInline("logo", Attachment.fromFile("logo.png"))
    .attach(Attachment.fromBytes("data.csv", "text/csv", csvBytes));
m.send(e);                                   // throws a typed MailError subtype on failure
int n = m.sendAll([e1, e2, e3]);             // batch, one reused connection, fail-fast
```

- **`Address` is validated at construction** (`new Address(email, name)` / `Address.of(email)`
  throws `InvalidAddressError`) — a `\r\n` smuggled into an address NEVER reaches a header, killing PHP
  `mail()`'s #1 footgun.
- **`.html(body)`** builds `multipart/alternative` with an auto-derived plaintext part; supply
  `.text(body)` to override the derivation.
- **DKIM**: `m.dkim("example.io", "sel1", new Secret(rsaPemKey))` arms signing for every send.

## Typed failures (all `extends MailError`)

`ConnectionFailedError` · `AuthFailedError` · `RecipientRejectedError` · `TlsError` · `InvalidAddressError` ·
`MessageBuildFailedError` (no `from`, no recipients, bad MIME) · `MailTimeoutError` · `MailIoError`.
`catch (MailError e)` catches them all; the subtype names the precise cause.

## Faults that cannot be runnable examples (Invariant 9)

- `phg transpile examples/mail/send.phg` → `E-TRANSPILE-MAIL` (native-only, no PHP mapping).
- `new Address("evil@x.y\r\nBcc: victim@z.w", "")` → `InvalidAddressError` at construction.
- `m.send(...)` on an Email with no `.from(...)` → `MessageBuildFailedError`.

## Testing

Unit (MIME nesting, injection gate, derivation): `src/native/mail.rs`. Integration on both backends:
`cargo test --features mail --test mail`. Live SMTP round-trip (optional):
`PHORJ_MAILPIT_SMTP=localhost:1025 cargo test --features mail --test mail` against a running Mailpit.
