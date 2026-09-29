// Run a transpiled Phorj program on a php-wasm instance.
//
// Why this is not just `php.run(code)`: php-wasm's `run()` hands `?>` + code to `pib_run`
// (php-wasm 0.1.0, `PhpBase._run`), so a script that opens with `<?php\ndeclare(strict_types=1);`
// reaches PHP with a close tag ahead of the `declare`, and PHP refuses it:
//   Fatal error: strict_types declaration must be the very first statement in the script
// Every program `phg transpile` emits opens that way (DEC-401), so every example failed.
//
// `pib_run` itself is fine with the declaration when nothing precedes it, so call it directly with
// the opening tag stripped. Dropping the declaration instead would run the oracle in weak mode, which
// changes coercion behaviour — the strict-mode TypeError still has to fire (php-run.test.mjs pins it).
// The stripping is exact: only a leading `<?php` plus the rest of that line, never anything deeper.
const OPEN_TAG = /^<\?php[ \t]*\r?\n?/;

export function stripOpenTag(code) {
  return OPEN_TAG.test(code) ? code.replace(OPEN_TAG, "") : null;
}

export async function runOnPhpWasm(php, code) {
  const bare = stripOpenTag(code);
  // No opening tag (or nothing we recognise): the public API is the only safe path.
  if (bare === null) return php.run(code);
  // The call `PhpBase._run` makes, minus its `?>` prefix. `flush` drains the stdout/stderr buffers
  // into the "output" / "error" events the caller listens on, exactly as `_run` does.
  try {
    return await php.binary.then((p) => p.ccall("pib_run", "number", ["string"], [bare]));
  } finally {
    php.flush();
  }
}
