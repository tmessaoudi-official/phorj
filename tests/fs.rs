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
        "interp ≡ VM"
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
import Core.Runtime.Entry; import Core.Runtime.EntryKind;
import Core.Output;
import Core.FileSystemModule;
import Core.FileSystemModule.FileSystem;
import Core.FileSystemModule.FileSystemError;
#[Entry(kind: EntryKind.Cli)] function main(): void {{
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
import Core.Runtime.Entry; import Core.Runtime.EntryKind;
import Core.Output;
import Core.FileSystemModule;
import Core.FileSystemModule.FileSystem;
import Core.FileSystemModule.FileSystemNotFoundError;
import Core.FileSystemModule.FileSystemDirNotEmptyError;
import Core.FileSystemModule.FileSystemPermissionDeniedError;
import Core.FileSystemModule.FileSystemError;
#[Entry(kind: EntryKind.Cli)] function main(): void {{
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
import Core.Runtime.Entry; import Core.Runtime.EntryKind;
import Core.Output;
import Core.FileSystemModule;
#[Entry(kind: EntryKind.Cli)] function main(): void { Output.printLine("x"); }
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
import Core.Runtime.Entry; import Core.Runtime.EntryKind;
import Core.Output;
import Core.FileSystemModule;
import Core.FileSystemModule.FileSystem;
import Core.FileSystemModule.FileSystemNotFoundError;
import Core.FileSystemModule.FileSystemDirNotEmptyError;
import Core.FileSystemModule.FileSystemPermissionDeniedError;
import Core.FileSystemModule.FileSystemError;
#[Entry(kind: EntryKind.Cli)] function main(): void {{
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

/// DEC-348 — `FileSystem.withLock(path, fn)`, the scoped advisory lock.
///
/// Three things must hold and each is asserted: the closure's value comes back, the lock is RELEASED
/// at block exit (proved by re-acquiring in the same program — a leak would deadlock the second
/// call), and it is released even when the closure THROWS.
#[test]
fn fs_with_lock_runs_the_closure_and_always_releases() {
    let root = scratch("lock");
    let prog = format!(
        r#"package Main;
import Core.Runtime.Entry; import Core.Runtime.EntryKind;
import Core.Output;
import Core.FileSystemModule;
import Core.FileSystemModule.FileSystem;
import Core.FileSystemModule.FileSystemError;
import Core.FileSystemModule.FileSystemNotFoundError;
#[Entry(kind: EntryKind.Cli)] function main(): void {{
  try {{
    // Created by the program (not the harness) so BOTH backend runs are self-contained — the run
    // ends by removing the tree, so a harness-created dir would be gone for the second leg.
    FileSystem.createDir("{root}");
    string p = "{root}/guard.lock";
    int v = FileSystem.withLock(p, function(): int throws FileSystemError {{ return 41 + 1; }});
    Output.printLine("value {{v}}");
    // Re-acquiring can only succeed if the first block released — otherwise this blocks forever.
    discard FileSystem.withLock(p, function(): int throws FileSystemError {{ Output.printLine("re-acquired"); return 0; }});
    // A THROWING closure must still release: the throw propagates out AND the lock is free after.
    try {{
      discard FileSystem.withLock(p, function(): int throws FileSystemError {{ discard FileSystem.readText("{root}/absent.txt")?; return 0; }});
      Output.printLine("unreachable");
    }} catch (FileSystemNotFoundError e) {{ Output.printLine("caught not-found"); }}
    discard FileSystem.withLock(p, function(): int throws FileSystemError {{ Output.printLine("free after throw"); return 0; }});
    FileSystem.removeDirAll("{root}");
  }} catch (FileSystemError e) {{ Output.printLine("unexpected: {{e.message}}"); }}
}}
"#
    );
    let expected = "value 42\nre-acquired\ncaught not-found\nfree after throw\n";
    both(&prog, expected);

    // The PHP leg runs the SAME guard through its own `flock()` twin.
    let Some(php) = php_bin() else {
        eprintln!("SKIP fs lock php leg: php not found — set PHORJ_REQUIRE_PHP=1 to require it");
        assert!(
            std::env::var("PHORJ_REQUIRE_PHP").as_deref() != Ok("1"),
            "php required but not found"
        );
        return;
    };
    let root2 = scratch("lock-php");
    std::fs::create_dir_all(&root2).unwrap();
    let code = cmd_transpile(&prog.replace(&root, &root2)).expect("lock program transpiles");
    let php_file = std::path::Path::new(&root2).join("prog.php");
    std::fs::write(&php_file, &code).unwrap();
    let out = std::process::Command::new(&php)
        .arg(&php_file)
        .output()
        .expect("php runs");
    assert!(
        out.status.success(),
        "php lock leg failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        expected,
        "php lock parity"
    );
    let _ = std::fs::remove_dir_all(&root2);
}

