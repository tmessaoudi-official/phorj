//! `Core.Fs` (W3) end-to-end fixture — typed filesystem surface on BOTH backends against a scratch
//! temp dir (the `tests/db.rs` pattern; `Core.FsSys` is impure → importing programs are quarantined
//! from the byte-identity differential).

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

fn scratch(tag: &str) -> String {
    let dir = std::env::temp_dir().join(format!("phorj-fs-it-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir.to_string_lossy().into_owned()
}

#[test]
fn fs_files_dirs_listings_and_walk_round_trip() {
    let root = scratch("main");
    let src = format!(
        r#"package Main;
import Core.Output;
import Core.Fs;
import Core.Fs.Fs;
import Core.Fs.FsError;
function main(): void {{
  try {{
    Fs.createDir("{root}/a/b");
    Fs.writeText("{root}/a/one.txt", "hello");
    Fs.appendText("{root}/a/one.txt", " world");
    Fs.writeText("{root}/a/b/two.txt", "deep");
    Output.printLine("read {{Fs.readText("{root}/a/one.txt")}}");
    Output.printLine("size {{Fs.size("{root}/a/one.txt")}}");
    Output.printLine("isFile {{Fs.isFile("{root}/a/one.txt")}} isDir {{Fs.isDir("{root}/a")}}");
    List<string> names = Fs.listDir("{root}/a");
    for (string n in names) {{ Output.printLine("entry {{n}}"); }}
    List<string> all = Fs.walk("{root}");
    for (string f in all) {{ Output.printLine("walk {{f}}"); }}
    Fs.copy("{root}/a/one.txt", "{root}/a/copy.txt");
    Fs.move("{root}/a/copy.txt", "{root}/a/moved.txt");
    Output.printLine("moved exists {{Fs.exists("{root}/a/moved.txt")}}");
    Fs.delete("{root}/a/moved.txt");
    Fs.removeDirAll("{root}");
    Output.printLine("cleaned {{Fs.exists("{root}")}}");
  }} catch (FsError e) {{ Output.printLine("unexpected: {{e.message}}"); }}
}}
"#
    );
    // Fs calls inside main's try need no `?` (try/catch context); listings are SORTED.
    both(
        &src,
        "read hello world\nsize 11\nisFile true isDir true\nentry b\nentry one.txt\nwalk a/b/two.txt\nwalk a/one.txt\nmoved exists true\ncleaned false\n",
    );
}

#[test]
fn fs_errors_are_typed_and_catchable() {
    let root = scratch("err");
    let src = format!(
        r#"package Main;
import Core.Output;
import Core.Fs;
import Core.Fs.Fs;
import Core.Fs.FsNotFound;
import Core.Fs.FsDirNotEmpty;
import Core.Fs.FsPermissionDenied;
import Core.Fs.FsError;
function main(): void {{
  try {{
    try {{
      discard Fs.readText("{root}/absent.txt");
      Output.printLine("unreachable");
    }} catch (FsNotFound e) {{
      Output.printLine("not-found");
    }}
    Fs.createDir("{root}/full");
    Fs.writeText("{root}/full/x.txt", "x");
    try {{
      Fs.removeDir("{root}/full");
      Output.printLine("unreachable");
    }} catch (FsDirNotEmpty e) {{
      Output.printLine("dir-not-empty");
    }}
    try {{
      Fs.removeDirAll("/");
      Output.printLine("unreachable");
    }} catch (FsPermissionDenied e) {{
      Output.printLine("root-refused");
    }}
    Fs.removeDirAll("{root}");
  }} catch (FsError e) {{ Output.printLine("unexpected: {{e.message}}"); }}
}}
"#
    );
    both(&src, "not-found\ndir-not-empty\nroot-refused\n");
}

/// THE LADDER RULE (for-now form): `Core.Fs` transpile is the clean E-TRANSPILE-FS (a real PHP
/// mapping is a recorded future lift).
#[test]
fn fs_transpile_is_a_clean_ladder_error() {
    let src = r#"package Main;
import Core.Output;
import Core.Fs;
function main(): void { Output.printLine("x"); }
"#;
    match cmd_transpile(src) {
        Ok(php) => panic!("expected E-TRANSPILE-FS, got PHP: {php:?}"),
        Err(e) => {
            assert!(e.contains("E-TRANSPILE-FS"), "{e}");
            assert!(!e.contains("E-UNKNOWN-IDENT"), "{e}");
        }
    }
}
