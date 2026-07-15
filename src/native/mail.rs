//! `Core.Mail` — the native mailer (DEC-223), architecturally a TWIN of `Core.Db` (DEC-208).
//!
//! LADDER (invariant 14, case 2 — native-only): PHP's `mail()` has no SMTP auth, no TLS, and is
//! header-injection-prone — there is no faithful safe PHP map, so `phg transpile` hard-errors with
//! `E-TRANSPILE-MAIL` (pipeline ladder gate) rather than silently downgrading. The natives are all
//! `pure:false`, so any importing program is auto-quarantined from the byte-identity differential;
//! correctness is `tests/mail.rs` (deterministic `file`/`null` transports; SMTP round-trip gated on a
//! reachable Mailpit) — the `tests/db.rs` pattern.
//!
//! Shape (mirrors `src/native/db/`): opaque handles ride [`Value::Db`] via the [`DbObject`]
//! erase-then-downcast pattern ([`MailerObj`] — a transport; [`EmailObj`] — a message draft the
//! prelude's `Email` builder mutates). Natives return the prelude-local `MailResult<T>` (Ok|Err) —
//! never a hard fault on a mail error — and the prelude throws the typed [`MailError`] taxonomy off
//! the `<<Kind>>` marker prefix, exactly like `DbError` (kinds: ConnectionFailed / AuthFailed /
//! RecipientRejected / TlsError / InvalidAddress / MessageBuildFailed / Timeout / Io).
//!
//! MIME is composed by `lettre`'s builder (RFC-correct multipart): text-only → a plain body;
//! `.html(body)` → `multipart/alternative` with an AUTO-DERIVED plaintext part ([`html_to_text`],
//! overridable via `.text`); inline CID attachments nest under `multipart/related`; file attachments
//! under `multipart/mixed`. The SMTP transport is lettre's BLOCKING `SmtpTransport` (no tokio at the
//! phorj-facing API); credentials via `Core.Secret` (never retained; only a redacted description is
//! stored). TLS posture (DEC-265, [`smtp_tls_choice`]): no-auth fakers stay STARTTLS-opportunistic, but
//! an AUTHENTICATED connection REQUIRES TLS by default (implicit on 465, STARTTLS-required otherwise) so
//! a MITM strip can't leak the password — the loud `allowInsecureAuth` opt-out is the only exception.

use super::{NativeEval, NativeFn};
use crate::types::Ty;
use crate::value::{DbObject, EnumVal, Value};
use lettre::message::header::ContentType;
use lettre::message::{Attachment as MimeAttachment, Mailbox, MultiPart, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::transport::smtp::client::{Tls, TlsParameters};
use lettre::{Message, SendmailTransport, SmtpTransport, Transport};
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

// ── MailResult wrappers (the DbResult mechanism, verbatim) ───────────────────────────────────────────

fn success(v: Value) -> Value {
    Value::Enum(Rc::new(EnumVal {
        ty: "MailResult".into(),
        variant: "Ok".into(),
        payload: vec![v],
    }))
}

fn failure(msg: String) -> Value {
    Value::Enum(Rc::new(EnumVal {
        ty: "MailResult".into(),
        variant: "Err".into(),
        payload: vec![Value::Str(msg.into())],
    }))
}

fn wrap(inner: Result<Value, String>) -> Value {
    match inner {
        Ok(v) => success(v),
        Err(msg) => failure(msg),
    }
}

// ── Handles ──────────────────────────────────────────────────────────────────────────────────────────

/// The transport behind a `Mailer` handle. SMTP holds the built blocking transport + a redacted
/// description (host:port, never the password); `File` numbers its `.eml`s per mailer; `Null` counts.
enum TransportKind {
    Smtp {
        transport: SmtpTransport,
        desc: String,
    },
    Sendmail(SendmailTransport),
    File {
        dir: PathBuf,
        counter: Cell<u64>,
    },
    Null {
        sent: Cell<u64>,
    },
}

struct MailerObj {
    transport: TransportKind,
    /// DKIM signing config (domain, selector, RSA private key PEM), applied at `send` when set.
    dkim: RefCell<Option<lettre::message::dkim::DkimConfig>>,
}

impl std::fmt::Debug for MailerObj {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match &self.transport {
            TransportKind::Smtp { desc, .. } => desc.as_str(),
            TransportKind::Sendmail(_) => "sendmail",
            TransportKind::File { .. } => "file",
            TransportKind::Null { .. } => "null",
        };
        f.debug_struct("MailerObj")
            .field("transport", &label)
            .finish()
    }
}

