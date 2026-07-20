//! `Core.Mail` MIME layer — injection-safe address parsing, HTML→plaintext derivation, RFC-correct
//! `Message` composition, and the SMTP error/TLS-posture classifiers (split per Invariant 13).

use super::handles::{Att, Draft};
use lettre::message::header::ContentType;
use lettre::message::{Attachment as MimeAttachment, Mailbox, MultiPart, SinglePart};
use lettre::Message;

// ── Address parsing (typed, injection-safe by construction) ─────────────────────────────────────────

/// Parse an address (+ optional display name) into a lettre [`Mailbox`]. This is the ONE gate every
/// recipient/sender passes, so raw-header injection (`"a@b\r\nBcc: attacker"`) is structurally
/// impossible — lettre rejects control characters and folds the display name per RFC 2047.
pub(super) fn parse_mailbox(email: &str, name: &str) -> Result<Mailbox, String> {
    let addr: lettre::Address = email
        .parse()
        .map_err(|e| format!("<<InvalidAddressError>>Core.Mail: invalid address `{email}`: {e}"))?;
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
pub(super) fn html_to_text(html: &str) -> String {
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
pub(super) fn build_message(draft: &Draft) -> Result<Message, String> {
    let err = |m: String| format!("<<MessageBuildFailedError>>Core.Mail: {m}");
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
pub(super) fn smtp_err_kind(e: &lettre::transport::smtp::Error) -> &'static str {
    if let Some(code) = e.status() {
        let n = u16::from(code.severity as u8) * 100
            + u16::from(code.category as u8) * 10
            + u16::from(code.detail as u8);
        return match n {
            535 | 534 | 530 => "AuthFailedError",
            550..=554 => "RecipientRejectedError",
            _ => "ConnectionFailedError",
        };
    }
    let text = e.to_string();
    if text.contains("tls") || text.contains("TLS") {
        "TlsError"
    } else if text.contains("timed out") || text.contains("timeout") {
        "TimeoutError"
    } else {
        "ConnectionFailedError"
    }
}

// ── SMTP TLS posture (DEC-265, factored PURE for unit tests) ───────────────────────────────────────────

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
pub(super) enum SmtpTlsChoice {
    Wrapper,
    Required,
    Opportunistic,
}

pub(super) fn smtp_tls_choice(
    has_creds: bool,
    allow_insecure: bool,
    mode: &str,
    port: u16,
) -> SmtpTlsChoice {
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
