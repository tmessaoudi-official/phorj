//! `Core.Log` (DEC-220) end-to-end fixture.
//!
//! Core.Log natives are `pure: false` (they write `[LEVEL]` lines to stderr), so `uses_impure_native`
//! auto-quarantines `examples/guide/logging.phg` from the byte-identity differential. This fixture is
//! therefore the SOLE gate that exercises the shipped example through the real language surface —
//! `import Core.Log` resolution + `Log.*` namespaced-native dispatch — rather than calling
//! `log_natives()` directly (which the unit tests do). It asserts STDOUT (the captured output buffer);
//! the `[LEVEL]` lines go to the process's real stderr, which is not captured here and need not be
//! (logs are the out-of-band sink). `interp ≡ VM` holds — only the PHP leg is quarantined.

use phorj::cli::{cmd_run, cmd_treewalk};

/// The Log-v2 channel registry is PROCESS-GLOBAL (`src/native/log/state.rs`), and Rust runs the
/// tests in this file on concurrent threads of one process — so any test that emits or configures
/// logs must hold this lock, or one test's configured handlers swallow another's lines mid-assert.
static LOG_GATE: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn logging_example_runs_on_both_backends() {
    let _gate = LOG_GATE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let src = std::fs::read_to_string("examples/guide/logging.phg").expect("read logging.phg");
    // STDOUT is only the `Output.printLine` result; every `Log.*` line went to (uncaptured) stderr.
    let tree = cmd_treewalk(&src).expect("logging.phg runs on the interpreter");
    assert_eq!(tree, "sum = 6\n");
    // interp ≡ VM: the VM must produce byte-identical stdout (both call the one shared native body).
    assert_eq!(cmd_run(&src).expect("logging.phg runs on the VM"), tree);
}

// ── DEC-317 Log-v2: channel/handler/formatter CONTENT parity across all three legs ──────────────
//
// The module is quarantined from the byte-identity differential (impure), so this fixture IS the
// spine for Log-v2: the same program runs on the interpreter, the VM, and (when php is present —
// same gating as tests/conformance.rs) the transpiled PHP; stdout AND the handler-written log
// files must agree byte-for-byte. The v1 formats carry no timestamp/pid, so file content is fully
// deterministic. Stderr handlers are exercised for liveness by the run itself (not captured).

use std::process::Command;

fn php_bin() -> Option<String> {
    if std::env::var("PHORJ_SKIP_PHP").as_deref() == Ok("1") {
        return None;
    }
    let cand = std::env::var("PHORJ_PHP").unwrap_or_else(|_| "php".to_string());
    let ok = Command::new(&cand)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    ok.then_some(cand)
}

fn channels_src(dir: &str) -> String {
    format!(
        r#"package Main;
import Core.Output;
import Core.Log;
import Core.Log.Level;
import Core.Log.LineFormatter;
import Core.Log.JsonFormatter;
import Core.Log.FileHandler;
import Core.Log.RotatingFileHandler;
import Core.Log.ChannelConfig;
import Core.Log.LogConfig;
import Core.Runtime.Entry;

#[Entry]
function main(): void {{
    Log.configure(new LogConfig([
        new ChannelConfig("default", [
            new FileHandler("{dir}/app.log", new Level.Warn(), new LineFormatter())
        ]),
        new ChannelConfig("payments", [
            new FileHandler("{dir}/pay.log", new Level.Debug(), new JsonFormatter()),
            new RotatingFileHandler("{dir}/rot.log", 40, 2, new Level.Debug(), new LineFormatter())
        ])
    ]));
    Log.info("dropped by min level");
    Log.error("kept on default");
    Log.channel("payments").warning("first \"quoted\" line");
    Log.channel("payments").critical("second line");
    Log.channel("payments").debug("third line forces rotation");
    Output.printLine("done");
}}
"#
    )
}

fn read_logs(dir: &std::path::Path) -> (String, String, String, String) {
    let rd = |n: &str| std::fs::read_to_string(dir.join(n)).unwrap_or_default();
    (rd("app.log"), rd("pay.log"), rd("rot.log"), rd("rot.log.1"))
}

