// Regression test for the playground's PHP leg: transpiled Phorj opens with
// `<?php\ndeclare(strict_types=1);` (DEC-401), which php-wasm's own `run()` rejects because it
// prefixes `?>` ("strict_types declaration must be the very first statement"). Executes the REAL
// php-wasm (0.1.0, the version main.js pins) against the REAL `phg transpile` output.
//
// Run:  PHG=target/release/phg PHP_WASM_DIR=<dir holding node_modules/php-wasm@0.1.0> \
//         node playground/tests/php-run.test.mjs
// Without PHP_WASM_DIR it installs php-wasm@0.1.0 into a temp dir (needs network).
import { execFileSync } from "node:child_process";
import { existsSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { createRequire } from "node:module";
import { pathToFileURL } from "node:url";

const here = resolve(new URL(".", import.meta.url).pathname);
// The transpile-backed check needs a built `phg`; without one (CI's playground job builds only the wasm
// crate) it falls back to the exact shape `phg transpile` emits, so the php-wasm checks always run.
const phgPath = resolve(process.env.PHG || join(here, "../../target/release/phg"));
const phg = existsSync(phgPath) ? phgPath : null;
let wasmDir = process.env.PHP_WASM_DIR;
if (!wasmDir) {
  wasmDir = mkdtempSync(join(tmpdir(), "php-wasm-"));
  execFileSync("npm", ["init", "-y"], { cwd: wasmDir, stdio: "ignore" });
  execFileSync("npm", ["i", "php-wasm@0.1.0"], { cwd: wasmDir, stdio: "ignore" });
}
const req = createRequire(join(wasmDir, "noop.js"));
const { PhpNode } = await import(pathToFileURL(req.resolve("php-wasm/PhpNode.mjs")).href);
const { runOnPhpWasm, stripOpenTag } = await import("../web/php-run.js");

let fails = 0;
const check = (name, ok, detail = "") => {
  console.log(`${ok ? "ok  " : "FAIL"} ${name}${ok ? "" : "  " + detail}`);
  if (!ok) fails++;
};

async function exec(runner, code) {
  const php = new PhpNode();
  await php.binary;
  let out = "";
  const collect = (e) => (e.detail || []).forEach((s) => (out += s));
  php.addEventListener("output", collect);
  php.addEventListener("error", collect);
  await runner(php, code);
  return out;
}

const tmp = mkdtempSync(join(tmpdir(), "phg-src-"));
function transpile(src) {
  const f = join(tmp, "t.phg");
  writeFileSync(f, src);
  return execFileSync(phg, ["transpile", f], { encoding: "utf8" });
}

// Exactly what `phg transpile` emits for a one-line `#[Entry]` program (checked against the real binary below).
const SHAPE = '<?php\ndeclare(strict_types=1);\nfunction main(): void {\n    echo "hello", "\\n";\n}\nmain();';
const hello = phg
  ? transpile(
      'package Main;\nimport Core.Output;\nimport Core.Runtime.Entry;\nimport Core.Runtime.EntryKind;\n\n#[Entry(kind: EntryKind.Cli)]\nfunction main(): void {\n    Output.printLine("hello");\n}\n',
    )
  : SHAPE;
if (phg) check("the hand-written SHAPE equals the real transpiler output", hello.trim() === SHAPE, JSON.stringify(hello));
else console.log("note: no phg binary — using the hand-written SHAPE (set PHG to also check the real transpiler)");
check("program opens with the strict_types declaration", /^<\?php\s*\ndeclare\(strict_types=1\);/.test(hello), JSON.stringify(hello.slice(0, 60)));

// 1. The bug, reproduced: php-wasm's own run() rejects it. If this stops failing, php-wasm fixed it
//    upstream and php-run.js can be deleted — the test says so instead of passing silently.
const viaRun = await exec((p, c) => p.run(c), hello);
check("php.run(code) still hits the strict_types fatal (the bug this module works around)", viaRun.includes("strict_types declaration must be the very first statement"), JSON.stringify(viaRun));

// 2. The fix.
const viaFix = await exec(runOnPhpWasm, hello);
check("runOnPhpWasm runs the transpiled program", viaFix === "hello\n", JSON.stringify(viaFix));

// 3. Strictness is preserved, not dropped: a strict-mode TypeError must fire (weak mode would coerce).
const strictProbe = "<?php\ndeclare(strict_types=1);\nfunction f(string $x): string { return $x; }\ntry { echo f(1); } catch (TypeError $e) { echo \"TypeError\"; }\n";
const strictOut = await exec(runOnPhpWasm, strictProbe);
check("strict_types stays in force (int to string param throws TypeError)", strictOut === "TypeError", JSON.stringify(strictOut));

// 4. Uncaught faults still surface as output, with the fault text.
const fault = await exec(runOnPhpWasm, "<?php\ndeclare(strict_types=1);\necho \"a\\n\";\nthrow new Exception(\"boom\");\n");
check("an uncaught exception is reported", fault.startsWith("a\n") && fault.includes("Uncaught Exception: boom"), JSON.stringify(fault));

// 5. A fresh instance per run is the caller's contract; two consecutive runs both work.
check("a second run on a fresh instance works", (await exec(runOnPhpWasm, hello)) === "hello\n");

// 6. stripOpenTag is exact: only a LEADING tag; nothing else is touched, and a non-PHP string is refused.
check("stripOpenTag drops only the leading tag line", stripOpenTag("<?php\necho '<?php';\n") === "echo '<?php';\n");
check("stripOpenTag refuses code with no leading tag", stripOpenTag("echo 1;") === null);

console.log(fails ? `${fails} FAILED` : "ALL PASSED");
process.exit(fails ? 1 : 0);
