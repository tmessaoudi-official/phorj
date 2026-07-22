// Phorj Playground — main thread orchestrator.
//
// Responsibilities: CodeMirror editor, a Web Worker running the Phorj WASM pipeline (with a
// per-call timeout that terminates a runaway program), lazy php-wasm to *execute* the transpiled
// PHP, the examples picker, the URL-hash permalink, diagnostics + `explain`, and the 2-way
// (Phorj VM vs transpiled PHP) agreement badge. Everything is client-side; nothing is sent to a
// server. NOTE: this is a WASM build — no native JIT tier exists in-browser on either leg; the
// "Phorj" pane is plain VM execution (what `phg run` does on the CLI, minus the JIT).

// CodeMirror is VENDORED (a single esbuild bundle in ./vendor/) rather than loaded from a CDN at
// runtime. esm.sh (the previous source) builds transitive deps on demand and was returning sustained
// 408 timeouts for `@codemirror/view`, so the editor never mounted and the page hung "loading". A CDN
// that serves split modules (jsdelivr `/+esm`) instead loads two `@codemirror/state` copies and
// crashes boot ("Unrecognized extension value"). Vendoring a single deduped bundle removes both
// failure modes and the runtime network dependency. Rebuild: see vendor/README.md.
import { EditorView, basicSetup } from "./vendor/codemirror.js";

// --- DOM ---------------------------------------------------------------------------------------
const $ = (id) => document.getElementById(id);
const RUN_TIMEOUT_MS = 5000;

// --- CodeMirror editor -------------------------------------------------------------------------
let view;
function initEditor(doc) {
  // Pass doc + extensions directly so EditorView builds the EditorState with ITS OWN bundled
  // @codemirror/state. Importing EditorState from a separate @codemirror/state copy created two
  // instances → CM threw "Unrecognized extension value in extension set" and killed boot() (so the
  // editor never mounted and the examples list never populated).
  view = new EditorView({
    doc,
    extensions: [basicSetup],
    parent: $("editor-pane"),
  });
}
const source = () => view.state.doc.toString();
function setSource(text) {
  view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: text } });
}

// --- WASM worker (with runaway-program timeout) ------------------------------------------------
let worker = null;
let nextId = 1;
const pending = new Map();

function spawnWorker() {
  worker = new Worker(new URL("./worker.js", import.meta.url), { type: "module" });
  worker.onmessage = (e) => {
    const p = pending.get(e.data.id);
    if (!p) return;
    clearTimeout(p.timer);
    pending.delete(e.data.id);
    e.data.ok ? p.resolve(e.data.result) : p.reject(new Error(e.data.error));
  };
  worker.onerror = () => {
    // The wasm instance aborted (e.g. a panic). Reject everything in flight and drop the worker;
    // the next call respawns a fresh one.
    for (const p of pending.values()) {
      clearTimeout(p.timer);
      p.reject(new Error("execution crashed"));
    }
    pending.clear();
    try { worker.terminate(); } catch { /* ignore */ }
    worker = null;
  };
}

function call(op, arg, timeoutMs = RUN_TIMEOUT_MS) {
  if (!worker) spawnWorker();
  const id = nextId++;
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      pending.delete(id);
      try { worker.terminate(); } catch { /* ignore */ }
      worker = null; // force respawn next call
      reject(new Error("execution timed out (possible infinite loop)"));
    }, timeoutMs);
    pending.set(id, { resolve, reject, timer });
    worker.postMessage({ id, op, arg });
  });
}

const callJson = (op, arg) =>
  call(op, arg).then(JSON.parse).catch((e) => ({ ok: false, error: String(e.message || e) }));

// --- php-wasm (lazy; the transpiled PHP executes in-browser, PHP 8.4) --------------------------
// php-wasm (seanmorris) defaults to PHP 8.4 — matching Phorj's transpile floor. Loaded only when
// the user first runs with "Run PHP" enabled. NOTE: this CDN import is the one integration point
// not exercised by the Rust test suite; pin a specific version once validated on first deploy.
const PHP_WASM_URL = "https://cdn.jsdelivr.net/npm/php-wasm/PhpWeb.mjs";
let PhpWebClass = null; // the imported class is cached; the PHP *instance* is not — see runPhp

async function loadPhpWeb() {
  if (!PhpWebClass) {
    ({ PhpWeb: PhpWebClass } = await import(PHP_WASM_URL));
  }
  return PhpWebClass;
}