impl DbObject for MailerObj {
    fn kind(&self) -> &'static str {
        "mailer"
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// One attachment on a draft: `cid = Some(...)` → inline (multipart/related), else a regular file
/// attachment (multipart/mixed).
#[derive(Debug, Clone)]
struct Att {
    cid: Option<String>,
    filename: String,
    mime: String,
    bytes: Vec<u8>,
}

#[derive(Debug, Default, Clone)]
struct Draft {
    from: Option<Mailbox>,
    reply_to: Option<Mailbox>,
    to: Vec<Mailbox>,
    cc: Vec<Mailbox>,
    bcc: Vec<Mailbox>,
    subject: Option<String>,
    text: Option<String>,
    html: Option<String>,
    attachments: Vec<Att>,
}

#[derive(Debug)]
struct EmailObj {
    draft: RefCell<Draft>,
}

impl DbObject for EmailObj {
    fn kind(&self) -> &'static str {
        "email-draft"
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn as_mailer(v: &Value) -> Result<&MailerObj, String> {
    match v {
        Value::Db(h) => h
            .as_any()
            .downcast_ref::<MailerObj>()
            .ok_or_else(|| "Core.Mail: expected a Mailer".to_string()),
        other => Err(format!(
            "Core.Mail: expected a Mailer, got {}",
            other.type_name()
        )),
    }
}

fn as_email(v: &Value) -> Result<&EmailObj, String> {
    match v {
        Value::Db(h) => h
            .as_any()
            .downcast_ref::<EmailObj>()
            .ok_or_else(|| "Core.Mail: expected an Email".to_string()),
        other => Err(format!(
            "Core.Mail: expected an Email, got {}",
            other.type_name()
        )),
    }
}

// ── Address parsing (typed, injection-safe by construction) ─────────────────────────────────────────

/// Parse an address (+ optional display name) into a lettre [`Mailbox`]. This is the ONE gate every
/// recipient/sender passes, so raw-header injection (`"a@b\r\nBcc: attacker"`) is structurally
/// impossible — lettre rejects control characters and folds the display name per RFC 2047.
fn parse_mailbox(email: &str, name: &str) -> Result<Mailbox, String> {
    let addr: lettre::Address = email
        .parse()
        .map_err(|e| format!("<<InvalidAddress>>Core.Mail: invalid address `{email}`: {e}"))?;
    let display = if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    };
    Ok(Mailbox::new(display, addr))
}

// ── HTML → plaintext auto-derivation (deterministic, std-only) ──────────────────────────────────────

/// Derive the `multipart/alternative` plaintext part from an HTML body when the user supplied only
/// `.html(...)`. Deliberately simple + deterministic: tags are dropped (block-ish tags become
/// newlines, `<li>` becomes a bullet), the common entities are decoded, whitespace runs collapse.
/// A user needing better fidelity supplies `.text(...)` explicitly (which overrides this).
fn html_to_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut chars = html.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '<' {
            let mut tag = String::new();
            for t in chars.by_ref() {
                if t == '>' {
                    break;
                }
                tag.push(t);
            }
            let tag_name = tag
                .trim_start_matches('/')
                .split([' ', '\t', '\n'])
                .next()
                .unwrap_or("")
                .to_ascii_lowercase();
            match tag_name.as_str() {
                "br" | "p" | "div" | "tr" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "ul"
                | "ol" => {
                    if !out.ends_with('\n') && !out.is_empty() {
                        out.push('\n');
                    }
                }
                "li" => {
                    if !out.ends_with('\n') && !out.is_empty() {
                        out.push('\n');
                    }
                    if !tag.starts_with('/') {
                        out.push_str("- ");
                    }
                }
                _ => {}
            }
        } else if c == '&' {
            let mut ent = String::new();
            let mut matched = false;
            for _ in 0..8 {
                match chars.peek() {
                    Some(&e) if e != ';' && e != '<' && e != '&' && !e.is_whitespace() => {
                        ent.push(e);
                        chars.next();
                    }
                    Some(&';') => {
                        chars.next();
                        matched = true;
                        break;
                    }
                    _ => break,
                }
            }
            if matched {
                match ent.as_str() {
                    "amp" => out.push('&'),
                    "lt" => out.push('<'),
                    "gt" => out.push('>'),
                    "quot" => out.push('"'),
                    "#39" | "apos" => out.push('\''),
                    "nbsp" => out.push(' '),
                    other => {
                        out.push('&');
                        out.push_str(other);
                        out.push(';');
                    }
                }
            } else {
                out.push('&');
                out.push_str(&ent);
            }
        } else {
            out.push(c);
        }
    }
    // Collapse >2 consecutive newlines and trim outer whitespace.
    let mut collapsed = String::with_capacity(out.len());
    let mut nl = 0;
    for c in out.chars() {
        if c == '\n' {
            nl += 1;
            if nl <= 2 {
                collapsed.push(c);
            }
        } else {
            nl = 0;
            collapsed.push(c);
        }
    }
    collapsed.trim().to_string()
}

