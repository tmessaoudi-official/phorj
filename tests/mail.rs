#![cfg(feature = "mail")]
//! `Core.Mail` (DEC-223) end-to-end fixture — the `tests/db.rs` pattern.
//!
//! The mailer natives are `pure:false` (network/filesystem delivery), so every `import Core.Mail`
//! program is auto-quarantined from the byte-identity differential; THIS file is the gate that runs
//! the surface on BOTH backends (`run ≡ runvm`) using the deterministic `file`/`null` transports.
//! The MIME internals (multipart nesting, auto-plaintext, CID inlines, address-injection rejection)
//! are unit-tested server-free in `src/native/mail.rs`. The LIVE SMTP round-trip is opt-in via
//! `PHORJ_MAILPIT_SMTP` (e.g. `localhost:1025` for a stack Mailpit) — skip-loudly when unset, so the
//! standard gate never needs a live server.

use phorj::cli::{cmd_run, cmd_transpile, cmd_treewalk};

fn both(src: &str, expected: &str) {
    let tree = cmd_treewalk(src).expect("program runs on the interpreter");
    assert_eq!(tree, expected, "interpreter output");
    assert_eq!(
        cmd_run(src).expect("program runs on the VM"),
        tree,
        "run ≡ runvm"
    );
}

/// A fresh scratch outbox dir per test (removed at start; the OS temp cleaner reaps leftovers).
fn outbox(tag: &str) -> String {
    let dir = std::env::temp_dir().join(format!("phorj-mail-it-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir.to_string_lossy().into_owned()
}

#[test]
fn mail_file_transport_round_trip_on_both_backends() {
    let dir = outbox("file");
    let src = format!(
        r#"package Main;
import Core.Output;
import Core.Mail;
import Core.Mail.Mailer;
import Core.Mail.Email;
import Core.Mail.Address;
import Core.Mail.FileTransport;
import Core.Mail.MailError;
#[Entry] function main(): void {{
  try {{
    Mailer m = new Mailer(new FileTransport("{dir}"));
    Email e = new Email()
      .from(new Address("app@example.io", "App"))
      .to(Address.of("user@example.org"))
      .cc(Address.of("obs@example.org"))
      .replyTo(Address.of("noreply@example.io"))
      .subject("Welcome")
      .html("<h1>Hi</h1><p>a &amp; b</p>");
    m.send(e);
    m.send(e);
    Output.printLine("sent twice");
  }} catch (MailError er) {{ Output.printLine("unexpected: {{er.message}}"); }}
}}
"#
    );
    both(&src, "sent twice\n");
    // Two numbered .eml files, RFC-shaped: headers + the alternative pair + the DERIVED plaintext.
    let raw = std::fs::read_to_string(std::path::Path::new(&dir).join("phorj-mail-0.eml"))
        .expect("first .eml written");
    assert!(std::path::Path::new(&dir).join("phorj-mail-1.eml").exists());
    for needle in [
        "From: App <app@example.io>",
        "To: user@example.org",
        "Cc: obs@example.org",
        "Reply-To: noreply@example.io",
        "Subject: Welcome",
        "multipart/alternative",
        "text/plain",
        "text/html",
    ] {
        assert!(raw.contains(needle), "missing {needle:?} in:\n{raw}");
    }
    // The derived plaintext decoded the entities and stripped the tags.
    assert!(raw.contains("a & b"), "derived plaintext missing:\n{raw}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn mail_null_transport_and_send_all_count() {
    let src = r#"package Main;
import Core.Output;
import Core.Mail;
import Core.Mail.Mailer;
import Core.Mail.Email;
import Core.Mail.Address;
import Core.Mail.NullTransport;
import Core.Mail.MailError;
#[Entry] function main(): void {
  try {
    Mailer m = new Mailer(new NullTransport());
    Email a = new Email().from(Address.of("x@y.io")).to(Address.of("u@y.io")).subject("a").text("1");
    Email b = new Email().from(Address.of("x@y.io")).to(Address.of("u@y.io")).subject("b").text("2");
    int n = m.sendAll([a, b]);
    Output.printLine("sent {n}");
  } catch (MailError e) { Output.printLine("unexpected: {e.message}"); }
}
"#;
    both(src, "sent 2\n");
}