// A PhpWeb instance keeps a PERSISTENT PHP runtime across run() calls, so re-running the program
// re-declares its global `function main()` -> "Cannot redeclare function main()". Use a FRESH instance
// per run for a clean global scope. (php.refresh() could reset one shared instance instead, but a fresh
// instance is correctness-certain regardless of php-wasm API specifics; the wasm binary stays cached.)
async function runPhp(code) {
  const PhpWeb = await loadPhpWeb();
  const php = new PhpWeb();
  await php.binary; // wait for this instance's wasm to be instantiated
  let out = "";
  const collect = (e) => (e.detail || []).forEach((s) => (out += s));
  php.addEventListener("output", collect);
  php.addEventListener("error", collect);
  try {
    await php.run(code);
  } finally {
    php.removeEventListener("output", collect);
    php.removeEventListener("error", collect);
  }
  return out;
}

// --- permalink (source in URL hash, browser-native compression, no JS dependency) --------------
function b64urlEncode(bytes) {
  let s = "";
  for (const b of bytes) s += String.fromCharCode(b);
  return btoa(s).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}
function b64urlDecode(str) {
  const s = atob(str.replace(/-/g, "+").replace(/_/g, "/"));
  return Uint8Array.from(s, (c) => c.charCodeAt(0));
}
async function streamThrough(stream, bytes) {
  const w = stream.writable.getWriter();
  w.write(bytes);
  w.close();
  return new Uint8Array(await new Response(stream.readable).arrayBuffer());
}
async function encodeSource(text) {
  const bytes = new TextEncoder().encode(text);
  if (typeof CompressionStream === "undefined") return "u" + b64urlEncode(bytes); // uncompressed fallback
  return "c" + b64urlEncode(await streamThrough(new CompressionStream("deflate-raw"), bytes));
}
async function decodeSource(hash) {
  const tag = hash[0];
  const bytes = b64urlDecode(hash.slice(1));
  if (tag === "u") return new TextDecoder().decode(bytes);
  if (tag === "c") {
    const out = await streamThrough(new DecompressionStream("deflate-raw"), bytes);
    return new TextDecoder().decode(out);
  }
  return null;
}

async function updatePermalink() {
  location.hash = await encodeSource(source());
}
async function share() {
  await updatePermalink();
  try {
    await navigator.clipboard.writeText(location.href);
    flashBadge("ok", "Link copied to clipboard.");
  } catch {
    flashBadge("neutral", "Link is in the address bar.");
  }
}

// --- lift (PHP -> Phorj draft) ----------------------------------------------------------------
// Treat the editor contents as PHP and run the `lift` engine (the inverse of transpile). On success
// the lifted Phorj draft REPLACES the editor (then runs through the VM/PHP pipeline). On failure the
// editor is left untouched and the badge carries the lift error — never a guess, never a silent blank.
// The lift carries no byte-identity guarantee (it's a best-effort scaffold), so — exactly like the
// `phg lift` CLI — we prepend a `// lifted (verify)` banner here at the entrypoint (the engine's
// `lift_source` returns the bare draft) so the review-required contract is visible in the editor.
const LIFT_BANNER = "// lifted (verify) — a best-effort PHP->Phorj draft; review before trusting it.\n";
async function liftPhp() {
  if (running) return;
  flashBadge("neutral", "Lifting PHP → Phorj…");
  const res = await callJson("lift", source());
  if (res.ok && res.phorj != null) {
    setSource(LIFT_BANNER + res.phorj);
    runAll(); // run the lifted draft so the user immediately sees it behave
  } else {
    // The engine's errors are already self-describing (they start with `lift …`); only the fallback
    // needs a prefix, so we avoid a double "lift: lift:".
    flashBadge("err", res.error || "lift: could not lift this PHP (outside the Tier-1 subset)");
  }
}

// --- tabs --------------------------------------------------------------------------------------
function showTab(name) {
  document.querySelectorAll(".tab").forEach((t) => t.classList.toggle("active", t.dataset.pane === name));
  document.querySelectorAll(".pane").forEach((p) => p.classList.toggle("active", p.id === "pane-" + name));
}