// ── MIME composition ─────────────────────────────────────────────────────────────────────────────────

/// Build the RFC-correct [`Message`] from a draft. Structure:
/// `mixed( related( alternative(text, html) | text, inline* ) | body, attachment* )` — each level
/// added only when needed, so a plain-text no-attachment mail is a plain singlepart.
fn build_message(draft: &Draft) -> Result<Message, String> {
    let err = |m: String| format!("<<MessageBuildFailed>>Core.Mail: {m}");
    let from = draft
        .from
        .clone()
        .ok_or_else(|| err("the email has no `from` address".into()))?;
    if draft.to.is_empty() && draft.cc.is_empty() && draft.bcc.is_empty() {
        return Err(err("the email has no recipients (`to`/`cc`/`bcc`)".into()));
    }
    let mut b = Message::builder().from(from);
    if let Some(r) = &draft.reply_to {
        b = b.reply_to(r.clone());
    }
    for m in &draft.to {
        b = b.to(m.clone());
    }
    for m in &draft.cc {
        b = b.cc(m.clone());
    }
    for m in &draft.bcc {
        b = b.bcc(m.clone());
    }
    b = b.subject(draft.subject.clone().unwrap_or_default());

    let (inlines, files): (Vec<&Att>, Vec<&Att>) =
        draft.attachments.iter().partition(|a| a.cid.is_some());
    let mime_of = |a: &Att| -> Result<ContentType, String> {
        a.mime.parse::<ContentType>().map_err(|_| {
            err(format!(
                "attachment `{}` has an invalid MIME type `{}`",
                a.filename, a.mime
            ))
        })
    };

    // The content core: text | alternative(text, html).
    enum Core {
        Text(String),
        Alt(MultiPart),
    }
    let core = match (&draft.text, &draft.html) {
        (_, Some(html)) => {
            let plain = draft.text.clone().unwrap_or_else(|| html_to_text(html));
            Core::Alt(MultiPart::alternative_plain_html(plain, html.clone()))
        }
        (Some(text), None) => Core::Text(text.clone()),
        (None, None) => Core::Text(String::new()),
    };

    // Wrap inline CIDs under `related`.
    let with_inlines: Result<Option<MultiPart>, String> = if inlines.is_empty() {
        Ok(None)
    } else {
        let mut related = match core {
            Core::Alt(ref alt) => MultiPart::related().multipart(alt.clone()),
            Core::Text(ref t) => MultiPart::related().singlepart(SinglePart::plain(t.clone())),
        };
        for a in &inlines {
            let cid = a.cid.clone().expect("partitioned on cid");
            related = related
                .singlepart(MimeAttachment::new_inline(cid).body(a.bytes.clone(), mime_of(a)?));
        }
        Ok(Some(related))
    };
    let with_inlines = with_inlines?;

    let msg = if files.is_empty() {
        match (with_inlines, core) {
            (Some(related), _) => b.multipart(related),
            (None, Core::Alt(alt)) => b.multipart(alt),
            (None, Core::Text(t)) => b.body(t),
        }
    } else {
        let mut mixed = match (with_inlines, core) {
            (Some(related), _) => MultiPart::mixed().multipart(related),
            (None, Core::Alt(alt)) => MultiPart::mixed().multipart(alt),
            (None, Core::Text(t)) => MultiPart::mixed().singlepart(SinglePart::plain(t)),
        };
        for a in &files {
            mixed = mixed.singlepart(
                MimeAttachment::new(a.filename.clone()).body(a.bytes.clone(), mime_of(a)?),
            );
        }
        b.multipart(mixed)
    };
    msg.map_err(|e| err(e.to_string()))
}

// ── SMTP error classification ────────────────────────────────────────────────────────────────────────

/// Best-effort lettre-SMTP-error → taxonomy marker. Auth failures are permanent 5xx on AUTH (535);
/// recipient rejections are the 55x family; TLS and connection problems surface as client errors.
fn smtp_err_kind(e: &lettre::transport::smtp::Error) -> &'static str {
    if let Some(code) = e.status() {
        let n = u16::from(code.severity as u8) * 100
            + u16::from(code.category as u8) * 10
            + u16::from(code.detail as u8);
        return match n {
            535 | 534 | 530 => "AuthFailed",
            550..=554 => "RecipientRejected",
            _ => "ConnectionFailed",
        };
    }
    let text = e.to_string();
    if text.contains("tls") || text.contains("TLS") {
        "TlsError"
    } else if text.contains("timed out") || text.contains("timeout") {
        "Timeout"
    } else {
        "ConnectionFailed"
    }
}

