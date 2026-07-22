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

/// DEC-313 (2026-07-22): the FS quarantine is LIFTED — `Core.FileSystemModule` transpiles through the
/// gated `__phorj_fs_*` helpers. This is the inverted ladder test: transpile must SUCCEED and emit
/// the helper defs, and (when php is present — same gating as tests/conformance.rs) the transpiled
/// program's stdout must match the backends byte-for-byte, INCLUDING the typed-error kinds
/// (`<<Kind>>` markers are the byte-identity contract; the message tail is out-of-contract).
#[test]
fn fs_transpiles_and_matches_the_backends_on_php() {
    let src = r#"package Main;
import Core.Runtime.Entry;
import Core.Output;
import Core.FileSystemModule;
#[Entry] function main(): void { Output.printLine("x"); }
"#;
    let php_src = cmd_transpile(src).expect("FS import transpiles (DEC-313)");
    assert!(
        php_src.contains("__phorj_fs_read_text"),
        "gated FS helpers present"
    );

    // Content parity: the pinned-kind error program + the happy round-trip, on a real php.
    let Some(php) = php_bin() else {
        eprintln!("SKIP fs php leg: php not found — set PHORJ_REQUIRE_PHP=1 to require it");
        assert!(
            std::env::var("PHORJ_REQUIRE_PHP").as_deref() != Ok("1"),
            "php required but not found"
        );
        return;
    };
    let root = scratch("php");
    let prog = format!(
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
    }} catch (FileSystemNotFoundError e) {{ Output.printLine("not-found"); }}
    FileSystem.createDir("{root}/full");
    FileSystem.writeText("{root}/full/x.txt", "hello world");
    Output.printLine("read {{FileSystem.readText("{root}/full/x.txt")}}");
    List<string> all = FileSystem.walk("{root}");
    for (string f in all) {{ Output.printLine("walk {{f}}"); }}
    try {{
      FileSystem.removeDir("{root}/full");
      Output.printLine("unreachable");
    }} catch (FileSystemDirNotEmptyError e) {{ Output.printLine("dir-not-empty"); }}
    try {{
      FileSystem.removeDirAll("/");
      Output.printLine("unreachable");
    }} catch (FileSystemPermissionDeniedError e) {{ Output.printLine("root-refused"); }}
    FileSystem.removeDirAll("{root}");
    Output.printLine("cleaned {{FileSystem.exists("{root}")}}");
  }} catch (FileSystemError e) {{ Output.printLine("unexpected: {{e.message}}"); }}
}}
"#
    );
    let expected =
        "not-found\nread hello world\nwalk full/x.txt\ndir-not-empty\nroot-refused\ncleaned false\n";
    both(&prog, expected);
    // Fresh scratch for the php leg (the backends already consumed + removed theirs).
    let code = cmd_transpile(&prog).expect("error program transpiles");
    let dir = std::env::temp_dir().join(format!("phorj-fs-phpleg-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let php_file = dir.join("prog.php");
    std::fs::write(&php_file, &code).unwrap();
    let out = std::process::Command::new(&php)
        .arg(&php_file)
        .output()
        .expect("php runs");
    assert!(
        out.status.success(),
        "php leg failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        expected,
        "php content parity"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

fn php_bin() -> Option<String> {
    if std::env::var("PHORJ_SKIP_PHP").as_deref() == Ok("1") {
        return None;
    }
    let cand = std::env::var("PHORJ_PHP").unwrap_or_else(|_| "php".to_string());
    let ok = std::process::Command::new(&cand)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    ok.then_some(cand)
}
