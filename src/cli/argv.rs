//! R9-G-1: the process argument vector as UTF-8 text, without `std::env::args()`'s panic. Arguments
//! are read as UTF-8 (every command, file name and program argument is a `String` downstream), so a
//! non-UTF-8 one is refused with one line on stderr and exit 1 — the no-crash contract (T-6) — never a
//! panic and never a lossy rewrite (mapping bytes to U+FFFD would name a different file, or hand the
//! program an argument it was not given). `argv[0]` is only a label (help text, the bare-file rewrite),
//! so it is read lossily: a `phg` reached through a non-UTF-8 path still runs.
use std::ffi::OsString;

/// `raw` as UTF-8 strings, or the index and value of the first argument after `argv[0]` that is not.
pub fn utf8_argv<I: IntoIterator<Item = OsString>>(
    raw: I,
) -> Result<Vec<String>, (usize, OsString)> {
    raw.into_iter()
        .enumerate()
        .map(|(i, a)| match i {
            0 => Ok(a.to_string_lossy().into_owned()),
            _ => a.into_string().map_err(|bad| (i, bad)),
        })
        .collect()
}

/// The whole argument vector (`argv[0]` included); a non-UTF-8 argument exits 1.
pub fn args_or_exit() -> Vec<String> {
    match utf8_argv(std::env::args_os()) {
        Ok(args) => args,
        Err((i, bad)) => {
            // `Debug` escapes the bytes (`"a\xFF.phg"`): deterministic, and no raw bytes on the terminal.
            eprintln!("error: argument {i} is not valid UTF-8: {bad:?} (arguments are read as UTF-8 text)");
            std::process::exit(1)
        }
    }
}

/// The arguments after the program name (a built binary's `Core.Process.args()`).
pub fn program_args_or_exit() -> Vec<String> {
    args_or_exit().into_iter().skip(1).collect()
}

#[cfg(test)]
mod tests {
    use super::utf8_argv;
    use std::ffi::OsString;

    #[cfg(unix)]
    fn os(b: &[u8]) -> OsString {
        use std::os::unix::ffi::OsStringExt;
        OsString::from_vec(b.to_vec())
    }

    #[cfg(unix)]
    #[test]
    fn the_first_non_utf8_argument_after_the_program_name_is_reported() {
        let got = utf8_argv([os(b"phg"), os(b"check"), os(b"a\xff"), os(b"\xfe")]);
        assert_eq!(got, Err((2, os(b"a\xff"))));
    }

    #[cfg(unix)]
    #[test]
    fn the_program_name_is_read_lossily_and_text_passes_through() {
        let got = utf8_argv([os(b"ph\xffg"), os("é".as_bytes()), os(b"")]);
        assert_eq!(
            got,
            Ok(vec![
                "ph\u{fffd}g".to_string(),
                "é".to_string(),
                String::new()
            ])
        );
        assert_eq!(utf8_argv(Vec::<OsString>::new()), Ok(Vec::new()));
    }
}