// ── Native bodies ────────────────────────────────────────────────────────────────────────────────────

fn mailer(transport: TransportKind) -> Value {
    Value::Db(Rc::new(MailerObj {
        transport,
        dkim: RefCell::new(None),
    }))
}

/// `MailSys.smtp(host, port, user, pw, tlsMode, allowInsecureAuth)` — `user == ""` → unauthenticated
/// (Mailpit-style fakers), which stay STARTTLS-opportunistic (fakers rarely offer TLS).
///
/// SECURITY (DEC-265): when credentials ARE set, TLS is REQUIRED by default so a MITM that strips the
/// STARTTLS advertisement can't force the password onto a plaintext channel (the pre-fix
/// `Tls::Opportunistic` did exactly that). `tlsMode` selects HOW: `auto` = implicit TLS on 465 else
/// STARTTLS-required; `starttls` = force STARTTLS-required; `implicit` = force TLS-from-connect.
/// `allowInsecureAuth = true` is the explicit, loud opt-out (trusted-LAN authed SMTP without TLS) —
/// it drops back to opportunistic; nothing else can express authenticated-plaintext. The password
/// arrives via `Secret.expose()` in the prelude and is NEVER retained here — only `host:port` is stored.
/// The TLS posture for an SMTP connection (DEC-265) — the security-critical decision, factored out
/// PURE so it is unit-tested without a live server. `Wrapper` = implicit TLS from connect; `Required`
/// = STARTTLS mandatory (a strip/downgrade fails the connect); `Opportunistic` = TLS if offered else
/// plaintext. The invariant: **authenticated (`has_creds`) is NEVER `Opportunistic` unless the caller
/// set the explicit, loud `allow_insecure` opt-out** — so a credential can never ride a channel that a
/// MITM could have quietly downgraded to plaintext.
#[derive(Debug, PartialEq, Eq)]
enum SmtpTlsChoice {
    Wrapper,
    Required,
    Opportunistic,
}

fn smtp_tls_choice(has_creds: bool, allow_insecure: bool, mode: &str, port: u16) -> SmtpTlsChoice {
    if has_creds && !allow_insecure {
        match mode {
            "implicit" => SmtpTlsChoice::Wrapper,
            "starttls" => SmtpTlsChoice::Required,
            // "auto": implicit TLS on the submissions port 465, STARTTLS-required elsewhere.
            _ if port == 465 => SmtpTlsChoice::Wrapper,
            _ => SmtpTlsChoice::Required,
        }
    } else {
        // No credentials (fakers), or the explicit insecure opt-out — nothing to protect / opted out.
        SmtpTlsChoice::Opportunistic
    }
}

fn smtp_inner(args: &[Value]) -> Result<Value, String> {
    let (host, port, user, pw, tls_mode, allow_insecure) = match args {
        [Value::Str(h), Value::Int(p), Value::Str(u), Value::Str(w), Value::Str(m), Value::Bool(ai)] => {
            (h.as_str(), *p, u.as_str(), w.as_str(), m.as_str(), *ai)
        }
        _ => {
            return Err(
                "Core.Mail.__smtp expects (string, int, string, string, string, bool)".into(),
            )
        }
    };
    let port = u16::try_from(port)
        .map_err(|_| format!("<<ConnectionFailed>>Core.Mail: invalid SMTP port {port}"))?;
    let mut builder = SmtpTransport::builder_dangerous(host).port(port);
    let has_creds = !user.is_empty();
    let tls_params = || {
        TlsParameters::new(host.to_string()).map_err(|e| {
            format!("<<TlsError>>Core.Mail: cannot build TLS params for `{host}`: {e}")
        })
    };
    match smtp_tls_choice(has_creds, allow_insecure, tls_mode, port) {
        // Require TLS — a downgrade/strip makes the connection FAIL rather than leak the password.
        SmtpTlsChoice::Wrapper => builder = builder.tls(Tls::Wrapper(tls_params()?)),
        SmtpTlsChoice::Required => builder = builder.tls(Tls::Required(tls_params()?)),
        // No credentials, OR the explicit allowInsecureAuth opt-out: opportunistic (TLS if offered).
        SmtpTlsChoice::Opportunistic => {
            if let Ok(params) = TlsParameters::new(host.to_string()) {
                builder = builder.tls(Tls::Opportunistic(params));
            }
        }
    }
    if has_creds {
        builder = builder.credentials(Credentials::new(user.to_string(), pw.to_string()));
    }
    Ok(mailer(TransportKind::Smtp {
        transport: builder.build(),
        desc: format!("smtp://{host}:{port}"),
    }))
}