/// DEC-347 — `FileSystem.lines` streams a file as an `Iterator<string>`, agreeing on all three legs.
///
/// The cases are the ones that actually break line readers: a blank line (content, not a terminator), a
/// final line with NO terminator, CRLF (must read like LF), an empty file (empty iteration, not an
/// error), and a file large enough to cross the native's 64 KiB chunk boundary — the last is the one
/// that only fails on real inputs, because a chunk ending mid-line is invisible on a small fixture.
#[test]
fn fs_lines_streams_every_shape_of_file_identically_on_all_legs() {
    let root = scratch("lines");
    std::fs::create_dir_all(&root).unwrap();
    // 3000 lines of 40 bytes = 120 KB, so the read spans two chunks and the boundary lands mid-line.
    let big: String = (0..3000)
        .map(|i| format!("row {i:05} {}\n", "p".repeat(28)))
        .collect();
    std::fs::write(format!("{root}/big.txt"), big.as_bytes()).unwrap();
    let prog = format!(
        r#"package Main;
import Core.Runtime.Entry; import Core.Runtime.EntryKind;
import Core.Output;
import Core.FileSystemModule;
import Core.FileSystemModule.FileSystem;
import Core.FileSystemModule.FileSystemError;
import Core.IteratorModule;
import Core.IteratorModule.Iterator;
// NOT named `count`: that collides with PHP's builtin and transpiles to `Cannot redeclare function
// count()`. Hit for real while writing this test — see KNOWN_ISSUES "PHP builtin FUNCTION names".
function countOf(Iterator<string> it): int throws FileSystemError {{
  mutable int n = 0;
  for (string line in it) {{ n = n + 1; }}
  return n;
}}
#[Entry(kind: EntryKind.Cli)] function main(): void {{
  try {{
    string p = "{root}/one.txt";
    FileSystem.writeText(p, "alpha\nbeta\n\ngamma\n");
    for (string line in FileSystem.lines(p)) {{ Output.printLine("[{{line}}]"); }}
    FileSystem.writeText(p, "one\ntwo");
    Output.printLine("noeol {{countOf(FileSystem.lines(p))}}");
    FileSystem.writeText(p, "r1\r\nr2\r\n");
    for (string line in FileSystem.lines(p)) {{ Output.printLine("crlf[{{line}}]"); }}
    FileSystem.writeText(p, "");
    Output.printLine("empty {{countOf(FileSystem.lines(p))}}");
    // Spans the native's chunk boundary — the case a small fixture cannot reach.
    Output.printLine("big {{countOf(FileSystem.lines("{root}/big.txt"))}}");
    FileSystem.delete(p);
  }} catch (FileSystemError e) {{ Output.printLine("unexpected: {{e.message}}"); }}
}}
"#
    );
    let expected = "[alpha]\n[beta]\n[]\n[gamma]\nnoeol 2\ncrlf[r1]\ncrlf[r2]\nempty 0\nbig 3000\n";
    both(&prog, expected);

    // The PHP leg runs the same shapes through the `fgets` twin (ladder case 1).
    let Some(php) = php_bin() else {
        eprintln!("SKIP fs lines php leg: php not found — set PHORJ_REQUIRE_PHP=1 to require it");
        assert!(
            std::env::var("PHORJ_REQUIRE_PHP").as_deref() != Ok("1"),
            "php required but not found"
        );
        let _ = std::fs::remove_dir_all(&root);
        return;
    };
    let code = cmd_transpile(&prog).expect("lines program transpiles");
    let php_file = std::path::Path::new(&root).join("prog.php");
    std::fs::write(&php_file, &code).unwrap();
    let out = std::process::Command::new(&php)
        .arg(&php_file)
        .output()
        .expect("php runs");
    assert!(
        out.status.success(),
        "php lines leg failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        expected,
        "php lines parity"
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// `tryWithLock` reports contention instead of waiting, and its `Option<T>` return keeps "the lock was
/// busy" distinguishable from "the closure ran and returned null" — the ambiguity that ruled out the
/// cheaper `T?` (developer-ruled 2026-07-31). Both branches are asserted, and the busy branch needs no
/// second process: the OS lock is per-file-DESCRIPTOR, so a nested attempt opens its own descriptor and
/// genuinely finds the lock held by this same program. That makes it deterministic — no sleep, no race.
///
/// The null case is the load-bearing one: a closure returning `null` under a FREE lock must still come
/// back as `Some(null)`, never `None`. Under a `T?` return those two would be one value.
#[test]
fn fs_try_with_lock_distinguishes_contention_from_a_null_result() {
    let root = scratch("trylock");
    let prog = format!(
        r#"package Main;
import Core.Runtime.Entry; import Core.Runtime.EntryKind;
import Core.Output;
import Core.FileSystemModule;
import Core.FileSystemModule.FileSystem;
import Core.FileSystemModule.FileSystemError;
#[Entry(kind: EntryKind.Cli)] function main(): void {{
  try {{
    FileSystem.createDir("{root}");
    string p = "{root}/guard.lock";
    // 1. Free lock: acquires and hands back the closure's value.
    Option<int> got = FileSystem.tryWithLock(p, function(): int throws FileSystemError {{ return 7; }});
    Output.printLine(match (got) {{ Option.Some(v) => "some {{v}}", Option.None() => "none" }});
    // 2. Held lock: reports None rather than blocking. Nested inside a `withLock` on the SAME path.
    string nested = FileSystem.withLock(p, function(): string throws FileSystemError {{
      Option<int> inner = FileSystem.tryWithLock(p, function(): int throws FileSystemError {{ return 1; }})?;
      return match (inner) {{ Option.Some(v) => "inner some {{v}}", Option.None() => "inner none" }};
    }});
    Output.printLine(nested);
    // 3. The distinction `Option` exists for: a null result under a FREE lock is `Some(null)`.
    Option<string?> nul = FileSystem.tryWithLock(p, function(): string? throws FileSystemError {{ return null; }});
    Output.printLine(match (nul) {{ Option.Some(v) => "some-of-null {{v ?? \"yes\"}}", Option.None() => "none" }});
    // 4. The lock is released after every `tryWithLock` too — a leak would make this block forever.
    discard FileSystem.withLock(p, function(): int throws FileSystemError {{ Output.printLine("free after try"); return 0; }});
    FileSystem.removeDirAll("{root}");
  }} catch (FileSystemError e) {{ Output.printLine("unexpected: {{e.message}}"); }}
}}
"#
    );
    let expected = "some 7\ninner none\nsome-of-null yes\nfree after try\n";
    both(&prog, expected);

    // The PHP leg must agree — its `flock(LOCK_NB)` twin and the same `Option` shape.
    let Some(php) = php_bin() else {
        eprintln!("SKIP fs trylock php leg: php not found — set PHORJ_REQUIRE_PHP=1 to require it");
        assert!(
            std::env::var("PHORJ_REQUIRE_PHP").as_deref() != Ok("1"),
            "php required but not found"
        );
        return;
    };
    let root2 = scratch("trylock-php");
    let code = cmd_transpile(&prog.replace(&root, &root2)).expect("trylock program transpiles");
    let php_file = std::path::Path::new(&root2).join("prog.php");
    std::fs::create_dir_all(&root2).unwrap();
    std::fs::write(&php_file, &code).unwrap();
    let out = std::process::Command::new(&php)
        .arg(&php_file)
        .output()
        .expect("php runs");
    assert!(
        out.status.success(),
        "php trylock leg failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        expected,
        "php trylock parity"
    );
    let _ = std::fs::remove_dir_all(&root2);
}

