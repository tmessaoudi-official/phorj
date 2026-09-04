//! `Process.run` / `Process.runWith` — shell-free process execution (DEC-472).
//!
//! **There is deliberately no string-to-shell form.** `Process.run("rm -rf " + dir)` cannot be
//! written, because `program` and `args` are separate and the argv vector is passed to the OS
//! directly: no shell parses it, so quoting, globbing and `;`/`&&`/backtick injection have nowhere
//! to happen. That is the ruling's first clause and the reason this surface exists at all.
//!
//! Surface ruled 2026-09-04: `run(program, args)` for the common case, `runWith(program, args,
//! options)` when timeout, cwd or env are needed — natives take a fixed parameter list, so the knobs
//! ride on an injected `ProcessOptions` class rather than on optional trailing arguments.
//!
//! **A timeout FAULTS rather than returning a result.** A killed process has no meaningful exit
//! code, and returning one in a `ProcessResult` would make "we gave up" indistinguishable from "the
//! program exited with that status" — precisely the conflation DEC-503 removed from `Json.parse`.
//! The process is killed first, so nothing is left running.
//!
//! Impure by construction (it observes and changes the world), so it is `pure: false` and its
//! examples are differential-quarantined like the rest of `Core.Process`.

use crate::native::*;
use crate::types::Ty;
use crate::value::{HKey, Instance, Value};
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::time::{Duration, Instant};

/// Options read off a `ProcessOptions` instance. All absent for the plain `run`.
#[derive(Default)]
struct Opts {
    timeout_secs: i64,
    cwd: Option<String>,
    env: Vec<(String, String)>,
}

fn opts_from(v: &Value) -> Result<Opts, String> {
    let Value::Instance(inst) = v else {
        return Err(format!(
            "Process.runWith expects a ProcessOptions, got {}",
            v.type_name()
        ));
    };
    let timeout_secs = match inst.get_field("timeout") {
        Some(Value::Int(n)) => n,
        _ => 0,
    };
    let cwd = match inst.get_field("cwd") {
        Some(Value::Str(s)) => Some(s.to_string()),
        _ => None,
    };
    let mut env = Vec::new();
    if let Some(Value::Map(m)) = inst.get_field("env") {
        for (k, val) in m.iter() {
            if let (HKey::Str(k), Value::Str(v)) = (k, val) {
                env.push((k.to_string(), v.to_string()));
            }
        }
        // Deterministic order: a map's iteration order must not leak into the child's environment
        // in a way that differs between runs or between backends.
        env.sort();
    }
    Ok(Opts {
        timeout_secs,
        cwd,
        env,
    })
}

fn strings(v: &Value, who: &str) -> Result<Vec<String>, String> {
    match v {
        Value::List(items) => items
            .iter()
            .map(|i| match i {
                Value::Str(s) => Ok(s.to_string()),
                other => Err(format!(
                    "{who}: every argument must be a string, got {}",
                    other.type_name()
                )),
            })
            .collect(),
        other => Err(format!(
            "{who} expects a List<string>, got {}",
            other.type_name()
        )),
    }
}

fn result_value(stdout: String, stderr: String, code: i64) -> Value {
    let inst = Instance::new(
        "ProcessResult".into(),
        crate::value::ClassLayout::from_sorted_names(&["exitCode", "stderr", "stdout"]),
    );
    inst.set_field("stdout", Value::Str(stdout.into()));
    inst.set_field("stderr", Value::Str(stderr.into()));
    inst.set_field("exitCode", Value::Int(code));
    Value::Instance(Rc::new(inst))
}