/// `MailSys.sendmail(path)` — `path == ""` → the platform default (`/usr/sbin/sendmail`).
fn sendmail_inner(args: &[Value]) -> Result<Value, String> {
    let path = match args {
        [Value::Str(p)] => p.as_str(),
        _ => return Err("Core.Mail.__sendmail expects (string path)".into()),
    };
    let t = if path.is_empty() {
        SendmailTransport::new()
    } else {
        SendmailTransport::new_with_command(path)
    };
    Ok(mailer(TransportKind::Sendmail(t)))
}

/// `MailSys.fileTransport(dir)` — writes each sent message as `phorj-mail-<n>.eml` under `dir`
/// (created if absent). The deterministic offline test transport.
fn file_transport_inner(args: &[Value]) -> Result<Value, String> {
    let dir = match args {
        [Value::Str(d)] => PathBuf::from(d.as_str()),
        _ => return Err("Core.Mail.__fileTransport expects (string dir)".into()),
    };
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("<<Io>>Core.Mail: cannot create `{}`: {e}", dir.display()))?;
    Ok(mailer(TransportKind::File {
        dir,
        counter: Cell::new(0),
    }))
}

fn null_transport_inner(args: &[Value]) -> Result<Value, String> {
    if !args.is_empty() {
        return Err("Core.Mail.__nullTransport expects no arguments".into());
    }
    Ok(mailer(TransportKind::Null { sent: Cell::new(0) }))
}

/// `MailSys.dkim(mailer, domain, selector, privateKeyPem)` — arm DKIM signing for every subsequent
/// `send` on this mailer. The key PEM arrives via `Secret.expose()` and lives only inside lettre's
/// signing config (never printed; the config is not introspectable from phorj).
fn dkim_inner(args: &[Value]) -> Result<Value, String> {
    use lettre::message::dkim::{DkimConfig, DkimSigningAlgorithm, DkimSigningKey};
    let (m, domain, selector, pem) = match args {
        [m, Value::Str(d), Value::Str(s), Value::Str(k)] => (m, d.as_str(), s.as_str(), k.as_str()),
        _ => return Err("Core.Mail.__dkim expects (Mailer, string, string, string)".into()),
    };
    let mailer = as_mailer(m)?;
    let key = DkimSigningKey::new(pem, DkimSigningAlgorithm::Rsa)
        .map_err(|e| format!("<<MessageBuildFailed>>Core.Mail: invalid DKIM key: {e}"))?;
    *mailer.dkim.borrow_mut() = Some(DkimConfig::default_config(
        selector.to_string(),
        domain.to_string(),
        key,
    ));
    Ok(Value::Null)
}

fn email_new_inner(args: &[Value]) -> Result<Value, String> {
    if !args.is_empty() {
        return Err("Core.Mail.__emailNew expects no arguments".into());
    }
    Ok(Value::Db(Rc::new(EmailObj {
        draft: RefCell::new(Draft::default()),
    })))
}

/// `MailSys.addressCheck(email)` — the `new Address(...)` validation gate (throwing ctor, DEC-221
/// pattern): an invalid address is a catchable `InvalidAddress` at CONSTRUCTION, so an `Address`
/// value is valid by construction everywhere downstream.
fn address_check_inner(args: &[Value]) -> Result<Value, String> {
    let (email, name) = match args {
        [Value::Str(e), Value::Str(n)] => (e.as_str(), n.as_str()),
        _ => return Err("Core.Mail.__addressCheck expects (string, string)".into()),
    };
    parse_mailbox(email, name)?;
    Ok(Value::Null)
}

/// Shared shape of the draft-mutating builder natives: `(email-handle, …fields) → the same handle`.
fn with_draft(
    args: &[Value],
    who: &str,
    f: impl FnOnce(&mut Draft, &[Value]) -> Result<(), String>,
) -> Result<Value, String> {
    let Some((h, rest)) = args.split_first() else {
        return Err(format!("Core.Mail.__{who} expects an Email handle"));
    };
    let email = as_email(h)?;
    f(&mut email.draft.borrow_mut(), rest)?;
    Ok(h.clone())
}

fn two_strs<'a>(rest: &'a [Value], who: &str) -> Result<(&'a str, &'a str), String> {
    match rest {
        [Value::Str(a), Value::Str(b)] => Ok((a.as_str(), b.as_str())),
        _ => Err(format!("Core.Mail.__{who} expects (string, string)")),
    }
}