// --- diagnostics + explain ---------------------------------------------------------------------
function renderDiagnostics(check) {
  const host = $("pane-diag");
  host.textContent = "";
  const diags = (check && check.diagnostics) || [];
  if (check && check.parseError) {
    const d = document.createElement("div");
    d.className = "diag";
    d.innerHTML = `<span class="sev-error">syntax error</span> <span class="loc"></span>`;
    d.querySelector(".loc").textContent = check.parseError;
    host.appendChild(d);
  }
  if (!diags.length && !(check && check.parseError)) {
    const d = document.createElement("div");
    d.className = "diag-empty";
    d.textContent = "✓ no diagnostics — type-checks clean.";
    host.appendChild(d);
  }
  for (const g of diags) {
    const d = document.createElement("div");
    d.className = "diag";
    const sev = document.createElement("span");
    sev.className = g.severity === "warning" ? "sev-warning" : "sev-error";
    sev.textContent = `${g.severity} (${g.stage})`;
    const loc = document.createElement("span");
    loc.className = "loc";
    loc.textContent = ` at ${g.line}:${g.col}: ${g.message} `;
    d.append(sev, loc);
    if (g.code) {
      const code = document.createElement("span");
      code.className = "code";
      code.textContent = g.code;
      code.title = "Click for `phg explain`";
      code.onclick = () => showExplain(g.code);
      d.append(code);
    }
    if (g.hint) {
      const hint = document.createElement("span");
      hint.className = "hint";
      hint.textContent = "hint: " + g.hint;
      d.append(hint);
    }
    host.appendChild(d);
  }
  const errs = diags.filter((g) => g.severity === "error").length + (check && check.parseError ? 1 : 0);
  $("diag-count").textContent = errs ? `(${errs})` : "";
}

// Append a backend (VM) rejection to the diagnostics pane when the checker was clean — otherwise a
// lowering rejection (not a checker diagnostic) leaves the tab empty under a "does not compile"
// badge.
function surfaceBackendRejection(check, vm) {
  const checkClean = !(check && check.parseError) && !((check && check.diagnostics) || []).length;
  if (!checkClean) return;
  const msg = vm.error;
  if (!msg) return;
  const host = $("pane-diag");
  host.querySelectorAll(".diag-empty").forEach((n) => n.remove());
  const d = document.createElement("div");
  d.className = "diag";
  const sev = document.createElement("span");
  sev.className = "sev-error";
  sev.textContent = "backend rejection";
  const loc = document.createElement("span");
  loc.className = "loc";
  loc.textContent = " " + msg;
  d.append(sev, loc);
  host.appendChild(d);
  const cur = $("diag-count").textContent;
  $("diag-count").textContent = cur ? cur : "(1)";
}

async function showExplain(code) {
  $("explain-code").textContent = code;
  $("explain-body").textContent = "…";
  $("explain").classList.remove("hidden");
  try {
    $("explain-body").textContent = await call("explain", code);
  } catch (e) {
    $("explain-body").textContent = String(e.message || e);
  }
}

// --- run orchestration + agreement badge -------------------------------------------------------
function paneText(result) {
  if (!result) return "";
  if (result.ok) return result.stdout ?? "";
  if (result.fault) return "⚠ runtime fault:\n" + result.fault;
  if (result.error) {
    // A JS "Maximum call stack size exceeded" isn't a Phorj error — it's the browser's small stack
    // hitting deep recursion (the playground runs the VM directly in-browser, without the native
    // 256MB deep-stack worker). Surface it honestly instead of as a scary "✗ rejected".
    if (/Maximum call stack size exceeded/i.test(result.error)) {
      return (
        "ℹ this program recurses too deeply for the browser's small stack.\n" +
        "It runs fine locally: `phg run <file>` (the native build uses a 256MB stack).\n" +
        "(Not a Phorj error — a browser-runtime limit.)"
      );
    }
    return "✗ rejected:\n" + result.error;
  }
  return "(no output)";
}

function flashBadge(kind, text) {
  const b = $("badge");
  b.className = "badge " + kind;
  b.textContent = text;
}

function renderBadge(vm, phpOut, phpErr, phpEnabled) {
  // Front-end / lowering rejection — the error text is in the Phorj pane (a VM-only rejection is
  // NOT a checker diagnostic, so surfaceBackendRejection mirrors it into the diagnostics tab too).
  if (vm.error) {
    flashBadge("err", "✗ Does not compile — see the Phorj pane (and diagnostics)");
    return;
  }
  if (vm.fault) {
    // A runtime fault is a Phorj-defined outcome (not a bug by itself); there's nothing else to
    // cross-check it against in-browser, so just report it.
    flashBadge("ok", "✓ ran — runtime fault (see the Phorj pane).");
    return;
  }
  if (!phpEnabled) {
    flashBadge("ok", "✓ ran (PHP execution off).");
    return;
  }
  if (phpErr) {
    flashBadge("warn", "✓ ran — PHP could not be executed: " + phpErr);
    return;
  }
  if (phpOut === null) {
    flashBadge("ok", "✓ ran — PHP output unavailable (program did not transpile).");
    return;
  }
  if (phpOut === vm.stdout) flashBadge("ok", "✅ Phorj ≡ PHP — both backends agree.");
  else flashBadge("warn", "⚠ Phorj ran, but the transpiled PHP output differs.");
}