fn spawn_and_wait(program: &str, args: &[String], o: &Opts) -> Result<Value, String> {
    let mut cmd = Command::new(program);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(dir) = &o.cwd {
        cmd.current_dir(dir);
    }
    for (k, v) in &o.env {
        cmd.env(k, v);
    }
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Process.run: cannot start `{program}` ({e})"))?;

    if o.timeout_secs <= 0 {
        let out = child
            .wait_with_output()
            .map_err(|e| format!("Process.run: `{program}` failed while running ({e})"))?;
        return Ok(result_value(
            String::from_utf8_lossy(&out.stdout).into_owned(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
            i64::from(out.status.code().unwrap_or(-1)),
        ));
    }

    // Drain both pipes on their own threads. Polling `try_wait` while the child writes to a full
    // pipe would deadlock: the child blocks on write, we block waiting for it to exit, and the
    // timeout never fires because nothing moves.
    let mut so = child.stdout.take().expect("piped");
    let mut se = child.stderr.take().expect("piped");
    let t_out = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = std::io::Read::read_to_end(&mut so, &mut b);
        b
    });
    let t_err = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = std::io::Read::read_to_end(&mut se, &mut b);
        b
    });

    let deadline = Instant::now() + Duration::from_secs(o.timeout_secs as u64);
    let status = loop {
        match child.try_wait() {
            Ok(Some(st)) => break st,
            Ok(None) => {
                if Instant::now() >= deadline {
                    // Kill FIRST, so a timeout never leaves the child running.
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!(
                        "Process.runWith: `{program}` exceeded its {}s timeout and was killed",
                        o.timeout_secs
                    ));
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(e) => {
                return Err(format!(
                    "Process.run: `{program}` failed while running ({e})"
                ))
            }
        }
    };
    let out = t_out.join().unwrap_or_default();
    let err = t_err.join().unwrap_or_default();
    Ok(result_value(
        String::from_utf8_lossy(&out).into_owned(),
        String::from_utf8_lossy(&err).into_owned(),
        i64::from(status.code().unwrap_or(-1)),
    ))
}

fn n_run(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(p), list] => {
            spawn_and_wait(p, &strings(list, "Process.run")?, &Opts::default())
        }
        _ => Err("Process.run expects (string, List<string>)".into()),
    }
}

fn n_run_with(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(p), list, opts] => {
            spawn_and_wait(p, &strings(list, "Process.runWith")?, &opts_from(opts)?)
        }
        _ => Err("Process.runWith expects (string, List<string>, ProcessOptions)".into()),
    }
}