fn email_from_inner(args: &[Value]) -> Result<Value, String> {
    with_draft(args, "from", |d, rest| {
        let (e, n) = two_strs(rest, "from")?;
        d.from = Some(parse_mailbox(e, n)?);
        Ok(())
    })
}

fn email_reply_to_inner(args: &[Value]) -> Result<Value, String> {
    with_draft(args, "replyTo", |d, rest| {
        let (e, n) = two_strs(rest, "replyTo")?;
        d.reply_to = Some(parse_mailbox(e, n)?);
        Ok(())
    })
}

fn email_to_inner(args: &[Value]) -> Result<Value, String> {
    with_draft(args, "to", |d, rest| {
        let (e, n) = two_strs(rest, "to")?;
        d.to.push(parse_mailbox(e, n)?);
        Ok(())
    })
}

fn email_cc_inner(args: &[Value]) -> Result<Value, String> {
    with_draft(args, "cc", |d, rest| {
        let (e, n) = two_strs(rest, "cc")?;
        d.cc.push(parse_mailbox(e, n)?);
        Ok(())
    })
}

fn email_bcc_inner(args: &[Value]) -> Result<Value, String> {
    with_draft(args, "bcc", |d, rest| {
        let (e, n) = two_strs(rest, "bcc")?;
        d.bcc.push(parse_mailbox(e, n)?);
        Ok(())
    })
}

fn email_subject_inner(args: &[Value]) -> Result<Value, String> {
    with_draft(args, "subject", |d, rest| match rest {
        [Value::Str(s)] => {
            d.subject = Some(s.as_str().to_string());
            Ok(())
        }
        _ => Err("Core.Mail.__subject expects (string)".into()),
    })
}

fn email_text_inner(args: &[Value]) -> Result<Value, String> {
    with_draft(args, "text", |d, rest| match rest {
        [Value::Str(s)] => {
            d.text = Some(s.as_str().to_string());
            Ok(())
        }
        _ => Err("Core.Mail.__text expects (string)".into()),
    })
}

fn email_html_inner(args: &[Value]) -> Result<Value, String> {
    with_draft(args, "html", |d, rest| match rest {
        [Value::Str(s)] => {
            d.html = Some(s.as_str().to_string());
            Ok(())
        }
        _ => Err("Core.Mail.__html expects (string)".into()),
    })
}

/// Read a file attachment's bytes, guessing the MIME from the extension when the caller passed `""`
/// (a small, honest table — pass an explicit MIME for anything exotic).
fn read_attachment(path: &str, mime: &str) -> Result<(String, String, Vec<u8>), String> {
    let bytes = std::fs::read(path)
        .map_err(|e| format!("<<Io>>Core.Mail: cannot read attachment `{path}`: {e}"))?;
    let filename = std::path::Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path)
        .to_string();
    let mime = if mime.is_empty() {
        match filename
            .rsplit('.')
            .next()
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str()
        {
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            "pdf" => "application/pdf",
            "txt" => "text/plain",
            "html" | "htm" => "text/html",
            "csv" => "text/csv",
            "json" => "application/json",
            "zip" => "application/zip",
            _ => "application/octet-stream",
        }
        .to_string()
    } else {
        mime.to_string()
    };
    Ok((filename, mime, bytes))
}

fn email_attach_file_inner(args: &[Value]) -> Result<Value, String> {
    with_draft(args, "attach", |d, rest| {
        let (path, mime) = two_strs(rest, "attach")?;
        let (filename, mime, bytes) = read_attachment(path, mime)?;
        d.attachments.push(Att {
            cid: None,
            filename,
            mime,
            bytes,
        });
        Ok(())
    })
}

fn email_attach_bytes_inner(args: &[Value]) -> Result<Value, String> {
    with_draft(args, "attachBytes", |d, rest| match rest {
        [Value::Str(name), Value::Str(mime), Value::Bytes(b)] => {
            d.attachments.push(Att {
                cid: None,
                filename: name.as_str().to_string(),
                mime: mime.as_str().to_string(),
                bytes: (**b).clone(),
            });
            Ok(())
        }
        _ => Err("Core.Mail.__attachBytes expects (string, string, bytes)".into()),
    })
}

fn email_attach_inline_bytes_inner(args: &[Value]) -> Result<Value, String> {
    with_draft(args, "attachInlineBytes", |d, rest| match rest {
        [Value::Str(cid), Value::Str(mime), Value::Bytes(b)] => {
            d.attachments.push(Att {
                cid: Some(cid.as_str().to_string()),
                filename: String::new(),
                mime: mime.as_str().to_string(),
                bytes: (**b).clone(),
            });
            Ok(())
        }
        _ => Err("Core.Mail.__attachInlineBytes expects (string cid, string mime, bytes)".into()),
    })
}

