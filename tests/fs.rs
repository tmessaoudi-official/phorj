//! `Core.FileSystemModule` (W3) end-to-end fixture — typed filesystem surface on BOTH backends against a scratch
//! temp dir (the `tests/database.rs` pattern; `Core.Native.FileSystem` is impure → importing programs are quarantined
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
import Core.Runtime.Entry;
import Core.Output;
import Core.FileSystemModule;
import Core.FileSystemModule.FileSystem;
import Core.FileSystemModule.FileSystemError;
#[Entry] function main(): void {{
  try {{
    FileSystem.createDir("{root}/a/b");
    FileSystem.writeText("{root}/a/one.txt", "hello");
    FileSystem.appendText("{root}/a/one.txt", " world");
    FileSystem.writeText("{root}/a/b/two.txt", "deep");
    Output.printLine("read {{FileSystem.readText("{root}/a/one.txt")}}");
    Output.printLine("size {{FileSystem.size("{root}/a/one.txt")}}");
    Output.printLine("isFile {{FileSystem.isFile("{root}/a/one.txt")}} isDir {{FileSystem.isDir("{root}/a")}}");
    List<string> names = FileSystem.listDir("{root}/a");
    for (string n in names) {{ Output.printLine("entry {{n}}"); }}
    List<string> all = FileSystem.walk("{root}");
    for (string f in all) {{ Output.printLine("walk {{f}}"); }}
    FileSystem.copy("{root}/a/one.txt", "{root}/a/copy.txt");
    FileSystem.move("{root}/a/copy.txt", "{root}/a/moved.txt");
    Output.printLine("moved exists {{FileSystem.exists("{root}/a/moved.txt")}}");
    FileSystem.delete("{root}/a/moved.txt");
    FileSystem.removeDirAll("{root}");
    Output.printLine("cleaned {{FileSystem.exists("{root}")}}");
  }} catch (FileSystemError e) {{ Output.printLine("unexpected: {{e.message}}"); }}
}}
"#
    );
    // FileSystem calls inside main's try need no `?` (try/catch context); listings are SORTED.
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
import Core.Runtime.Entry;
import Core.Output;
import Core.FileSystemModule;
import Core.FileSystemModule.FileSystem;
import Core.FileSystemModule.FileSystemNotFoundError;
import Core.FileSystemModule.FileSystemDirNotEmptyError;
import Core.FileSystemModule.FileSystemPermissionDeniedError;
import Core.FileSystemModule.FileSystemError;
#[Entry] function main(): void {{
  try {{
    try {{
      discard FileSystem.readText("{root}/absent.txt");
      Output.printLine("unreachable");
    }} catch (FileSystemNotFoundError e) {{
      Output.printLine("not-found");
    }}
    FileSystem.createDir("{root}/full");
    FileSystem.writeText("{root}/full/x.txt", "x");
    try {{
      FileSystem.removeDir("{root}/full");
      Output.printLine("unreachable");
    }} catch (FileSystemDirNotEmptyError e) {{
      Output.printLine("dir-not-empty");
    }}
    try {{
      FileSystem.removeDirAll("/");
      Output.printLine("unreachable");
    }} catch (FileSystemPermissionDeniedError e) {{
      Output.printLine("root-refused");
    }}
    FileSystem.removeDirAll("{root}");
  }} catch (FileSystemError e) {{ Output.printLine("unexpected: {{e.message}}"); }}
}}
"#
    );
    both(&src, "not-found\ndir-not-empty\nroot-refused\n");
}

/// THE LADDER RULE (for-now form): `Core.FileSystemModule` transpile is the clean E-TRANSPILE-FS (a real PHP
/// mapping is a recorded future lift).
#[test]
fn fs_transpile_is_a_clean_ladder_error() {
    let src = r#"package Main;
import Core.Runtime.Entry;
import Core.Output;
import Core.FileSystemModule;
#[Entry] function main(): void { Output.printLine("x"); }
"#;
    match cmd_transpile(src) {
        Ok(php) => panic!("expected E-TRANSPILE-FS, got PHP: {php:?}"),
        Err(e) => {
            assert!(e.contains("E-TRANSPILE-FS"), "{e}");
            assert!(!e.contains("E-UNKNOWN-IDENT"), "{e}");
        }
    }
}
