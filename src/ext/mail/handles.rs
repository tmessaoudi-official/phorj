//! `Core.Mail` handle types + `MailResult` wrappers — the opaque `Value::Db` payloads (transport +
//! draft) and the `DatabaseResult`-mechanism Ok|Err constructors (split per Invariant 13).

use crate::value::{DbObject, EnumVal, Value};
use lettre::message::Mailbox;
use lettre::{SendmailTransport, SmtpTransport};
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

// ── MailResult wrappers (the DatabaseResult mechanism, verbatim) ───────────────────────────────────────────

fn success(v: Value) -> Value {
    Value::Enum(Rc::new(EnumVal {
        ty: "MailResult".into(),
        variant: "Ok".into(),
        payload: crate::value::Payload::One(v),
    }))
}

fn failure(msg: String) -> Value {
    Value::Enum(Rc::new(EnumVal {
        ty: "MailResult".into(),
        variant: "Err".into(),
        payload: crate::value::Payload::One(Value::Str(msg.into())),
    }))
}

pub(super) fn wrap(inner: Result<Value, String>) -> Value {
    match inner {
        Ok(v) => success(v),
        Err(msg) => failure(msg),
    }
}

// ── Handles ──────────────────────────────────────────────────────────────────────────────────────────

/// The transport behind a `Mailer` handle. SMTP holds the built blocking transport + a redacted
/// description (host:port, never the password); `File` numbers its `.eml`s per mailer; `Null` counts.
pub(super) enum TransportKind {
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

pub(super) struct MailerObj {
    pub(super) transport: TransportKind,
    /// DKIM signing config (domain, selector, RSA private key PEM), applied at `send` when set.
    pub(super) dkim: RefCell<Option<lettre::message::dkim::DkimConfig>>,
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
pub(super) struct Att {
    pub(super) cid: Option<String>,
    pub(super) filename: String,
    pub(super) mime: String,
    pub(super) bytes: Vec<u8>,
}

#[derive(Debug, Default, Clone)]
pub(super) struct Draft {
    pub(super) from: Option<Mailbox>,
    pub(super) reply_to: Option<Mailbox>,
    pub(super) to: Vec<Mailbox>,
    pub(super) cc: Vec<Mailbox>,
    pub(super) bcc: Vec<Mailbox>,
    pub(super) subject: Option<String>,
    pub(super) text: Option<String>,
    pub(super) html: Option<String>,
    pub(super) attachments: Vec<Att>,
}

#[derive(Debug)]
pub(super) struct EmailObj {
    pub(super) draft: RefCell<Draft>,
}

impl DbObject for EmailObj {
    fn kind(&self) -> &'static str {
        "email-draft"
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub(super) fn as_mailer(v: &Value) -> Result<&MailerObj, String> {
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

pub(super) fn as_email(v: &Value) -> Result<&EmailObj, String> {
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