fn email_attach_inline_inner(args: &[Value]) -> Result<Value, String> {
    with_draft(args, "attachInline", |d, rest| match rest {
        [Value::Str(cid), Value::Str(path), Value::Str(mime)] => {
            let (filename, mime, bytes) = read_attachment(path.as_str(), mime.as_str())?;
            d.attachments.push(Att {
                cid: Some(cid.as_str().to_string()),
                filename,
                mime,
                bytes,
            });
            Ok(())
        }
        _ => Err("Core.Mail.__attachInline expects (string cid, string path, string mime)".into()),
    })
}

/// Deliver one built message over the mailer's transport.
fn deliver(mailer: &MailerObj, msg: &Message) -> Result<(), String> {
    match &mailer.transport {
        TransportKind::Smtp { transport, desc } => transport
            .send(msg)
            .map(|_| ())
            .map_err(|e| format!("<<{}>>Core.Mail: {desc}: {e}", smtp_err_kind(&e))),
        TransportKind::Sendmail(t) => t
            .send(msg)
            .map_err(|e| format!("<<Io>>Core.Mail: sendmail: {e}")),
        TransportKind::File { dir, counter } => {
            let n = counter.get();
            counter.set(n + 1);
            let path = dir.join(format!("phorj-mail-{n}.eml"));
            std::fs::write(&path, msg.formatted())
                .map_err(|e| format!("<<Io>>Core.Mail: cannot write `{}`: {e}", path.display()))
        }
        TransportKind::Null { sent } => {
            sent.set(sent.get() + 1);
            Ok(())
        }
    }
}

fn send_one(mailer: &MailerObj, email: &EmailObj) -> Result<(), String> {
    let mut msg = build_message(&email.draft.borrow())?;
    if let Some(dkim) = mailer.dkim.borrow().as_ref() {
        msg.sign(dkim);
    }
    deliver(mailer, &msg)
}

fn send_inner(args: &[Value]) -> Result<Value, String> {
    let (m, e) = match args {
        [m, e] => (as_mailer(m)?, as_email(e)?),
        _ => return Err("Core.Mail.__send expects (Mailer, Email)".into()),
    };
    send_one(m, e)?;
    Ok(Value::Null)
}

/// `MailSys.sendAll(mailer, emails)` — batch over ONE transport/connection (lettre's SMTP transport
/// pools its connection). Fail-fast: the first failure aborts with that message's error, and the
/// count of already-delivered messages is part of the error text (no silent partial success).
fn send_all_inner(args: &[Value]) -> Result<Value, String> {
    let (m, list) = match args {
        [m, Value::List(l)] => (as_mailer(m)?, l),
        _ => return Err("Core.Mail.__sendAll expects (Mailer, List<Email>)".into()),
    };
    for (i, v) in list.iter().enumerate() {
        let e = as_email(v)?;
        send_one(m, e).map_err(|err| {
            format!("{err} (batch aborted at message {i}; {i} message(s) already sent)")
        })?;
    }
    Ok(Value::Int(i64::try_from(list.len()).unwrap_or(i64::MAX)))
}

// ── Public natives (the `wrap` discipline) + registry ───────────────────────────────────────────────

macro_rules! mail_native {
    ($name:ident, $inner:ident) => {
        fn $name(args: &[Value], _out: &mut String) -> Result<Value, String> {
            Ok(wrap($inner(args)))
        }
    };
}
mail_native!(mail_smtp, smtp_inner);
mail_native!(mail_sendmail, sendmail_inner);
mail_native!(mail_file_transport, file_transport_inner);
mail_native!(mail_null_transport, null_transport_inner);
mail_native!(mail_dkim, dkim_inner);
/// `emailNew` is infallible — it returns the bare handle (no `MailResult`), so `new Email()` needs
/// no try/throws (a builder must be cheap to open).
fn mail_email_new(args: &[Value], _out: &mut String) -> Result<Value, String> {
    email_new_inner(args)
}
mail_native!(mail_address_check, address_check_inner);
mail_native!(mail_email_from, email_from_inner);
mail_native!(mail_email_reply_to, email_reply_to_inner);
mail_native!(mail_email_to, email_to_inner);
mail_native!(mail_email_cc, email_cc_inner);
mail_native!(mail_email_bcc, email_bcc_inner);
mail_native!(mail_email_subject, email_subject_inner);
mail_native!(mail_email_text, email_text_inner);
mail_native!(mail_email_html, email_html_inner);
mail_native!(mail_email_attach_file, email_attach_file_inner);
mail_native!(mail_email_attach_bytes, email_attach_bytes_inner);
mail_native!(mail_email_attach_inline, email_attach_inline_inner);
mail_native!(
    mail_email_attach_inline_bytes,
    email_attach_inline_bytes_inner
);
mail_native!(mail_send, send_inner);
mail_native!(mail_send_all, send_all_inner);