#[test]
fn mail_invalid_address_is_typed_and_catchable_at_construction() {
    let src = r#"package Main;
import Core.Output;
import Core.Mail;
import Core.Mail.Address;
import Core.Mail.InvalidAddressError;
import Core.Mail.MailError;
#[Entry] function main(): void {
  try {
    Address bad = new Address("evil@x.y
Bcc: victim@z.w", "");
    Output.printLine("unreachable {bad.email}");
  } catch (InvalidAddressError e) {
    Output.printLine("invalid-address");
  } catch (MailError e) {
    Output.printLine("wrong-subtype: {e.message}");
  }
}
"#;
    both(src, "invalid-address\n");
}

#[test]
fn mail_missing_from_is_message_build_failed() {
    let src = r#"package Main;
import Core.Output;
import Core.Mail;
import Core.Mail.Mailer;
import Core.Mail.Email;
import Core.Mail.Address;
import Core.Mail.NullTransport;
import Core.Mail.MessageBuildFailedError;
import Core.Mail.MailError;
#[Entry] function main(): void {
  try {
    Mailer m = new Mailer(new NullTransport());
    Email e = new Email().to(Address.of("u@y.io")).subject("s").text("b");
    m.send(e);
    Output.printLine("unreachable");
  } catch (MessageBuildFailedError e) {
    Output.printLine("build-failed");
  } catch (MailError e) {
    Output.printLine("wrong-subtype: {e.message}");
  }
}
"#;
    both(src, "build-failed\n");
}

/// THE LADDER RULE: `Core.Mail` is native-only — transpile is the clean `E-TRANSPILE-MAIL`.
#[test]
fn mail_program_transpile_is_a_clean_ladder_error() {
    let src = r#"package Main;
import Core.Output;
import Core.Mail;
import Core.Mail.NullTransport;
#[Entry] function main(): void { Output.printLine("x"); }
"#;
    match cmd_transpile(src) {
        Ok(php) => panic!("expected E-TRANSPILE-MAIL, got PHP: {php:?}"),
        Err(e) => {
            assert!(e.contains("E-TRANSPILE-MAIL"), "{e}");
            assert!(!e.contains("E-UNKNOWN-IDENT"), "{e}");
        }
    }
}

/// LIVE SMTP round-trip — opt-in via `PHORJ_MAILPIT_SMTP=host:port` (a Mailpit/MailHog faker accepts
/// unauthenticated delivery). Skips loudly when unset.
#[test]
fn mail_smtp_round_trip_against_mailpit() {
    let Ok(hostport) = std::env::var("PHORJ_MAILPIT_SMTP") else {
        eprintln!(
            "mail: SKIP — set PHORJ_MAILPIT_SMTP=host:port (e.g. localhost:1025, a running Mailpit) \
             to run the live SMTP round-trip. The deterministic transports run in every gate."
        );
        return;
    };
    let (host, port) = hostport
        .split_once(':')
        .expect("PHORJ_MAILPIT_SMTP must be host:port");
    let src = format!(
        r#"package Main;
import Core.Output;
import Core.Mail;
import Core.Mail.Mailer;
import Core.Mail.Email;
import Core.Mail.Address;
import Core.Mail.SmtpConfig;
import Core.Mail.MailError;
#[Entry] function main(): void {{
  try {{
    Mailer m = new Mailer(new SmtpConfig("{host}", {port}));
    Email e = new Email()
      .from(new Address("app@example.io", "App"))
      .to(Address.of("user@example.org"))
      .subject("phorj live test")
      .html("<p>hello from phorj</p>");
    m.send(e);
    Output.printLine("smtp sent");
  }} catch (MailError er) {{ Output.printLine("unexpected: {{er.message}}"); }}
}}
"#
    );
    both(&src, "smtp sent\n");
}
