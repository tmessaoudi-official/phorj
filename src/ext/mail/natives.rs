//! `Core.Mail` — the native mailer (DEC-223), architecturally a TWIN of `Core.DatabaseModule` (DEC-208).
//!
//! LADDER (invariant 14, case 2 — native-only): PHP's `mail()` has no SMTP auth, no TLS, and is
//! header-injection-prone — there is no faithful safe PHP map, so `phg transpile` hard-errors with
//! `E-TRANSPILE-MAIL` (pipeline ladder gate) rather than silently downgrading. The natives are all
//! `pure:false`, so any importing program is auto-quarantined from the byte-identity differential;
//! correctness is `tests/mail.rs` (deterministic `file`/`null` transports; SMTP round-trip gated on a
//! reachable Mailpit) — the `tests/database.rs` pattern.
//!
//! Shape (mirrors `src/ext/database/`): opaque handles ride [`Value::Db`] via the [`DbObject`]
//! erase-then-downcast pattern (`MailerObj` — a transport; `EmailObj` — a message draft the
//! prelude's `Email` builder mutates). Natives return the prelude-local `MailResult<T>` (Ok|Err) —
//! never a hard fault on a mail error — and the prelude throws the typed `MailError` taxonomy off
//! the `<<Kind>>` marker prefix, exactly like `DatabaseError` (kinds: ConnectionFailedError / AuthFailedError /
//! RecipientRejectedError / TlsError / InvalidAddressError / MessageBuildFailedError / TimeoutError / Io).
//!
//! MIME is composed by `lettre`'s builder (RFC-correct multipart) in the sibling [`super::mime`]
//! module; the handle types + `MailResult` wrappers live in [`super::handles`] and the draft-builder
//! natives in [`super::build`] (file-size cap, Invariant 13). The SMTP transport is lettre's BLOCKING
//! `SmtpTransport` (no tokio at the phorj-facing API); credentials via `Core.Secret` (never retained;
//! only a redacted description is stored). TLS posture (DEC-265, [`super::mime::smtp_tls_choice`]): no-auth
//! fakers stay STARTTLS-opportunistic, but an AUTHENTICATED connection REQUIRES TLS by default (implicit
//! on 465, STARTTLS-required otherwise) so a MITM strip can't leak the password — the loud
//! `allowInsecureAuth` opt-out is the only exception.

use super::build::*;
use super::handles::*;
use super::mime::*;
use crate::native::{NativeEval, NativeFn};
use crate::types::Ty;
use crate::value::Value;
use lettre::transport::smtp::authentication::Credentials;
use lettre::transport::smtp::client::{Tls, TlsParameters};
use lettre::{Message, SendmailTransport, SmtpTransport, Transport};
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

// ── Native bodies ────────────────────────────────────────────────────────────────────────────────────

pub(super) fn mailer(transport: TransportKind) -> Value {
    Value::Db(Rc::new(MailerObj {
        transport,
        dkim: RefCell::new(None),
    }))
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
        .map_err(|_| format!("<<ConnectionFailedError>>Core.Mail: invalid SMTP port {port}"))?;
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
pub(super) fn file_transport_inner(args: &[Value]) -> Result<Value, String> {
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
        .map_err(|e| format!("<<MessageBuildFailedError>>Core.Mail: invalid DKIM key: {e}"))?;
    *mailer.dkim.borrow_mut() = Some(DkimConfig::default_config(
        selector.to_string(),
        domain.to_string(),
        key,
    ));
    Ok(Value::Null)
}

/// Deliver one built message over the mailer's transport.
pub(super) fn deliver(mailer: &MailerObj, msg: &Message) -> Result<(), String> {
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

/// The `Core.Native.Mail` registry entries — the INTERNAL natives the `Core.Mail` prelude wraps (the
/// `Core.Native.Database` twin). Every handle is the reserved opaque `MailHandle`; every native is `pure:false`
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
            module: "Core.Native.Mail",
            name,
            params,
            ret,
            pure: false,
            eval: NativeEval::Pure(eval),
            lift_from: &[],
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

// Unit tests live in the sibling `tests.rs` (file-size cap, Invariant 13), mounted as a child
// module so they see this module's private items via `use super::*`.