pub(super) fn process_run_natives() -> Vec<NativeFn> {
    let result = || Ty::Named("ProcessResult".to_string(), vec![]);
    vec![
        NativeFn {
            module: "Core.Process",
            name: "run",
            params: vec![Ty::String, Ty::List(Box::new(Ty::String))],
            ret: result(),
            pure: false,
            eval: NativeEval::Pure(n_run),
            // No lift: PHP's `exec`/`shell_exec`/`system` all take a SHELL STRING, which is exactly
            // the form DEC-472 refuses. Lifting one onto this would reintroduce the injection
            // surface under a safe-looking name.
            lift_from: &[],
            php: |a| {
                format!(
                    "new ProcessResult(...__phorj_proc_run({}, {}, 0, null, null))",
                    parg(a, 0),
                    parg(a, 1)
                )
            },
        },
        NativeFn {
            module: "Core.Process",
            name: "runWith",
            params: vec![
                Ty::String,
                Ty::List(Box::new(Ty::String)),
                Ty::Named("ProcessOptions".to_string(), vec![]),
            ],
            ret: result(),
            pure: false,
            eval: NativeEval::Pure(n_run_with),
            lift_from: &[],
            php: |a| {
                format!(
                    "new ProcessResult(...__phorj_proc_run({}, {}, {}->timeout, {}->cwd, {}->env))",
                    parg(a, 0),
                    parg(a, 1),
                    parg(a, 2),
                    parg(a, 2),
                    parg(a, 2)
                )
            },
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(program: &str, args: &[&str]) -> Result<(String, String, i64), String> {
        let o = Opts::default();
        let a: Vec<String> = args.iter().map(|s| (*s).to_string()).collect();
        let v = spawn_and_wait(program, &a, &o)?;
        let Value::Instance(i) = v else {
            panic!("expected a ProcessResult")
        };
        let s = |n: &str| match i.get_field(n) {
            Some(Value::Str(s)) => s.to_string(),
            other => panic!("{n}: {other:?}"),
        };
        let code = match i.get_field("exitCode") {
            Some(Value::Int(n)) => n,
            other => panic!("exitCode: {other:?}"),
        };
        Ok((s("stdout"), s("stderr"), code))
    }

    #[test]
    fn captures_stdout_and_a_zero_exit() {
        let (out, err, code) = run("echo", &["hello", "world"]).expect("echo runs");
        assert_eq!(out, "hello world\n");
        assert_eq!(err, "");
        assert_eq!(code, 0);
    }

    /// A non-zero exit is a RESULT, not a fault — the caller decides whether it means failure.
    /// Faulting here would make `grep` finding nothing indistinguishable from `grep` being missing.
    #[test]
    fn a_non_zero_exit_is_a_result_not_a_fault() {
        let (_, _, code) = run("false", &[]).expect("`false` runs, it just exits 1");
        assert_eq!(code, 1);
    }

    /// THE security property DEC-472 exists for: no shell parses the argv, so metacharacters are
    /// literal data. If this ever regresses, `Process.run` becomes a command-injection surface.
    #[test]
    fn nothing_is_shell_interpreted() {
        let (out, _, _) = run("echo", &["a > b ; rm -rf / && whoami | cat"]).expect("echo runs");
        assert_eq!(
            out, "a > b ; rm -rf / && whoami | cat\n",
            "the argument must arrive LITERALLY — no redirect, no chaining, no substitution"
        );
        // A second guard on the same property: `$HOME` is not expanded, because nothing expands it.
        let (out2, _, _) = run("echo", &["$HOME"]).expect("echo runs");
        assert_eq!(out2, "$HOME\n");
    }

    #[test]
    fn stderr_is_captured_separately_from_stdout() {
        let (out, err, _) = run("sh", &["-c", "echo o; echo e 1>&2"]).expect("sh runs");
        assert_eq!(out, "o\n");
        assert_eq!(err, "e\n");
    }

    #[test]
    fn a_missing_program_is_an_error_not_a_panic() {
        let e = run("phorj-no-such-program-xyz", &[]).expect_err("must not start");
        assert!(e.contains("cannot start"), "{e}");
    }

    /// The timeout kills the child and FAULTS. Both halves matter: a timeout that returned a
    /// `ProcessResult` would make "we gave up" indistinguishable from an exit status, and one that
    /// did not kill would leak the process.
    #[test]
    fn a_timeout_kills_the_child_and_faults() {
        let o = Opts {
            timeout_secs: 1,
            ..Opts::default()
        };
        let t0 = std::time::Instant::now();
        let e = spawn_and_wait("sleep", &["30".to_string()], &o).expect_err("must time out");
        assert!(e.contains("timeout"), "{e}");
        assert!(
            t0.elapsed() < Duration::from_secs(10),
            "must give up at the deadline, not run the full 30s: {:?}",
            t0.elapsed()
        );
    }

    #[test]
    fn cwd_and_env_are_honoured() {
        let o = Opts {
            cwd: Some("/tmp".to_string()),
            ..Opts::default()
        };
        let v = spawn_and_wait("pwd", &[], &o).expect("pwd runs");
        let Value::Instance(i) = v else { panic!() };
        assert!(
            matches!(i.get_field("stdout"), Some(Value::Str(s)) if s.trim() == "/tmp"),
            "cwd must be honoured"
        );

        let o2 = Opts {
            env: vec![("PHORJ_TEST_VAR".to_string(), "set-by-test".to_string())],
            ..Opts::default()
        };
        let v2 = spawn_and_wait(
            "sh",
            &[
                "-c".to_string(),
                "printf %s \"$PHORJ_TEST_VAR\"".to_string(),
            ],
            &o2,
        )
        .expect("sh runs");
        let Value::Instance(i2) = v2 else { panic!() };
        assert!(
            matches!(i2.get_field("stdout"), Some(Value::Str(s)) if &*s == "set-by-test"),
            "env must reach the child"
        );
    }
}
