//! Tests: source/argument resolution (`resolve_source`, `resolve_source_and_args`).
use super::super::*;
use super::rest;

#[test]
fn resolve_source_handles_all_forms() {
    assert_eq!(
        resolve_source(&rest(&["prog.phg"])),
        Some(SourceSpec::File("prog.phg".into()))
    );
    assert_eq!(resolve_source(&rest(&["-"])), Some(SourceSpec::Stdin));
    assert_eq!(
        resolve_source(&rest(&["-e", "x"])),
        Some(SourceSpec::Inline("x".into()))
    );
    assert_eq!(
        resolve_source(&rest(&["--eval", "y"])),
        Some(SourceSpec::Inline("y".into()))
    );
    // `--` lets a path start with '-'.
    assert_eq!(
        resolve_source(&rest(&["--", "-weird.phg"])),
        Some(SourceSpec::File("-weird.phg".into()))
    );
}

#[test]
fn resolve_source_and_args_splits_program_argv_on_terminator() {
    // `<file> -- a b c` → File + argv.
    assert_eq!(
        resolve_source_and_args(&rest(&["app.phg", "--", "a", "b"])),
        Some((SourceSpec::File("app.phg".into()), rest(&["a", "b"])))
    );
    // No `--` → empty argv.
    assert_eq!(
        resolve_source_and_args(&rest(&["app.phg"])),
        Some((SourceSpec::File("app.phg".into()), vec![]))
    );
    // `-e code -- args` and `- -- args`.
    assert_eq!(
        resolve_source_and_args(&rest(&["-e", "x", "--", "a"])),
        Some((SourceSpec::Inline("x".into()), rest(&["a"])))
    );
    assert_eq!(
        resolve_source_and_args(&rest(&["-", "--", "a", "b"])),
        Some((SourceSpec::Stdin, rest(&["a", "b"])))
    );
    // Leading `--` is the literal-path escape; a SECOND `--` then carries argv.
    assert_eq!(
        resolve_source_and_args(&rest(&["--", "-weird.phg"])),
        Some((SourceSpec::File("-weird.phg".into()), vec![]))
    );
    assert_eq!(
        resolve_source_and_args(&rest(&["--", "-weird.phg", "--", "a"])),
        Some((SourceSpec::File("-weird.phg".into()), rest(&["a"])))
    );
    // An empty argv after `--` is allowed (`app.phg --`).
    assert_eq!(
        resolve_source_and_args(&rest(&["app.phg", "--"])),
        Some((SourceSpec::File("app.phg".into()), vec![]))
    );
}

#[test]
fn resolve_source_rejects_bad_forms() {
    assert_eq!(resolve_source(&rest(&[])), None); // missing source
    assert_eq!(resolve_source(&rest(&["-e"])), None); // -e without code
    assert_eq!(resolve_source(&rest(&["-x"])), None); // unknown flag (use -- to pass it)
    assert_eq!(resolve_source(&rest(&["a", "b"])), None); // too many positionals
}