#[test]
fn log_v2_channels_write_identical_content_on_every_leg() {
    let _gate = LOG_GATE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let base = std::env::temp_dir().join(format!("phorj-logv2-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);

    // Leg 1: interpreter.
    let d1 = base.join("interp");
    std::fs::create_dir_all(&d1).unwrap();
    let src1 = channels_src(d1.to_str().unwrap());
    assert_eq!(cmd_treewalk(&src1).expect("interp runs"), "done\n");
    let interp = read_logs(&d1);
    assert_eq!(interp.0, "[ERROR] kept on default\n");
    assert_eq!(
        interp.1,
        "{\"channel\":\"payments\",\"level\":\"WARN\",\"message\":\"first \\\"quoted\\\" line\"}\n\
         {\"channel\":\"payments\",\"level\":\"CRITICAL\",\"message\":\"second line\"}\n\
         {\"channel\":\"payments\",\"level\":\"DEBUG\",\"message\":\"third line forces rotation\"}\n"
    );
    // 40-byte cap: the first two lines rotate away before the third is written.
    assert_eq!(interp.2, "[DEBUG] payments: third line forces rotation\n");
    assert_eq!(
        interp.3,
        "[WARN] payments: first \"quoted\" line\n[CRITICAL] payments: second line\n"
    );

    // Leg 2: VM.
    let d2 = base.join("vm");
    std::fs::create_dir_all(&d2).unwrap();
    let src2 = channels_src(d2.to_str().unwrap());
    assert_eq!(cmd_run(&src2).expect("vm runs"), "done\n");
    assert_eq!(read_logs(&d2), interp, "interp ≡ VM on handler content");

    // Leg 3: transpiled PHP (same gating as the conformance oracle; skip-loud without php).
    if let Some(php) = php_bin() {
        let d3 = base.join("php");
        std::fs::create_dir_all(&d3).unwrap();
        let src3 = channels_src(d3.to_str().unwrap());
        let code = phorj::cli::cmd_transpile(&src3).expect("transpiles");
        let php_file = base.join("prog.php");
        std::fs::write(&php_file, &code).unwrap();
        let out = Command::new(&php)
            .arg(&php_file)
            .output()
            .expect("php runs");
        assert!(
            out.status.success(),
            "php leg failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&out.stdout), "done\n");
        assert_eq!(read_logs(&d3), interp, "php leg content parity");
    } else {
        eprintln!("SKIP log_v2 php leg: php not found — set PHORJ_REQUIRE_PHP=1 to require it");
        assert!(
            std::env::var("PHORJ_REQUIRE_PHP").as_deref() != Ok("1"),
            "php required but not found"
        );
    }

    let _ = std::fs::remove_dir_all(&base);
}

/// The shipped Log-v2 example is differential-QUARANTINED (impure), so nothing else executes it —
/// this smoke keeps it from rotting green (audit 2026-07-22, P2).
#[test]
fn logging_v2_example_runs_on_both_backends() {
    let _gate = LOG_GATE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let src =
        std::fs::read_to_string("examples/guide/logging-v2.phg").expect("read logging-v2.phg");
    let tree = cmd_treewalk(&src).expect("logging-v2.phg runs on the interpreter");
    assert_eq!(tree, "program output still owns stdout\n");
    assert_eq!(cmd_run(&src).expect("runs on the VM"), tree, "interp ≡ VM");
}

/// DEC-329.4: the processor tail (`| ts=… pid=…` / trailing `"ts"`/`"pid"` json keys) is
/// OUT-OF-CONTRACT (env-dependent) — this test STRIPS it and byte-compares the deterministic
/// prefix on all three legs, while asserting the tail SHAPE is present on each.
#[test]
fn log_v2_processor_tail_is_out_of_contract_but_shaped() {
    let _gate = LOG_GATE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let base = std::env::temp_dir().join(format!("phorj-logpi-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let prog = |dir: &str| {
        format!(
            r#"package Main;
import Core.Output;
import Core.Log;
import Core.Log.Level;
import Core.Log.LineFormatter;
import Core.Log.JsonFormatter;
import Core.Log.FileHandler;
import Core.Log.ChannelConfig;
import Core.Log.LogConfig;
import Core.Runtime.Entry;
#[Entry] function main(): void {{
    Log.configure(new LogConfig([
        new ChannelConfig("app", [
            new FileHandler("{dir}/line.log", new Level.Debug(), new LineFormatter(true)),
            new FileHandler("{dir}/json.log", new Level.Debug(), new JsonFormatter(true))
        ])
    ]));
    Log.channel("app").warning("tail check");
    Output.printLine("done");
}}
"#
        )
    };
    let line_re = |s: &str| {
        let (prefix, tail) = s.trim_end().rsplit_once(" | ").expect("line tail present");
        assert!(
            tail.starts_with("ts=") && tail.contains(" pid="),
            "tail shape: {s}"
        );
        prefix.to_string()
    };
    let json_prefix = |s: &str| {
        let i = s.find(",\"ts\":").expect("json tail present");
        s[..i].to_string()
    };
    let read = |d: &std::path::Path| {
        (
            std::fs::read_to_string(d.join("line.log")).unwrap(),
            std::fs::read_to_string(d.join("json.log")).unwrap(),
        )
    };

    let d1 = base.join("interp");
    std::fs::create_dir_all(&d1).unwrap();
    assert_eq!(cmd_treewalk(&prog(d1.to_str().unwrap())).unwrap(), "done\n");
    let (l1, j1) = read(&d1);
    assert_eq!(line_re(&l1), "[WARN] app: tail check");
    assert_eq!(
        json_prefix(&j1),
        "{\"channel\":\"app\",\"level\":\"WARN\",\"message\":\"tail check\""
    );

    let d2 = base.join("vm");
    std::fs::create_dir_all(&d2).unwrap();
    assert_eq!(cmd_run(&prog(d2.to_str().unwrap())).unwrap(), "done\n");
    let (l2, j2) = read(&d2);
    assert_eq!(line_re(&l2), line_re(&l1), "interp ≡ VM prefix");
    assert_eq!(json_prefix(&j2), json_prefix(&j1));

    if let Some(php) = php_bin() {
        let d3 = base.join("php");
        std::fs::create_dir_all(&d3).unwrap();
        let code = phorj::cli::cmd_transpile(&prog(d3.to_str().unwrap())).expect("transpiles");
        let f = base.join("pi.php");
        std::fs::write(&f, &code).unwrap();
        let out = Command::new(&php).arg(&f).output().expect("php runs");
        assert!(
            out.status.success(),
            "php: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let (l3, j3) = read(&d3);
        assert_eq!(line_re(&l3), line_re(&l1), "php prefix parity");
        assert_eq!(json_prefix(&j3), json_prefix(&j1));
    }
    let _ = std::fs::remove_dir_all(&base);
}
