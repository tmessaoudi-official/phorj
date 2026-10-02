//! `Core.Cryptography` — password hashing (Argon2id). The FIRST module backed by an external crate
//! (RustCrypto `argon2`), admitted under `docs/specs/2026-06-27-dependency-policy.md` (the
//! audited-crypto-only exception to `std`-only). Rationale: secure password hashing demands a *vetted*
//! implementation ("never roll your own crypto"), `std` ships none, and the capability must be NATIVE
//! to Phorj's Rust backends — never delegated to the PHP transpile target. The `php` closures emit
//! `password_hash`/`password_verify` as a **peer** target; because both sides speak the standard PHC
//! string (`$argon2id$…`), a hash made by either backend verifies in the other.
//!
//! These natives are **`pure: false`** where non-deterministic: `hashPassword` uses a random salt, so
//! it is quarantined from the byte-identity oracle (tested in `tests/crypto.rs`). `verifyPassword` is
//! deterministic for a fixed `(password, hash)` pair, so it CAN appear in a byte-identity-gated
//! example (against a committed PHC hash).

use crate::native::*;
use crate::types::Ty;
use crate::value::Value;
use argon2::password_hash::{phc::PasswordHash, PasswordHasher, PasswordVerifier};
use argon2::Argon2;

/// `Crypto.hashPassword(string) -> string` — Argon2id over a fresh random salt; returns the standard
/// PHC string. Non-deterministic (`pure: false`). PHP: `password_hash($pw, PASSWORD_ARGON2ID)`.
pub(super) fn crypto_hash_password(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(pw)] => {
            // argon2 0.6 draws the salt itself, from the OS RNG (`getrandom`, a default feature).
            let hash = Argon2::default()
                .hash_password(pw.as_bytes())
                .map_err(|e| format!("password hashing failed: {e}"))?
                .to_string();
            Ok(Value::Str(hash.into()))
        }
        _ => Err("Crypto.hashPassword expects (string)".into()),
    }
}

/// Is `hash` a bcrypt crypt string (`$2a$`, `$2b$`, `$2x$`, `$2y$`)? Mirrored by the PHP guard in
/// `__phorj_verify_password` (`/^\$2[abxy]\$/`).
fn is_bcrypt_hash(hash: &str) -> bool {
    let b = hash.as_bytes();
    b.len() >= 4
        && b[0] == b'$'
        && b[1] == b'2'
        && matches!(b[2], b'a' | b'b' | b'x' | b'y')
        && b[3] == b'$'
}

/// `Crypto.verifyPassword(string password, string hash) -> bool` — constant-time verify against a PHC
/// hash. A malformed hash string is `false` (mirrors PHP `password_verify`); a bcrypt hash FAULTS (DEC-561).
/// Deterministic. PHP: `password_verify($pw, $hash)`.
pub(super) fn crypto_verify_password(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(pw), Value::Str(hash)] => {
            // DEC-561: bcrypt (`$2a$`/`$2b$`/`$2x$`/`$2y$` — PHP's `password_hash` default) can never
            // verify here, so `false` would be a silent wrong-password lockout. Fault, naming the cause.
            if is_bcrypt_hash(hash) {
                return Err(
                    "Cryptography.verifyPassword: unsupported hash algorithm (bcrypt) — only argon2 hashes verify"
                        .into(),
                );
            }
            let parsed = match PasswordHash::new(hash) {
                Ok(p) => p,
                Err(_) => return Ok(Value::Bool(false)),
            };
            Ok(Value::Bool(
                Argon2::default()
                    .verify_password(pw.as_bytes(), &parsed)
                    .is_ok(),
            ))
        }
        _ => Err("Crypto.verifyPassword expects (string, string)".into()),
    }
}

pub fn cryptography_natives() -> Vec<NativeFn> {
    vec![
        NativeFn {
            module: "Core.Cryptography",
            name: "hashPassword",
            params: vec![Ty::String],
            ret: Ty::String,
            pure: false, // random salt → quarantined from the oracle
            eval: NativeEval::Pure(crypto_hash_password),
            lift_from: &[],
            php: |a| format!("password_hash({}, PASSWORD_ARGON2ID)", parg(a, 0)),
        },
        NativeFn {
            module: "Core.Cryptography",
            name: "verifyPassword",
            params: vec![Ty::String, Ty::String],
            ret: Ty::Bool,
            pure: true, // deterministic for a fixed (password, hash) → gateable
            eval: NativeEval::Pure(crypto_verify_password),
            lift_from: &["password_verify"],
            php: |a| format!("__phorj_verify_password({}, {})", parg(a, 0), parg(a, 1)),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn verify(pw: &str, hash: &str) -> Result<Value, String> {
        crypto_verify_password(
            &[Value::Str(pw.into()), Value::Str(hash.into())],
            &mut String::new(),
        )
    }

    /// DEC-561: a bcrypt hash (PHP's `password_hash` default) can never verify here — only `argon2` is
    /// admitted — so answering `false` was a silent wrong-password lockout. It faults, naming the cause.
    #[test]
    fn a_bcrypt_hash_faults_instead_of_silently_failing() {
        for prefix in ["$2y$", "$2a$", "$2b$", "$2x$"] {
            let hash = format!("{prefix}10$abcdefghijklmnopqrstuuABCDEFGHIJKLMNOPQRSTUVWXYZ01234");
            let e = verify("secret", &hash).expect_err(prefix);
            assert!(e.contains("bcrypt"), "{prefix}: {e}");
        }
    }

    #[test]
    fn other_malformed_hashes_stay_false_and_argon2_still_verifies() {
        for bad in [
            "",
            "plain",
            "$2$short",
            "$1$md5$hash",
            "$2y",
            "$argon2id$garbage",
        ] {
            assert!(
                matches!(verify("x", bad), Ok(Value::Bool(false))),
                "{bad:?}"
            );
        }
        let Ok(Value::Str(h)) =
            crypto_hash_password(&[Value::Str("secret".into())], &mut String::new())
        else {
            panic!("hash");
        };
        assert!(matches!(verify("secret", &h), Ok(Value::Bool(true))));
        assert!(matches!(verify("wrong", &h), Ok(Value::Bool(false))));
    }
}