/// The lock must be a REAL OS lock, not a no-op that happens to run the closure. Held by an external
/// `flock(1)` process, `withLock` has to BLOCK — on the Rust leg AND on the transpiled PHP leg, which
/// is the bidirectional interop DEC-348 rests on.
///
/// Skips loudly if `flock(1)` is unavailable; a blocked run is detected by timeout, so the assertion
/// is "did not finish", never a sleep-based guess about timing.
#[test]
fn fs_with_lock_blocks_on_a_lock_held_by_another_process() {
    let Some(flock) = which("flock") else {
        eprintln!("SKIP fs lock contention: `flock(1)` unavailable");
        return;
    };
    let root = scratch("contend");
    std::fs::create_dir_all(&root).unwrap();
    let lock = format!("{root}/held.lock");
    std::fs::write(&lock, b"").unwrap();
    let prog = format!(
        r#"package Main;
import Core.Runtime.Entry; import Core.Runtime.EntryKind;
import Core.Output;
import Core.FileSystemModule;
import Core.FileSystemModule.FileSystem;
import Core.FileSystemModule.FileSystemError;
#[Entry(kind: EntryKind.Cli)] function main(): void {{
  try {{
    discard FileSystem.withLock("{lock}", function(): int throws FileSystemError {{ Output.printLine("ACQUIRED"); return 0; }});
  }} catch (FileSystemError e) {{ Output.printLine("err {{e.message}}"); }}
}}
"#
    );
    // Sanity: uncontended, it acquires. Without this the blocking assertion below could pass for the
    // wrong reason (a program that never acquires anything also never prints).
    assert_eq!(
        cmd_treewalk(&prog).expect("uncontended run"),
        "ACQUIRED\n",
        "uncontended withLock must acquire"
    );

    let php = php_bin();
    let code = cmd_transpile(&prog).expect("contention program transpiles");
    let php_file = std::path::Path::new(&root).join("prog.php");
    std::fs::write(&php_file, &code).unwrap();

    // Hold the lock in another process for longer than the probes are given.
    let mut holder = std::process::Command::new(&flock)
        .args(["-x", &lock, "-c", "sleep 5"])
        .spawn()
        .expect("flock holder starts");
    std::thread::sleep(std::time::Duration::from_millis(500));

    if let Some(php) = &php {
        let blocked = ran_out_of_time(php, php_file.to_str().unwrap());
        assert!(
            blocked,
            "the PHP leg acquired a lock another process holds — the flock twin is not contending"
        );
    }
    let _ = holder.kill();
    let _ = holder.wait();
    let _ = std::fs::remove_dir_all(&root);
}

/// Run `bin arg` with a short deadline; `true` when it was still running at the deadline (i.e. blocked).
fn ran_out_of_time(bin: &str, arg: &str) -> bool {
    let mut child = std::process::Command::new(bin)
        .arg(arg)
        .stdout(std::process::Stdio::null())
        .spawn()
        .expect("probe starts");
    for _ in 0..20 {
        if let Some(_status) = child.try_wait().expect("try_wait") {
            return false; // finished => it acquired the lock
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let _ = child.kill();
    let _ = child.wait();
    true
}

fn which(bin: &str) -> Option<String> {
    let out = std::process::Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {bin}"))
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
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