let running = false;
async function runAll() {
  if (running) return;
  running = true;
  flashBadge("neutral", "Running…");
  try {
    const phpEnabled = $("php-toggle").checked;
    const src = source();

    const check = await callJson("check", src);
    renderDiagnostics(check);

    const [vm, tr] = await Promise.all([
      callJson("vm", src),
      callJson("transpile", src),
    ]);

    $("pane-run").textContent = paneText(vm);
    $("pane-phpsrc").textContent = tr.ok ? tr.php : paneText(tr);

    // If the program type-checks clean but the VM still REJECTS it (a compiler/VM lowering
    // rejection, which is not a checker diagnostic), the diagnostics tab would otherwise look empty
    // while the badge says "does not compile". Surface the backend rejection there so the tab the
    // badge points at is never misleadingly blank.
    surfaceBackendRejection(check, vm);

    let phpOut = null;
    let phpErr = null;
    if (phpEnabled && tr.ok && tr.php) {
      $("pane-php").textContent = "Running PHP (php-wasm)…";
      try {
        phpOut = await runPhp(tr.php);
        $("pane-php").textContent = phpOut;
      } catch (e) {
        phpErr = String(e.message || e);
        $("pane-php").textContent = "php-wasm error:\n" + phpErr;
      }
    } else {
      $("pane-php").textContent = phpEnabled ? "(no PHP — program did not transpile)" : "(PHP execution disabled)";
    }

    renderBadge(vm, phpOut, phpErr, phpEnabled);
  } catch (e) {
    flashBadge("err", String(e.message || e));
  } finally {
    running = false;
  }
}

// --- examples sidebar --------------------------------------------------------------------------
// `window.PHORJ_EXAMPLES` is an ordered [{category, name, src}] list (gen_examples.py). The sidebar
// renders one section header per category with every example visible at once — click to load + run.
function initExamples() {
  const list = $("sidebar-list");
  if (!list) return;
  const examples = window.PHORJ_EXAMPLES || [];
  let lastCat = null;
  let activeBtn = null;
  for (const ex of examples) {
    if (ex.category !== lastCat) {
      const h = document.createElement("div");
      h.className = "sb-cat";
      h.textContent = ex.category;
      list.appendChild(h);
      lastCat = ex.category;
    }
    const b = document.createElement("button");
    b.className = "sb-item";
    b.type = "button";
    b.textContent = ex.name;
    b.title = `${ex.category} / ${ex.name}`;
    b.onclick = () => {
      setSource(ex.src);
      if (activeBtn) activeBtn.classList.remove("active");
      b.classList.add("active");
      activeBtn = b;
      runAll();
    };
    list.appendChild(b);
  }
}

// --- boot --------------------------------------------------------------------------------------
async function boot() {
  const examples = window.PHORJ_EXAMPLES || [];
  // Fallback when examples.js hasn't populated the global (e.g. it failed to load). Must be VALID
  // current Phorj — return types are mandatory — so the editor never boots a program that errors
  // on the first run. Mirrors gen_examples.py's DEFAULT (keep the two in sync).
  const defaultEx = examples.find((e) => e.name === "hello (default)");
  let initialDoc = (defaultEx && defaultEx.src) ||
    'package Main;\nimport Core.Output;\n\nfunction main(): void {\n    List<string> who = ["world", "Phorj"];\n    for (string w in who) {\n        Output.printLine("Hello, {w}!");\n    }\n}\n';
  if (location.hash.length > 2) {
    try {
      const decoded = await decodeSource(location.hash.slice(1));
      if (decoded) initialDoc = decoded;
    } catch { /* fall back to default */ }
  }

  initEditor(initialDoc);
  initExamples();

  $("run").onclick = runAll;
  $("share").onclick = share;
  $("lift").onclick = liftPhp;
  $("php-toggle").onchange = runAll;
  $("explain-close").onclick = () => $("explain").classList.add("hidden");
  document.querySelectorAll(".tab").forEach((t) => (t.onclick = () => showTab(t.dataset.pane)));
  document.addEventListener("keydown", (e) => {
    if ((e.ctrlKey || e.metaKey) && e.key === "Enter") { e.preventDefault(); runAll(); }
  });

  $("version").textContent = "latest @ master";
  runAll();
}

boot();
