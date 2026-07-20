//! `Core.Mail` draft-builder natives — the `new Email()` handle plus every field/attachment mutator
//! (`(email-handle, …fields) → the same handle`), split from the transports/send path per Invariant 13.

use super::handles::{as_email, Att, Draft, EmailObj};
use super::mime::parse_mailbox;
use crate::value::Value;
use std::cell::RefCell;
use std::rc::Rc;

pub(super) fn email_new_inner(args: &[Value]) -> Result<Value, String> {
    if !args.is_empty() {
        return Err("Core.Mail.__emailNew expects no arguments".into());
    }
    Ok(Value::Db(Rc::new(EmailObj {
        draft: RefCell::new(Draft::default()),
    })))
}

/// `MailSys.addressCheck(email)` — the `new Address(...)` validation gate (throwing ctor, DEC-221
/// pattern): an invalid address is a catchable `InvalidAddressError` at CONSTRUCTION, so an `Address`
/// value is valid by construction everywhere downstream.
pub(super) fn address_check_inner(args: &[Value]) -> Result<Value, String> {
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

pub(super) fn email_from_inner(args: &[Value]) -> Result<Value, String> {
    with_draft(args, "from", |d, rest| {
        let (e, n) = two_strs(rest, "from")?;
        d.from = Some(parse_mailbox(e, n)?);
        Ok(())
    })
}

pub(super) fn email_reply_to_inner(args: &[Value]) -> Result<Value, String> {
    with_draft(args, "replyTo", |d, rest| {
        let (e, n) = two_strs(rest, "replyTo")?;
        d.reply_to = Some(parse_mailbox(e, n)?);
        Ok(())
    })
}

pub(super) fn email_to_inner(args: &[Value]) -> Result<Value, String> {
    with_draft(args, "to", |d, rest| {
        let (e, n) = two_strs(rest, "to")?;
        d.to.push(parse_mailbox(e, n)?);
        Ok(())
    })
}

pub(super) fn email_cc_inner(args: &[Value]) -> Result<Value, String> {
    with_draft(args, "cc", |d, rest| {
        let (e, n) = two_strs(rest, "cc")?;
        d.cc.push(parse_mailbox(e, n)?);
        Ok(())
    })
}

pub(super) fn email_bcc_inner(args: &[Value]) -> Result<Value, String> {
    with_draft(args, "bcc", |d, rest| {
        let (e, n) = two_strs(rest, "bcc")?;
        d.bcc.push(parse_mailbox(e, n)?);
        Ok(())
    })
}

pub(super) fn email_subject_inner(args: &[Value]) -> Result<Value, String> {
    with_draft(args, "subject", |d, rest| match rest {
        [Value::Str(s)] => {
            d.subject = Some(s.as_str().to_string());
            Ok(())
        }
        _ => Err("Core.Mail.__subject expects (string)".into()),
    })
}

pub(super) fn email_text_inner(args: &[Value]) -> Result<Value, String> {
    with_draft(args, "text", |d, rest| match rest {
        [Value::Str(s)] => {
            d.text = Some(s.as_str().to_string());
            Ok(())
        }
        _ => Err("Core.Mail.__text expects (string)".into()),
    })
}

pub(super) fn email_html_inner(args: &[Value]) -> Result<Value, String> {
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

pub(super) fn email_attach_file_inner(args: &[Value]) -> Result<Value, String> {
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

pub(super) fn email_attach_bytes_inner(args: &[Value]) -> Result<Value, String> {
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

pub(super) fn email_attach_inline_bytes_inner(args: &[Value]) -> Result<Value, String> {
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

pub(super) fn email_attach_inline_inner(args: &[Value]) -> Result<Value, String> {
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