/// The `Core.MailSys` registry entries — the INTERNAL natives the `Core.Mail` prelude wraps (the
/// `Core.DbSys` twin). Every handle is the reserved opaque `MailHandle`; every native is `pure:false`
/// (network / filesystem side effects → byte-identity quarantine) and returns `MailResult<T>`. The
/// `php` emitters are unreachable placeholders — `Core.Mail` is `E-TRANSPILE-MAIL` native-only
/// (pipeline ladder gate rejects the program before any emitter runs).
pub fn mail_natives() -> Vec<NativeFn> {
    let handle = || Ty::Named("MailHandle".into(), vec![]);
    let res = |t: Ty| Ty::Named("MailResult".into(), vec![t]);
    let opt_null = || Ty::Optional(Box::new(Ty::Named("MailHandle".into(), vec![])));
    let entry =
        |name: &'static str,
         params: Vec<Ty>,
         ret: Ty,
         eval: fn(&[Value], &mut String) -> Result<Value, String>| NativeFn {
            module: "Core.MailSys",
            name,
            params,
            ret,
            pure: false,
            eval: NativeEval::Pure(eval),
            php: |a| a.first().cloned().unwrap_or_else(|| "''".to_string()),
        };
    vec![
        entry(
            "smtp",
            // host, port, user, pw, tlsMode ("auto"|"starttls"|"implicit"), allowInsecureAuth (DEC-265)
            vec![
                Ty::String,
                Ty::Int,
                Ty::String,
                Ty::String,
                Ty::String,
                Ty::Bool,
            ],
            res(handle()),
            mail_smtp,
        ),
        entry("sendmail", vec![Ty::String], res(handle()), mail_sendmail),
        entry(
            "fileTransport",
            vec![Ty::String],
            res(handle()),
            mail_file_transport,
        ),
        entry("nullTransport", vec![], res(handle()), mail_null_transport),
        entry(
            "dkim",
            vec![handle(), Ty::String, Ty::String, Ty::String],
            res(opt_null()),
            mail_dkim,
        ),
        entry("emailNew", vec![], handle(), mail_email_new),
        entry(
            "addressCheck",
            vec![Ty::String, Ty::String],
            res(opt_null()),
            mail_address_check,
        ),
        entry(
            "from",
            vec![handle(), Ty::String, Ty::String],
            res(handle()),
            mail_email_from,
        ),
        entry(
            "replyTo",
            vec![handle(), Ty::String, Ty::String],
            res(handle()),
            mail_email_reply_to,
        ),
        entry(
            "to",
            vec![handle(), Ty::String, Ty::String],
            res(handle()),
            mail_email_to,
        ),
        entry(
            "cc",
            vec![handle(), Ty::String, Ty::String],
            res(handle()),
            mail_email_cc,
        ),
        entry(
            "bcc",
            vec![handle(), Ty::String, Ty::String],
            res(handle()),
            mail_email_bcc,
        ),
        entry(
            "subject",
            vec![handle(), Ty::String],
            res(handle()),
            mail_email_subject,
        ),
        entry(
            "text",
            vec![handle(), Ty::String],
            res(handle()),
            mail_email_text,
        ),
        entry(
            "html",
            vec![handle(), Ty::String],
            res(handle()),
            mail_email_html,
        ),
        entry(
            "attachFile",
            vec![handle(), Ty::String, Ty::String],
            res(handle()),
            mail_email_attach_file,
        ),
        entry(
            "attachBytes",
            vec![handle(), Ty::String, Ty::String, Ty::Bytes],
            res(handle()),
            mail_email_attach_bytes,
        ),
        entry(
            "attachInline",
            vec![handle(), Ty::String, Ty::String, Ty::String],
            res(handle()),
            mail_email_attach_inline,
        ),
        entry(
            "attachInlineBytes",
            vec![handle(), Ty::String, Ty::String, Ty::Bytes],
            res(handle()),
            mail_email_attach_inline_bytes,
        ),
        entry("send", vec![handle(), handle()], res(opt_null()), mail_send),
        entry(
            "sendAll",
            vec![handle(), Ty::List(Box::new(handle()))],
            res(Ty::Int),
            mail_send_all,
        ),
    ]
}

// Unit tests live in the sibling `mail_tests.rs` (file-size cap, Invariant 13), mounted as a child
// module so they see this module's private items via `use super::*`.
#[cfg(test)]
#[path = "mail_tests.rs"]
mod tests;
