//! `Core.Database` — the SINGLE SOURCE of savepoint-control SQL (DEC-351, the D5 fold-in).
//!
//! **Why this file exists.** The savepoint SQL was written three times (the generic depth ops in
//! [`super::ops_tx`], plus each driver's bulk-insert guard) and the copies had DRIFTED into two
//! genuinely non-portable forms:
//!
//! - **bare `RELEASE <name>`** — legal in SQLite and Postgres, a **syntax error in MySQL**, where the
//!   keyword is mandatory (`RELEASE SAVEPOINT <name>`). `mysql.rs`'s own bulk path already spelled it
//!   correctly, so the module contradicted itself.
//! - **a `;`-joined PAIR** (`ROLLBACK TO x; RELEASE x`) — fine through SQLite's `execute_batch` and
//!   Postgres's `batch_execute`, but MySQL's `query_drop` runs ONE statement, so the same string is a
//!   syntax error there. The [`DriverConn::control`](super::driver::DriverConn::control) contract is
//!   single-statement; the pair violated it silently on the two backends that tolerate it.
//!
//! Both forms sat on the nested-savepoint path, which had ZERO MySQL/Postgres coverage — so nesting
//! `db.begin()` twice on MySQL would have failed at `commit`. Value kernels are single-sourced for
//! exactly this reason (Invariant 4); transaction-control SQL earns the same treatment.
//!
//! **The portable forms.** All three dialects accept these verbatim, as single statements:
//! `SAVEPOINT n` · `RELEASE SAVEPOINT n` · `ROLLBACK TO SAVEPOINT n`. The `SAVEPOINT` keyword is
//! OPTIONAL after `RELEASE`/`ROLLBACK TO` in SQLite and Postgres and MANDATORY in MySQL, so spelling it
//! always is the intersection — never the union.
//!
//! Rolling back to a savepoint does **not** pop it in any of the three, so a full unwind is genuinely
//! two statements (`ROLLBACK TO SAVEPOINT n` then `RELEASE SAVEPOINT n`), issued as two `control` calls.
//! The PHP leg mirrors this in `src/transpile/db_php.rs`, savepoint names included.

/// The savepoint name for transaction depth `depth` (the level being opened). Load-bearing: the PHP
/// leg emits the same names, so a database inspected mid-transaction looks the same on either leg.
pub(super) fn name(depth: u32) -> String {
    format!("phorj_sp_{depth}")
}

/// `SAVEPOINT n` — open a nested level.
pub(super) fn open(name: &str) -> String {
    format!("SAVEPOINT {name}")
}

/// `RELEASE SAVEPOINT n` — pop a level, KEEPING its work (the nested `commit`). The `SAVEPOINT` keyword
/// is what makes this MySQL-legal.
pub(super) fn release(name: &str) -> String {
    format!("RELEASE SAVEPOINT {name}")
}

/// `ROLLBACK TO SAVEPOINT n` — discard a level's work. Does NOT pop the savepoint on any backend, so
/// callers follow it with [`release`].
pub(super) fn rollback_to(name: &str) -> String {
    format!("ROLLBACK TO SAVEPOINT {name}")
}

/// The bulk-insert savepoint (`executeMany`'s own level, outside the depth counter).
pub(super) const BULK: &str = "phorj_bulk";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_portable_forms_are_exact() {
        assert_eq!(name(2), "phorj_sp_2");
        assert_eq!(open("phorj_sp_2"), "SAVEPOINT phorj_sp_2");
        // The `SAVEPOINT` keyword after RELEASE is MANDATORY in MySQL — bare `RELEASE x` is a syntax
        // error there, which is the bug this module was created to remove.
        assert_eq!(release("phorj_sp_2"), "RELEASE SAVEPOINT phorj_sp_2");
        assert_eq!(
            rollback_to("phorj_sp_2"),
            "ROLLBACK TO SAVEPOINT phorj_sp_2"
        );
    }

    #[test]
    fn every_form_is_a_single_statement() {
        // The `DriverConn::control` contract. MySQL's `query_drop` runs ONE statement, so a `;`-joined
        // pair is a syntax error there even though SQLite/Postgres batch APIs tolerate it.
        for sql in [
            open(BULK),
            release(BULK),
            rollback_to(BULK),
            open(&name(1)),
            release(&name(1)),
            rollback_to(&name(1)),
        ] {
            assert!(!sql.contains(';'), "not a single statement: {sql}");
        }
    }

    // ── The ratchet: no emitter anywhere may re-inline a non-portable form ──────────────────────

    /// Every source file that can emit transaction-control SQL: the whole `Core.Database` module (all
    /// three drivers + the generic ops) and the PHP leg's `__phorj_db_*` helpers.
    fn control_sql_sources() -> Vec<(String, String)> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut out = Vec::new();
        let db_dir = root.join("src/ext/database/natives");
        let mut paths: Vec<_> = std::fs::read_dir(&db_dir)
            .expect("read src/ext/database/natives")
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "rs"))
            .collect();
        paths.push(root.join("src/transpile/db_php.rs"));
        paths.sort();
        for p in paths {
            // This file itself is the vocabulary; its own doc comments quote the WRONG forms on purpose.
            if p.file_name().is_some_and(|n| n == "savepoint.rs") {
                continue;
            }
            let text = std::fs::read_to_string(&p).expect("read source");
            out.push((p.display().to_string(), text));
        }
        out
    }

    /// Is this line prose rather than code? Rust comments and the PHP helpers' `//` comments both.
    fn is_comment(line: &str) -> bool {
        line.trim_start().starts_with("//")
    }

    #[test]
    fn no_emitter_uses_a_bare_release() {
        for (path, text) in control_sql_sources() {
            for (i, line) in text.lines().enumerate() {
                if is_comment(line) || !line.contains("RELEASE ") {
                    continue;
                }
                assert!(
                    line.contains("RELEASE SAVEPOINT "),
                    "{path}:{} emits a bare `RELEASE` — a MySQL syntax error; use \
                     `savepoint::release` (`RELEASE SAVEPOINT n`):\n  {line}",
                    i + 1
                );
            }
        }
    }

    #[test]
    fn no_emitter_uses_a_bare_rollback_to() {
        for (path, text) in control_sql_sources() {
            for (i, line) in text.lines().enumerate() {
                if is_comment(line) || !line.contains("ROLLBACK TO ") {
                    continue;
                }
                assert!(
                    line.contains("ROLLBACK TO SAVEPOINT "),
                    "{path}:{} emits `ROLLBACK TO` without the `SAVEPOINT` keyword; use \
                     `savepoint::rollback_to`:\n  {line}",
                    i + 1
                );
            }
        }
    }

    #[test]
    fn no_emitter_joins_two_control_statements_with_a_semicolon() {
        // `; RELEASE …` / `; ROLLBACK …` inside one literal: the pair that MySQL's single-statement
        // `query_drop` rejects. A trailing Rust `;` is harmless — only a `;` FOLLOWED by a SQL verb
        // inside the same string is the defect.
        for (path, text) in control_sql_sources() {
            for (i, line) in text.lines().enumerate() {
                if is_comment(line) {
                    continue;
                }
                for verb in ["RELEASE", "ROLLBACK", "SAVEPOINT", "COMMIT", "BEGIN"] {
                    for joined in [format!("; {verb}"), format!(";{verb}")] {
                        assert!(
                            !line.contains(&joined),
                            "{path}:{} joins two control statements with `;`, which MySQL's \
                             single-statement `control` rejects — issue them as two calls:\n  {line}",
                            i + 1
                        );
                    }
                }
            }
        }
    }
}
