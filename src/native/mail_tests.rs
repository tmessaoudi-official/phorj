//! `Core.Mail` native unit tests (split from `mail.rs` per the file-size cap, Invariant 13 —
//! the `src/native/*_tests.rs` sibling convention). Server-free and deterministic: MIME nesting,
//! the address-injection gate, plaintext derivation, transports.

use super::*;

fn draft_with(f: impl FnOnce(&mut Draft)) -> Draft {
    let mut d = Draft::default();
    f(&mut d);
    d
}

#[test]
fn smtp_tls_requires_tls_when_authenticated_unless_explicit_opt_out() {
    use SmtpTlsChoice::*;
    // No credentials (Mailpit/MailHog fakers) → opportunistic, any port/mode — nothing to protect.
    assert_eq!(smtp_tls_choice(false, false, "auto", 1025), Opportunistic);
    assert_eq!(smtp_tls_choice(false, false, "auto", 587), Opportunistic);
    // Authenticated + default (auto): implicit TLS on 465, STARTTLS-required elsewhere.
    assert_eq!(smtp_tls_choice(true, false, "auto", 465), Wrapper);
    assert_eq!(smtp_tls_choice(true, false, "auto", 587), Required);
    assert_eq!(smtp_tls_choice(true, false, "auto", 25), Required);
    // Authenticated + explicit modes override the port heuristic.
    assert_eq!(smtp_tls_choice(true, false, "starttls", 465), Required);
    assert_eq!(smtp_tls_choice(true, false, "implicit", 587), Wrapper);
    // The explicit, loud opt-out is the ONLY way authenticated plaintext can happen.
    assert_eq!(smtp_tls_choice(true, true, "auto", 587), Opportunistic);
    assert_eq!(smtp_tls_choice(true, true, "starttls", 587), Opportunistic);
    // THE DEC-265 INVARIANT: authenticated + not-opted-out is NEVER opportunistic (the pre-fix vuln).
    for port in [25u16, 465, 587, 2525] {
        for mode in ["auto", "starttls", "implicit"] {
            assert_ne!(
                smtp_tls_choice(true, false, mode, port),
                Opportunistic,
                "authenticated must never be downgradeable (port {port}, mode {mode})"
            );
        }
    }
}

fn mb(e: &str) -> Mailbox {
    parse_mailbox(e, "").unwrap()
}

#[test]
fn address_gate_rejects_injection_and_junk() {
    // Raw-header injection is structurally impossible: the CR/LF never parses as an address.
    assert!(parse_mailbox("a@b.c\r\nBcc: evil@x.y", "").is_err());
    assert!(parse_mailbox("not-an-address", "").is_err());
    let err = parse_mailbox("", "").unwrap_err();
    assert!(err.contains("<<InvalidAddress>>"), "{err}");
    // A display name with tricky characters is FOLDED by the MIME layer, never spliced raw.
    assert!(parse_mailbox("a@b.c", "Weird\r\nName").is_ok());
}

#[test]
fn build_requires_from_and_a_recipient() {
    let e = build_message(&Draft::default()).unwrap_err();
    assert!(
        e.contains("<<MessageBuildFailed>>") && e.contains("no `from`"),
        "{e}"
    );
    let d = draft_with(|d| d.from = Some(mb("a@b.c")));
    let e = build_message(&d).unwrap_err();
    assert!(e.contains("no recipients"), "{e}");
}

#[test]
fn html_body_derives_a_plaintext_alternative() {
    let d = draft_with(|d| {
        d.from = Some(mb("app@x.io"));
        d.to.push(mb("user@y.io"));
        d.subject = Some("Hi".into());
        d.html = Some("<h1>Hello</h1><p>Caf&eacute; &amp; friends</p>".into());
    });
    let msg = build_message(&d).unwrap();
    let raw = String::from_utf8_lossy(&msg.formatted()).to_string();
    assert!(
        raw.contains("multipart/alternative"),
        "no alternative part:\n{raw}"
    );
    assert!(raw.contains("text/plain"), "no derived plaintext:\n{raw}");
    assert!(raw.contains("text/html"), "no html part:\n{raw}");
}

#[test]
fn attachments_and_inline_cids_nest_correctly() {
    let d = draft_with(|d| {
        d.from = Some(mb("app@x.io"));
        d.to.push(mb("user@y.io"));
        d.html = Some("<img src=\"cid:logo\">".into());
        d.attachments.push(Att {
            cid: Some("logo".into()),
            filename: "logo.png".into(),
            mime: "image/png".into(),
            bytes: vec![1, 2, 3],
        });
        d.attachments.push(Att {
            cid: None,
            filename: "doc.pdf".into(),
            mime: "application/pdf".into(),
            bytes: vec![4, 5],
        });
    });
    let msg = build_message(&d).unwrap();
    let raw = String::from_utf8_lossy(&msg.formatted()).to_string();
    assert!(raw.contains("multipart/mixed"), "{raw}");
    assert!(raw.contains("multipart/related"), "{raw}");
    assert!(raw.contains("Content-ID"), "{raw}");
    assert!(raw.contains("doc.pdf"), "{raw}");
}

#[test]
fn html_to_text_strips_tags_decodes_entities_and_bullets() {
    assert_eq!(
        html_to_text("<h1>Hello</h1><p>a &amp; b</p><ul><li>x</li><li>y</li></ul>"),
        "Hello\na & b\n- x\n- y"
    );
    assert_eq!(html_to_text("no tags"), "no tags");
    assert_eq!(html_to_text("a<br>b"), "a\nb");
}

#[test]
fn file_transport_writes_numbered_eml() {
    let dir = std::env::temp_dir().join(format!("phorj-mail-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let m = file_transport_inner(&[Value::Str(dir.to_string_lossy().into_owned().into())]).unwrap();
    let mailer = as_mailer(&m).unwrap();
    let d = draft_with(|d| {
        d.from = Some(mb("a@b.c"));
        d.to.push(mb("u@y.io"));
        d.subject = Some("s".into());
        d.text = Some("body".into());
    });
    let msg = build_message(&d).unwrap();
    deliver(mailer, &msg).unwrap();
    deliver(mailer, &msg).unwrap();
    assert!(dir.join("phorj-mail-0.eml").exists());
    assert!(dir.join("phorj-mail-1.eml").exists());
    let raw = std::fs::read_to_string(dir.join("phorj-mail-0.eml")).unwrap();
    assert!(raw.contains("Subject: s"), "{raw}");
    let _ = std::fs::remove_dir_all(&dir);
}
