//! PHP transpiler — the `__phorj_proc_run` helper (DEC-472), gated by `uses_proc_run`.
//!
//! **`proc_open` is called with an ARRAY, never a string.** That is the whole point: the array form
//! (PHP 7.4+) passes argv to the OS directly with no shell, so `Process.run("echo", ["a ; rm -rf /"])`
//! passes one literal argument on this leg exactly as it does natively. Passing a string here would
//! reintroduce the injection surface DEC-472 exists to remove, on the leg least likely to be tested.
//!
//! The helper returns a 3-element array and the CALL SITE constructs `ProcessResult` from it via
//! argument unpacking. That is the DEC-313 `FileSystemResult` precedent, and it is not cosmetic: a
//! `new ProcessResult(...)` written inside a global helper would bind whatever `ProcessResult` is
//! visible THERE, which in a namespaced program is the wrong class or none.
//!
//! Both pipes are drained on every poll rather than after the wait. A child that fills a pipe blocks
//! on write while we wait for it to exit — the same deadlock the native side spawns reader threads
//! to avoid, in the shape PHP can express.

use super::*;

impl Transpiler {
    pub(super) fn emit_proc_run_helper(&mut self) {
        if !self.gates.uses_proc_run {
            return;
        }
        for line in PROC_RUN_HELPER.lines() {
            self.line(line);
        }
    }
}

const PROC_RUN_HELPER: &str = r#"function __phorj_proc_run($program, $args, $timeout, $cwd, $env) {
$cmd = array_merge([$program], $args);
$desc = [0 => ['pipe', 'r'], 1 => ['pipe', 'w'], 2 => ['pipe', 'w']];
$envArg = $env === null ? null : array_merge(getenv(), $env);
$proc = @proc_open($cmd, $desc, $pipes, $cwd, $envArg);
if ($proc === false) { throw new \RuntimeException("Process.run: cannot start `" . $program . "`"); }
fclose($pipes[0]);
stream_set_blocking($pipes[1], false);
stream_set_blocking($pipes[2], false);
$out = ''; $err = ''; $code = -1; $start = microtime(true);
while (true) {
$st = proc_get_status($proc);
$out .= stream_get_contents($pipes[1]);
$err .= stream_get_contents($pipes[2]);
if (!$st['running']) { $code = $st['exitcode']; break; }
if ($timeout > 0 && (microtime(true) - $start) >= $timeout) {
proc_terminate($proc, 9);
fclose($pipes[1]); fclose($pipes[2]); proc_close($proc);
throw new \RuntimeException("Process.runWith: `" . $program . "` exceeded its " . $timeout . "s timeout and was killed");
}
usleep(20000);
}
$out .= stream_get_contents($pipes[1]);
$err .= stream_get_contents($pipes[2]);
fclose($pipes[1]); fclose($pipes[2]); proc_close($proc);
return [$out, $err, $code];
}"#;
