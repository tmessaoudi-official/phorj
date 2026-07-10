//! CLI pipeline helpers, kept in the library so they are unit-testable without
//! spawning the binary. `main.rs` is a thin dispatcher over these. Each command
//! is `fn(&str) -> Result<String, String>`: `Ok` is text to print verbatim
//! (newline-terminated where appropriate), `Err` is a rendered error message.

use crate::ast::Program;
use crate::chunk::{BytecodeProgram, Chunk, Op};
use crate::compiler::compile_with;
use crate::interpreter::{interpret, interpret_main};
use crate::parser::Parser;
use crate::tokenizer::lex;
use crate::vm::Vm;

// Self-contained command groups (M-Decomp W1.2): the `explain` diagnostic-code table and the
// `bench` profiling suite. Re-exported so callers keep referring to `cli::cmd_explain` etc.
mod benchmark;
mod debug_repl;
mod explain;
mod format_cmd;
mod rewrite_new;
mod test_runner;
pub use benchmark::{
    cmd_benchmark, cmd_benchmark_json, cmd_benchmark_vs_php, cmd_benchmark_vs_php_json,
};
pub use debug_repl::run_repl;
pub use explain::{cmd_explain, explain_text};
pub use format_cmd::{cmd_format, format_source};
pub use rewrite_new::cmd_rewrite_new;
pub use test_runner::cmd_test;

/// The `--version` line: `phg <version>` (from `CARGO_PKG_VERSION`).
pub fn version_line() -> String {
    format!("phg {}", env!("CARGO_PKG_VERSION"))
}

/// The `--help` text: version banner + commands + source forms + options.
pub fn help_text() -> String {
    format!(
        "{version}\n\
         usage:\n  \
         phg <command> <source> [options]\n\n\
         commands:\n  \
         run        run the program on the bytecode VM (--tree-walker for the interpreter oracle)\n  \
         check      type-check only\n  \
         parse      print the AST\n  \
         tokenize   print the token stream\n  \
         transpile  emit PHP\n  \
         lift       PHP -> a Phorj draft (review required; inverse of transpile)\n  \
         disassemble print the compiled bytecode\n  \
         benchmark  benchmark the interpreter vs the VM (time + memory)\n  \
         build      compile to a standalone executable (-o <out>)\n  \
         vendor     fetch [require] git deps into an offline vendor/ (writes phorj.lock)\n  \
         serve      serve the program over HTTP (calls respond(bytes): bytes per request)\n  \
         lsp        run the language server over stdio (LSP; for editors)\n  \
         debug      run the program under the interactive debugger (dev; --dap for DAP)\n  \
         test       discover and run `test` blocks (under tests/, or a given file/dir)\n  \
         format     format source to canonical form (--check for CI; - for stdin)\n  \
         explain    explain a diagnostic code (e.g. phg explain E-UNKNOWN-IDENT)\n\n\
         source:\n  \
         <file>     read the program from a file\n  \
         -          read the program from stdin\n  \
         -e <code>  run an inline program (alias: --eval)\n  \
         --         treat the next argument as a file path (even if it starts with '-')\n\n\
         options:\n  \
         -h, --help     print this help and exit\n  \
         -v, --version  print the version and exit\n",
        version = version_line()
    )
}

/// Per-command help: a one-line description, the source/flag forms, and 1–2 worked examples.
/// An unknown command falls back to the top-level [`help_text`].
pub fn help_for(cmd: &str) -> String {
    let body = match cmd {
        "run" => {
            "run — run the program on the bytecode VM (the runtime).\n\n\
                  usage:\n  phg run <file | - | -e code> [--tree-walker] [--no-jit] [--]\n\n\
                  flags:\n  \
                  --tree-walker   run on the tree-walking interpreter instead (the correctness\n                  \
                  oracle — slow by design, byte-identical to the VM; for validation, not everyday use)\n  \
                  --no-jit        run on the pure VM without native codegen (JIT is on by default;\n                  \
                  byte-identical to the JIT — an escape hatch, no rebuild needed)\n\n\
                  examples:\n  \
                  phg run hello.phg\n  \
                  phg run -e 'package Main; import Core.Output; function main(): void { Output.printLine(\"hi\"); }'\n  \
                  echo 'package Main; import Core.Output; function main(): void { Output.printLine(\"hi\"); }' | phg run -\n"
        }
        "check" => {
            "check — type-check only; print OK or the type errors, run nothing.\n\n\
                    usage:\n  phg check [--json] <file | - | -e code>\n\n\
                    flags:\n  \
                    --json   emit diagnostics as a JSON array (stage/severity/message/line/col/\n           \
                    code/hint) to stdout for editors/LSP; exit 0 if clean, 1 if errors\n\n\
                    examples:\n  \
                    phg check src.phg\n  \
                    phg check --json src.phg\n"
        }
        "parse" => {
            "parse — print the parsed AST (no type-check).\n\n\
                    usage:\n  phg parse <file | - | -e code>\n\n\
                    examples:\n  \
                    phg parse src.phg\n"
        }
        "tokenize" => {
            "tokenize — print the token stream with positions.\n\n\
                  usage:\n  phg tokenize <file | - | -e code>\n\n\
                  examples:\n  \
                  phg tokenize -e 'var x = 1;'\n"
        }
        "transpile" => {
            "transpile — emit idiomatic PHP for the program.\n\n\
                        usage:\n  phg transpile <file | - | -e code>\n\n\
                        examples:\n  \
                        phg transpile src.phg\n"
        }
        "lift" => {
            "lift — read PHP, emit a Phorj **draft** (the inverse of transpile). Best-effort and\n       \
                   REVIEW-REQUIRED: the output is a scaffold a human checks, prefixed `// lifted\n       \
                   (verify)`. Anything outside the Tier-1 subset (e.g. an `array` type, a backed enum,\n       \
                   string interpolation) is refused with a clear `lift …` error rather than guessed.\n\n\
                   usage:\n  phg lift <file.php | - | -e code>\n\n\
                   examples:\n  \
                   phg lift legacy.php\n  \
                   phg lift legacy.php > draft.phg\n"
        }
        "disassemble" => {
            "disassemble — print the compiled bytecode the VM will execute.\n\n\
                     usage:\n  phg disassemble <file | - | -e code>\n\n\
                     examples:\n  \
                     phg disassemble -e 'package Main; function main(): void { int x = 1 + 2; }'\n"
        }
        "benchmark" => {
            "benchmark — benchmark the interpreter vs the VM (median wall-clock + memory).\n\n\
                    usage:\n  phg benchmark [--vs-php] <file | - | -e code>\n\n\
                    flags:\n  \
                    --vs-php   also transpile + median-time the PHP backend (3-way comparison;\n             \
                               requires `php` on PATH; output-identity-gated)\n\n\
                    examples:\n  \
                    phg benchmark examples/benchmark/workload.phg\n  \
                    phg benchmark --vs-php examples/benchmark/workload.phg\n"
        }
        "build" => {
            "build — compile to a standalone executable (embeds the program source).\n\n\
                    usage:\n  phg build <file> [-o out] [--target triple | --all]\n\n\
                    examples:\n  \
                    phg build app.phg\n  \
                    phg build app.phg -o dist/app\n  \
                    phg build app.phg --target x86_64-unknown-linux-musl\n"
        }
        "test" => {
            "test — discover and run `test \"name\" { … }` blocks on the interpreter.\n\n\
                   With no path, runs every `*.phg` under the project's `tests/` directory (the\n\
                   project root is the nearest ancestor holding a `phorj.toml`, else the current\n\
                   directory). With a path, runs that file, or every `*.phg` under that directory.\n\
                   Each test runs independently; a failing assertion (or any fault) is reported with\n\
                   its message and the test keeps going. Exit 0 iff every test passed, else 1.\n\n\
                   usage:\n  phg test [path…]\n\n\
                   examples:\n  \
                   phg test\n  \
                   phg test tests/math.phg\n  \
                   phg test tests/\n"
        }
        "format" => {
            "format — format Phorj source to canonical form (comment-preserving, meaning-preserving).\n\n\
                  Prints from the parsed AST, so formatting never changes what the program means\n\
                  (parse(fmt(x)) == parse(x)); it is idempotent, and an unparseable file is left\n\
                  untouched (its diagnostic is reported, exit 2). v1 is tidy + comment-safe (canonical\n\
                  indentation/spacing/blank-lines), no line-wrapping yet.\n\n\
                  usage:\n  phg format [--check] [path… | -]\n\n\
                  flags:\n  \
                  --check   report files that aren't already formatted and exit 1; write nothing (CI)\n\n\
                  paths:\n  \
                  <none>    format every *.phg under the current directory, recursively\n  \
                  <file>    format that file in place\n  \
                  <dir>     format every *.phg under that directory in place\n  \
                  -         read from stdin, write the formatted result to stdout\n\n\
                  examples:\n  \
                  phg format\n  \
                  phg format src/app.phg\n  \
                  phg format --check .\n  \
                  cat app.phg | phg format -\n"
        }
        "explain" => {
            "explain — print the explanation for a diagnostic code.\n\n\
                      usage:\n  phg explain <CODE>\n\n\
                      examples:\n  \
                      phg explain E-UNKNOWN-IDENT\n"
        }
        "vendor" => {
            "vendor — fetch the project's `[require]` git dependencies into an offline `vendor/`.\n\n\
                     Clones each dependency at its pinned tag/rev, copies its source into\n\
                     `vendor/<vendor>/<package>/`, and writes `phorj.lock` (resolved SHA + content\n\
                     hash). This is the only command that touches the network; commit `vendor/` +\n\
                     `phorj.lock` so `run`/`check`/`transpile` resolve fully offline.\n\n\
                     usage:\n  phg vendor [project-dir | phorj.toml]   (defaults to .)\n\n\
                     examples:\n  \
                     phg vendor\n  \
                     phg vendor path/to/project\n"
        }
        "serve" => {
            "serve — serve the program over HTTP/1.1.\n\n\
                    The program must define `respond(bytes): bytes`: the runtime frames each\n\
                    incoming request, calls `respond` (where the program's own `parse_request` /\n\
                    router / `serialize_response` live — all pure Phorj), and writes the bytes back\n\
                    (`Connection: close`, one request per connection). A request fault degrades to a\n\
                    500; a malformed request is the program's concern (→ a 400 from `respond`).\n\n\
                    Concurrency (--workers, M6 W3): each request is handled on its own worker thread\n\
                    with its own value heap (the Rc heap is never shared — values don't cross threads),\n\
                    so the server scales across CPU cores. Default = number of cores; --workers 1 is the\n\
                    single-threaded server. Bind 127.0.0.1 (the default) on untrusted networks, and use\n\
                    --timeout so a slow/idle client cannot wedge a worker (slowloris). A per-connection\n\
                    read/write error never ends the server — it is logged and the next connection is\n\
                    served.\n\n\
                    Requests run on the bytecode VM by default (faster than the tree-walker —\n\
                    measured ~2.3x lower end-to-end latency on a representative handler, byte-identical\n\
                    output); --tree-walker selects the interpreter oracle instead (and is required to\n\
                    serve an overloaded `respond`, which the VM path rejects).\n\n\
                    usage:\n  phg serve <file> [--addr 127.0.0.1:8080] [--timeout SECONDS] [--workers N] [--tree-walker]\n\n\
                    options:\n  \
                    --addr ADDR        host:port to bind (default 127.0.0.1:8080)\n  \
                    --timeout SECONDS  per-connection read/write timeout; 0 = none (default 30)\n  \
                    --workers N        request concurrency; 1 = single-threaded (default = CPU cores)\n  \
                    --tree-walker      run requests on the interpreter oracle, not the (default) VM\n  \
                    --dev              rich HTML error page on an uncaught fault (DEV ONLY; prod = bare 500)\n\n\
                    examples:\n  \
                    phg serve examples/web/server.phg\n  \
                    phg serve app.phg --addr 0.0.0.0:3000 --timeout 15 --workers 8\n"
        }
        "lsp" => {
            "lsp — run the Phorj language server over stdio (LSP for editors).\n\n\
                   Speaks JSON-RPC on stdin/stdout; takes no source argument. Editors (VSCode,\n\
                   PhpStorm, or any LSP client) launch `phg lsp` as the server command; it serves\n\
                   diagnostics, hover, and completion backed by the checker.\n\n\
                   usage:\n  phg lsp\n"
        }
        "debug" => {
            "debug — run the program under the interactive debugger (dev-only, interpreter).\n\n\
                     Reads debugger commands on stdin and writes the UI to stderr; `--dap` speaks the\n\
                     Debug Adapter Protocol instead (for editor debug integration). Source load is\n\
                     project-aware (nearest ancestor with a phorj.toml, else the current directory).\n\n\
                     usage:\n  phg debug [--dap] <file>\n\n\
                     examples:\n  \
                     phg debug app.phg\n"
        }
        _ => return help_text(),
    };
    format!("{}\n{body}", version_line())
}

/// `vendor [project-dir | phorj.toml]`: fetch the project's `[require]` git dependencies into an
/// offline `vendor/` tree and (re)write `phorj.lock`. `arg` is a directory or a manifest path
/// (default `.`); the project root is found by walking up to a `phorj.toml`. The only network-
/// touching command — see [`crate::vendor`].
pub fn cmd_vendor(arg: &str) -> Result<String, String> {
    let start = std::path::Path::new(arg);
    match crate::manifest::Project::detect(start)? {
        Some(project) => crate::vendor::vendor(&project),
        None => Err(format!(
            "no phorj.toml found at or above `{arg}` — `phg vendor` requires a project \
             (add a phorj.toml with a [require] section)"
        )),
    }
}

/// Where a command reads its program from, resolved from the args after the subcommand.
#[derive(Debug, PartialEq, Eq)]
pub enum SourceSpec {
    /// Read the program from this file path.
    File(String),
    /// Read the program from standard input.
    Stdin,
    /// Run this inline program text directly.
    Inline(String),
}

/// Resolve the program source from the args following the subcommand (`args[2..]`):
/// `<file>` | `-` (stdin) | `-e <code>` / `--eval <code>` | `-- <file>`. Returns `None` on a usage
/// error (missing source, dangling `-e`, an unknown leading-`-` arg, or extra positionals) — the
/// caller prints usage and exits 2.
pub fn resolve_source(rest: &[String]) -> Option<SourceSpec> {
    resolve_source_and_args(rest).map(|(spec, _)| spec)
}

/// Like [`resolve_source`], but also returns the program's arguments (`Core.Process.args()`), taken
/// from a `--` terminator (Q5 of the Process-I/O design). Grammar:
/// `<file> [-- arg…]` | `- [-- arg…]` | `-e <code> [-- arg…]` | `-- <file> [-- arg…]`.
/// The **leading** `--` is the existing literal-path escape (`-- <file>`, for a path beginning with
/// `-`); a *non-leading* `--` separates phg's source-spec from the program's argv. So
/// `phg run app.phg -- a b` → `File(app.phg)` + `["a","b"]`, while `phg run -- -weird.phg -- a` →
/// `File(-weird.phg)` + `["a"]`. Returns `None` on a usage error (the caller prints usage, exits 2).
pub fn resolve_source_and_args(rest: &[String]) -> Option<(SourceSpec, Vec<String>)> {
    // Leading `--`: literal-path escape. `-- <file>` (no argv) | `-- <file> -- <argv…>`.
    if rest.first().map(String::as_str) == Some("--") {
        return match &rest[1..] {
            [path] => Some((SourceSpec::File(path.clone()), Vec::new())),
            [path, sep, args @ ..] if sep == "--" => {
                Some((SourceSpec::File(path.clone()), args.to_vec()))
            }
            _ => None,
        };
    }
    // Otherwise split the source-spec (before the first `--`) from the program argv (after it).
    let (head, args) = match rest.iter().position(|a| a == "--") {
        Some(i) => (&rest[..i], rest[i + 1..].to_vec()),
        None => (rest, Vec::new()),
    };
    let spec = match head {
        [flag, code] if flag == "-e" || flag == "--eval" => SourceSpec::Inline(code.clone()),
        [one] if one == "-" => SourceSpec::Stdin,
        [one] if !one.starts_with('-') => SourceSpec::File(one.clone()),
        _ => return None,
    };
    Some((spec, args))
}

/// Run a pipeline closure on a worker thread with a large (256 MB) stack. The tokenizer is iterative,
/// but the parser, checker, compiler, and tree-walking interpreter all recurse on the native stack
/// in proportion to expression/call nesting. A generous, *known* stack makes the explicit depth
/// limits (`limits::MAX_NEST_DEPTH`, `limits::MAX_CALL_DEPTH`) — not Rust's ambient frame budget —
/// the thing that bounds recursion, so adversarial-but-bounded input faults cleanly instead of
/// aborting, identically whether called from the CLI's main thread or a 2 MB test thread.
fn on_deep_stack<T: Send>(f: impl FnOnce() -> T + Send) -> T {
    std::thread::scope(|s| {
        std::thread::Builder::new()
            .stack_size(256 * 1024 * 1024)
            .spawn_scoped(s, f)
            .expect("spawn pipeline worker thread")
            .join()
            .expect("pipeline worker thread panicked")
    })
}

/// lex + parse, rendering the stage error to a single line. Every stage now returns a unified
/// [`crate::diagnostic::Diagnostic`] that renders itself (stage prefix + position), so the CLI
/// just calls `to_string()` rather than hand-formatting per stage.
fn lex_parse(src: &str) -> Result<Program, String> {
    let tokens = lex(src).map_err(|e| e.render(src))?;
    Parser::new(tokens)
        .parse_program()
        .map_err(|e| e.render(src))
}

/// Public lex + parse of a single source string into an **unchecked** `Program` (no type-check, no
/// alias/generic expansion). Exposes the private [`lex_parse`] for callers that want to run the
/// type-checker themselves and surface its diagnostics without aborting — e.g. the WASM playground,
/// which feeds the parsed program to [`check_json_program`] to render errors *and* warnings rather
/// than the fatal first-error string [`parse_checked`] produces. A syntax error still returns `Err`.
pub fn parse_program(src: &str) -> Result<Program, String> {
    lex_parse(src)
}

/// Type-check + de-alias an already-parsed program (the gate, minus lex/parse). De-aliases so every
/// backend sees alias-free types (aliases are front-end sugar; the checker validated them, including
/// cycles + built-in shadowing). Non-fatal warnings (the lint channel, M3 S2.5) render to stderr and
/// never gate the build. `diag_src` is the source used to render error carets — the single file for a
/// loose program, or `""` for a merged multi-file unit (where no single source aligns, so diagnostics
/// print message + position without a source line).
/// The canonical `Core.Json` value model, injected (below) when a program imports `Core.Json`. A
/// recursive enum over the JSON shapes; `Int`/`Float` are distinct (PHP-faithful, design-locked).
const JSON_PRELUDE: &str = "enum Json { Null(), Bool(bool value), Int(int value), \
     Float(float value), String(string value), Array(List<Json> items), Object(Map<string, Json> entries) }";

/// The canonical `RoundingMode` enum, injected (below) when a program imports `Core.Decimal`
/// (M-NUM S2). Zero-payload variants — constructed `new HalfUp()` and matched `HalfUp()`, the
/// project's zero-payload variant convention — read by `Decimal.div`/`Decimal.round` via the
/// variant name. The seven modes mirror `value::RoundMode`. (Same [[core-json-and-injected-types]]
/// injected-type pattern as `Json`.)
const ROUNDING_MODE_PRELUDE: &str =
    "enum RoundingMode { HalfUp(), HalfDown(), HalfEven(), Up(), Down(), Ceiling(), Floor() }";

/// True if the program imports the module `module` (e.g. `["Core", "Http"]`) either as a whole
/// (`import Core.Http`) OR via a **member-import** of one of its types, one segment deeper
/// (`import Core.Http.Router`). Import-redesign S2: a member-import must also pull in the injected
/// prelude, since the leaf type it names is one of that prelude's classes/enums.
fn imports_module_or_member(prog: &Program, module: &[&str]) -> bool {
    prog.items.iter().any(|it| {
        matches!(it, crate::ast::Item::Import { path, .. }
            if (path.len() == module.len() || path.len() == module.len() + 1)
                && path.iter().zip(module).all(|(a, b)| a == b))
    })
}

/// The canonical `Core.Option<T>` value model (DEC-182, Wave B foundation), injected (below) when a
/// program imports `Core.Option`. The opt-in rich absence type — distinct from the built-in `T?`
/// (lightweight built-in absence + what stdlib returns); interconvert explicitly, never implicitly.
/// The FIRST *generic* injected enum: `T` is checked as `Ty::Param` (the inject chain runs before
/// `check_resolutions`) then erased by the downstream `erase_generics` — identical discipline to a
/// user-declared `enum Option<T>`. Matches the canonical shape in `examples/guide/generic-enums.phg`.
const OPTION_PRELUDE: &str = "enum Option<T> { None, Some(T value) }";

/// The canonical `Core.Result<T, E>` value model (DEC-182, Wave B foundation), injected (below) when
/// a program imports `Core.Result`. Error-as-value: `Success(T)` or `Failure(E)`, where the error
/// payload `E` is a user enum. Pairs with the built-in `Error` marker + typed multi-catch; faults
/// stay uncatchable (bugs only). A generic injected enum like [`OPTION_PRELUDE`] — `T`/`E` are
/// erased downstream. Matches the canonical shape in `examples/guide/generic-enums.phg`.
const RESULT_PRELUDE: &str = "enum Result<T, E> { Success(T value), Failure(E error) }";

/// The canonical `Core.Http` types, injected (below) when a program imports `Core.Http` (M6 W1 →
/// stdlib). The portable handler model — `handle(Request): Response` — at the value level: `Request`
/// and `Response` are immutable values; `Request.parse(bytes) -> Request?` and `resp.serialize()`
/// round-trip the HTTP/1.1 wire form. The bodies reuse `Core.Bytes`/`Core.String` (so the prelude also
/// imports them), so this is the same proven logic as `examples/web/handler.phg`, promoted to the
/// stdlib behind the static-method API (slice B0). Flows through every backend as ordinary classes.
const HTTP_PRELUDE: &str = r#"
import Core.Bytes;
import Core.String;
import Core.List;
import Core.Regex;
class Request {
  constructor(public string method, public string path, public bytes body, private List<string> headerLines, private List<string> attrs) {}
  function header(string name): string? {
    for (string line in this.headerLines) {
      if (String.contains(line, ":")) {
        List<string> kv = String.splitOnce(line, ":");
        string key = String.trim(kv[0]);
        if (key == name) { return String.trim(kv[1]); }
      }
    }
    return null;
  }
  function param(string name): string? {
    mutable int i = 0;
    int n = List.length(this.attrs);
    while (i + 1 < n) {
      if (this.attrs[i] == name) { return this.attrs[i + 1]; }
      i += 2;
    }
    return null;
  }
  function withParams(List<string> p): Request {
    return new Request(this.method, this.path, this.body, this.headerLines, p);
  }
  static function parse(bytes raw): Request? {
    int sep = Bytes.find(raw, b"\x0d\x0a\x0d\x0a") ?? -1;
    if (sep < 0) { return null; }
    bytes headBytes = Bytes.slice(raw, 0, sep);
    bytes body = Bytes.slice(raw, sep + 4, Bytes.length(raw));
    string head = Bytes.toString(headBytes) ?? "";
    string nl = Bytes.toString(b"\x0d\x0a") ?? "";
    List<string> lines = String.split(head, nl);
    string requestLine = lines[0];
    List<string> rl = String.split(requestLine, " ");
    string method = rl[0];
    string path = rl[1];
    return new Request(method, path, body, lines, []);
  }
}
class Response {
  constructor(public int status, public bytes body, public List<string> headerLines) {}
  static function text(int status, string body): Response {
    return new Response(status, Bytes.fromString(body), ["Content-Type: text/plain"]);
  }
  static function reason(int s): string {
    return if (s == 200) { "OK" }
      else { if (s == 400) { "Bad Request" }
      else { if (s == 404) { "Not Found" }
      else { "Internal Server Error" } } };
  }
  function serialize(): bytes {
    string nl = Bytes.toString(b"\x0d\x0a") ?? "";
    string reason = Response.reason(this.status);
    int st = this.status;
    string statusLine = "HTTP/1.1 {st} {reason}";
    int bodyLen = Bytes.length(this.body);
    string userHeaders = String.join(this.headerLines, nl);
    string head = "{statusLine}{nl}Content-Length: {bodyLen}{nl}{userHeaders}{nl}{nl}";
    return Bytes.concat(Bytes.fromString(head), this.body);
  }
}
class Route {
  constructor(public string method, public string pattern, public (Request) -> Response handler) {}
}
class Router {
  // `table` = the registered routes; `mws` = middleware applied (outermost-first) to every matched
  // handler. Middleware is `(Request, next) -> Response`: it may call `next(req)` to continue the
  // chain, or short-circuit (e.g. return 401 without calling `next`).
  constructor(private List<Route> table, private List<(Request, (Request) -> Response) -> Response> mws) {}
  function route(string method, string pattern, (Request) -> Response handler): Router {
    return new Router(List.concat(this.table, [new Route(method, pattern, handler)]), this.mws);
  }
  // Append a middleware (applies to every route this router handles). Chainable, immutable.
  function use((Request, (Request) -> Response) -> Response mw): Router {
    return new Router(this.table, List.concat(this.mws, [mw]));
  }
  // Mount a sub-router under `prefix`: run `build` on a fresh empty router, then merge each sub-route
  // with `prefix` prepended to its pattern and the sub-router's own middleware composed around its
  // handler (so group-scoped middleware applies). The parent's `use` middleware still applies on top
  // in `handle`.
  function group(string prefix, (Router) -> Router build): Router {
    var builder = build;
    Router sub = builder(new Router([], []));
    mutable List<Route> merged = this.table;
    for (Route r in sub.table) {
      var h = r.handler;
      var wrapped = Router.compose(sub.mws, h);
      merged = List.concat(merged, [new Route(r.method, prefix + r.pattern, wrapped)]);
    }
    return new Router(merged, this.mws);
  }
  // Fold a middleware list around a handler: first-registered runs OUTERMOST. Each step builds a
  // `function(req) => mw(req, prev)` closure capturing the middleware and the previously-wrapped handler.
  static function compose(List<(Request, (Request) -> Response) -> Response> mws, (Request) -> Response handler): (Request) -> Response {
    mutable var h = handler;
    mutable int i = List.length(mws) - 1;
    while (i >= 0) {
      var mw = mws[i];
      var prev = h;
      h = function(Request req) -> Response { return mw(req, prev); };
      i -= 1;
    }
    return h;
  }
  static function idStrs(List<string> xs): List<string> { return xs; }
  // A pattern segment is a parameter iff it is `{...}`. The inner text is `name` (bare) or
  // `name:regex` (constrained); the regex must match the WHOLE path segment.
  static function isParam(string seg): bool {
    return String.startsWith(seg, "\{") && String.endsWith(seg, "\}");
  }
  static function paramInner(string seg): string {
    // Drop only the OUTER braces (substring 1..len-1) — a constraint regex may itself contain braces
    // (`\d{4}`), so stripping every `{`/`}` would corrupt it. `substring(s, 1, -1)` = bytes[1..len-1]
    // on both backends and PHP `substr($s, 1, -1)`.
    return String.substring(seg, 1, -1);
  }
  static function paramName(string seg): string {
    string inner = Router.paramInner(seg);
    if (String.contains(inner, ":")) { List<string> kv = String.splitOnce(inner, ":"); return kv[0]; }
    return inner;
  }
  // A constrained segment matches its path component iff the (whole-segment-anchored) regex matches.
  static function constraintOk(string seg, string component): bool {
    string inner = Router.paramInner(seg);
    if (String.contains(inner, ":")) {
      List<string> kv = String.splitOnce(inner, ":");
      var re = Regex.compile("^(?:" + kv[1] + ")$");
      return Regex.matches(re, component);
    }
    return true; // a bare `{name}` matches any component
  }
  // Specificity score (higher = more specific), or -1 for no match. A literal segment scores 2, a
  // matching CONSTRAINED param scores 1, a bare param scores 0 — so literal > constrained > param.
  // A constrained param whose component fails its regex makes the whole route not match.
  static function segScore(string pattern, string path): int {
    List<string> ps = String.split(pattern, "/");
    List<string> xs = String.split(path, "/");
    if (List.length(ps) != List.length(xs)) { return -1; }
    mutable int score = 0;
    mutable int i = 0;
    int n = List.length(ps);
    while (i < n) {
      string p = ps[i];
      if (Router.isParam(p)) {
        if (!Router.constraintOk(p, xs[i])) { return -1; }
        if (String.contains(Router.paramInner(p), ":")) { score += 1; }
      } else {
        if (p != xs[i]) { return -1; }
        score += 2;
      }
      i += 1;
    }
    return score;
  }
  static function extractParams(string pattern, string path): List<string> {
    List<string> ps = String.split(pattern, "/");
    List<string> xs = String.split(path, "/");
    mutable List<string> out = Router.idStrs([]);
    mutable int i = 0;
    int n = List.length(ps);
    while (i < n) {
      string p = ps[i];
      if (Router.isParam(p)) {
        out = List.concat(out, [Router.paramName(p), xs[i]]);
      }
      i += 1;
    }
    return out;
  }
  function handle(Request req): Response {
    mutable int best = -1;
    mutable int bestScore = -1;
    mutable int idx = 0;
    for (Route r in this.table) {
      if (r.method == req.method) {
        int sc = Router.segScore(r.pattern, req.path);
        if (sc > bestScore) { best = idx; bestScore = sc; }
      }
      idx += 1;
    }
    if (best < 0) { return Response.text(404, "Not Found: {req.method} {req.path}"); }
    Route chosen = this.table[best];
    List<string> params = Router.extractParams(chosen.pattern, req.path);
    var composed = Router.compose(this.mws, chosen.handler);
    return composed(req.withParams(params));
  }
}
"#;

/// The `phg serve` bridge: the runtime's `respond(bytes): bytes` entry, synthesized to wrap a
/// user-defined `handle(Request): Response` (closes Batch-1 C). Injected only when `Core.Http` is
/// imported, a `handle` exists, and the user hasn't written their own `respond`. A malformed request
/// (parse returns null) becomes a 400 — HTTP policy lives here in Phorj, not in the Rust runtime.
const HTTP_RESPOND_BRIDGE: &str = r#"
function respond(bytes raw): bytes {
  if (var req = Request.parse(raw)) {
    return handle(req).serialize();
  }
  return Response.text(400, "Bad Request").serialize();
}
"#;

/// The opaque compiled-`Regex` value model, injected when a program imports `Core.Regex` (Fork A,
/// `docs/specs/2026-06-28-core-regex-design.md`). A `Regex` value is built only by `Regex.compile`
/// (which validates via the `regex` crate); the `pattern` field is the **bare** pattern. It is public
/// so the transpiled `__phorj_regex_*` global helpers can read `$re->pattern` to build the
/// `/u`-delimited PHP `preg_*` form. Injected by [`inject_core_modules`] via the `Core.Regex`
/// registry row — a no-op unless `Core.Regex` is imported and no `Regex` class is already declared.
const REGEX_PRELUDE: &str = "class Regex { constructor(public string pattern) {} }";

/// The `Secret<T>` opaque-wrapper type, injected when a program imports `Core.Secret` (Fork B,
/// `docs/specs/2026-06-28-secret-type-design.md`). A `Secret<T>` value is constructed `new Secret(x)`
/// and read only through `expose()` — the `value` field is private, and a `Secret` instance is not a
/// `string`, so printing/interpolating it is a clean type error (the primary, loud guarantee; no
/// runtime `***`). Reuses the generic-class machinery (`Box<T>`) wholesale — no new `Op`/`Value`/`Ty`.
/// Injected by [`inject_core_modules`] via the `Core.Secret` registry row — a no-op unless
/// `Core.Secret` is imported and no `Secret` class is already declared. The transpiler adds `final`
/// + `#[\SensitiveParameter]` for this class by name.
const SECRET_PRELUDE: &str =
    "class Secret<T> { constructor(private T value) {} function expose(): T { return this.value; } }";

/// The `Core.Time` value model (M-TIME, `docs/specs/2026-06-28-m-time-design.md`): the pure-Phorj
/// `Instant`, `Duration`, `Date`, and `DateTime` classes. Because the prelude is run through the same
/// backends and transpiler as user code, all calendar and formatting math is byte-identical by
/// construction; the only native is the clock seam (the `Core.Time` module in `src/native/time.rs`).
/// The model is UTC-only because timezones are non-deterministic and would break the byte-identity
/// spine. Calendar math uses Hinnant's truncating-division-safe civil/day conversions, which port
/// verbatim since Phorj int division truncates toward zero (PHP `intdiv`).
const TIME_PRELUDE: &str = r#"
class Duration {
  constructor(public int ms) {}
  static function milliseconds(int n) -> Duration { return new Duration(n); }
  static function seconds(int n) -> Duration { return new Duration(n * 1000); }
  static function minutes(int n) -> Duration { return new Duration(n * 60000); }
  static function hours(int n) -> Duration { return new Duration(n * 3600000); }
  static function days(int n) -> Duration { return new Duration(n * 86400000); }
  function toMilliseconds() -> int { return this.ms; }
  function toSeconds() -> int { return this.ms / 1000; }
  function toMinutes() -> int { return this.ms / 60000; }
  function toHours() -> int { return this.ms / 3600000; }
  function toDays() -> int { return this.ms / 86400000; }
  function plus(Duration o) -> Duration { return new Duration(this.ms + o.ms); }
  function minus(Duration o) -> Duration { return new Duration(this.ms - o.ms); }
  function negate() -> Duration { return new Duration(-this.ms); }
  function isZero() -> bool { return this.ms == 0; }
  function isNegative() -> bool { return this.ms < 0; }
}
class Date {
  constructor(public int epochDay) {}
  // Howard Hinnant's days-from-civil / civil-from-days (truncating-division safe; Phorj int `/` is
  // truncate-toward-zero = PHP intdiv). `daysFromCivil`/`civil`/`pad2` are low-level building blocks
  // reused by `DateTime`; the everyday API is `of`/`year`/`month`/`day`/`addDays`/`toString`.
  static function daysFromCivil(int y, int m, int d) -> int {
    int yy = y - (if (m <= 2) { 1 } else { 0 });
    int era = (if (yy >= 0) { yy } else { yy - 399 }) / 400;
    int yoe = yy - era * 400;
    int doy = (153 * (if (m > 2) { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    int doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    return era * 146097 + doe - 719468;
  }
  static function civil(int z) -> List<int> {
    int zz = z + 719468;
    int era = (if (zz >= 0) { zz } else { zz - 146096 }) / 146097;
    int doe = zz - era * 146097;
    int yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    int y = yoe + era * 400;
    int doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    int mp = (5 * doy + 2) / 153;
    int d = doy - (153 * mp + 2) / 5 + 1;
    int m = if (mp < 10) { mp + 3 } else { mp - 9 };
    return [y + (if (m <= 2) { 1 } else { 0 }), m, d];
  }
  static function pad2(int n) -> string { return if (n < 10) { "0{n}" } else { "{n}" }; }
  // Zero-pad a non-negative year to 4 digits (ISO `YYYY`); proleptic negatives are emitted verbatim.
  static function pad4(int n) -> string {
    return if (n < 0) { "{n}" } else { if (n < 10) { "000{n}" } else { if (n < 100) { "00{n}" } else { if (n < 1000) { "0{n}" } else { "{n}" } } } };
  }
  static function of(int y, int m, int d) -> Date { return new Date(Date.daysFromCivil(y, m, d)); }
  static function ofEpochDay(int d) -> Date { return new Date(d); }
  function year() -> int { return Date.civil(this.epochDay)[0]; }
  function month() -> int { return Date.civil(this.epochDay)[1]; }
  function day() -> int { return Date.civil(this.epochDay)[2]; }
  function addDays(int n) -> Date { return new Date(this.epochDay + n); }
  function minusDays(int n) -> Date { return new Date(this.epochDay - n); }
  function daysUntil(Date o) -> int { return o.epochDay - this.epochDay; }
  // 1=Mon … 7=Sun (ISO-8601). epochDay 0 = 1970-01-01 = Thursday.
  function dayOfWeek() -> int {
    int w = (this.epochDay + 3) % 7;
    return (if (w < 0) { w + 7 } else { w }) + 1;
  }
  function isLeapYear() -> bool {
    int y = this.year();
    return (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0);
  }
  function isBefore(Date o) -> bool { return this.epochDay < o.epochDay; }
  function isAfter(Date o) -> bool { return this.epochDay > o.epochDay; }
  function compareTo(Date o) -> int {
    return if (this.epochDay < o.epochDay) { -1 } else { if (this.epochDay > o.epochDay) { 1 } else { 0 } };
  }
  function toString() -> string {
    List<int> c = Date.civil(this.epochDay);
    return "{Date.pad4(c[0])}-{Date.pad2(c[1])}-{Date.pad2(c[2])}";
  }
}
class Instant {
  constructor(public int ms) {}
  static function ofEpochMilliseconds(int m) -> Instant { return new Instant(m); }
  static function ofEpochSeconds(int s) -> Instant { return new Instant(s * 1000); }
  static function now() -> Instant { return new Instant(Time.nowMilliseconds()); }
  function epochMilliseconds() -> int { return this.ms; }
  function epochSeconds() -> int { return this.ms / 1000; }
  function plus(Duration d) -> Instant { return new Instant(this.ms + d.ms); }
  function minus(Duration d) -> Instant { return new Instant(this.ms - d.ms); }
  function durationSince(Instant o) -> Duration { return new Duration(this.ms - o.ms); }
  function isBefore(Instant o) -> bool { return this.ms < o.ms; }
  function isAfter(Instant o) -> bool { return this.ms > o.ms; }
  function compareTo(Instant o) -> int {
    return if (this.ms < o.ms) { -1 } else { if (this.ms > o.ms) { 1 } else { 0 } };
  }
  // Civil-date view (UTC, day-resolution): floor-divide milliseconds by a day (floor, not truncate, so a
  // pre-1970 instant maps to the right civil day).
  function toDate() -> Date {
    int day = if (this.ms >= 0) { this.ms / 86400000 } else { (this.ms - 86399999) / 86400000 };
    return Date.ofEpochDay(day);
  }
  // ── civil (wall-time) view, UTC ──────────────────────────────────────────────────────────────
  // An `Instant` is also the human date-time: it exposes year/month/day/hour/minute/second/milliseconds and
  // an ISO-8601 string. (No separate `DateTime` class — that name collides with PHP's built-in, and
  // `Instant` already IS the point in time; fields are derived on demand.) `ofCivil` builds an instant
  // from broken-down UTC fields.
  static function ofCivil(int y, int mo, int d, int h, int mi, int s) -> Instant {
    int day = Date.daysFromCivil(y, mo, d);
    return new Instant(day * 86400000 + h * 3600000 + mi * 60000 + s * 1000);
  }
  // Milliseconds within the current UTC day, always in [0, 86399999] (uses the floored epoch-day).
  function millisecondsOfDay() -> int {
    int day = if (this.ms >= 0) { this.ms / 86400000 } else { (this.ms - 86399999) / 86400000 };
    return this.ms - day * 86400000;
  }
  function year() -> int { return this.toDate().year(); }
  function month() -> int { return this.toDate().month(); }
  function day() -> int { return this.toDate().day(); }
  function dayOfWeek() -> int { return this.toDate().dayOfWeek(); }
  function hour() -> int { return this.millisecondsOfDay() / 3600000; }
  function minute() -> int { return (this.millisecondsOfDay() / 60000) % 60; }
  function second() -> int { return (this.millisecondsOfDay() / 1000) % 60; }
  function milliseconds() -> int { return this.millisecondsOfDay() % 1000; }
  // ISO-8601 UTC: `YYYY-MM-DDTHH:MM:SSZ` (always `Z`; second-resolution, sub-second dropped). For any
  // other layout, interpolate the accessors directly (Phorj has first-class string interpolation).
  function toIso() -> string {
    List<int> c = Date.civil(this.toDate().epochDay);
    string date = "{Date.pad4(c[0])}-{Date.pad2(c[1])}-{Date.pad2(c[2])}";
    string time = "{Date.pad2(this.hour())}:{Date.pad2(this.minute())}:{Date.pad2(this.second())}";
    return "{date}T{time}Z";
  }
}
"#;

/// A virtual `Core.*` module: its import path, its optional injected prelude source, how it gates
/// (whole-module-only vs also member-imports), and the injected member-type names that must be
/// import-qualified (the `module_of` contribution). UA-L2 (registry-unification): the single source
/// for BOTH the prelude-injection fold ([`inject_core_modules`]) AND the injected-type discipline
/// ([`core_module_of`]) — so a new Core module (Db, HTTP expansions) is ONE row here, not edits in
/// the eight `inject_*_prelude` fns plus the hand-synced `module_of` match this replaced.
struct VirtualModule {
    /// The import path segments, e.g. `["Core", "Http"]`. Gates injection; also the qualifier root.
    module: &'static [&'static str],
    /// The `module_of` return value for this row's `bare_types` (the dotted module below `Core.`),
    /// e.g. `"Http"`, `"Time"`, `"Runtime.Integer"`. Only meaningful when `bare_types` is non-empty.
    qualifier: &'static str,
    /// The prelude source to inject when the module is imported; `None` for attribute-only modules
    /// (`Core.DI`/`Core.Runtime*`) that contribute to `module_of` but inject no enum/class prelude.
    src: Option<&'static str>,
    /// The conditionally-injected `respond` serve-bridge source (Http only) — appended when the
    /// program defines `handle` and no `respond`. The one honest residual special-case.
    respond_bridge: Option<&'static str>,
    /// `true` → a member-import (`import Core.Http.Router`) also pulls the prelude in
    /// ([`imports_module_or_member`]); `false` → only a whole-module import (`import Core.Json`).
    member_gated: bool,
    /// Injected member-type names that `module_of` maps to `qualifier` — seeded EXPLICITLY to the
    /// pre-UA-L2 `module_of` set. NB: kept separate from the prelude's own declared names (the
    /// shadow-check derives those from the parsed source) — e.g. `Core.Time` injects `DateTime` too,
    /// but `DateTime` is deliberately NOT in `module_of` (see KNOWN_ISSUES). Fusing the two lists
    /// would silently change gating; `DateTime` is the proof they diverge.
    bare_types: &'static [&'static str],
}

/// The Core-module registry, in the SAME order as the pre-UA-L2 injection chain — ORDER IS
/// LOAD-BEARING: `HTTP_PRELUDE` transitively `import Core.Regex`, and Http runs BEFORE Regex, so
/// that transitive import is what triggers `Regex`-class injection for `Router.constraintOk`. A
/// reorder that broke this would still pass most tests; `examples/web/route-constraints.phg` (a
/// regex-constrained route with no explicit `import Core.Regex`) is the regression guard.
const CORE_MODULES: &[VirtualModule] = &[
    VirtualModule {
        module: &["Core", "Json"],
        qualifier: "Json",
        src: Some(JSON_PRELUDE),
        respond_bridge: None,
        member_gated: false,
        bare_types: &[],
    },
    VirtualModule {
        module: &["Core", "Decimal"],
        qualifier: "Decimal",
        src: Some(ROUNDING_MODE_PRELUDE),
        respond_bridge: None,
        member_gated: true,
        bare_types: &["RoundingMode"],
    },
    VirtualModule {
        module: &["Core", "Option"],
        qualifier: "Option",
        src: Some(OPTION_PRELUDE),
        respond_bridge: None,
        member_gated: true,
        bare_types: &[],
    },
    VirtualModule {
        module: &["Core", "Result"],
        qualifier: "Result",
        src: Some(RESULT_PRELUDE),
        respond_bridge: None,
        member_gated: true,
        bare_types: &[],
    },
    VirtualModule {
        module: &["Core", "Http"],
        qualifier: "Http",
        src: Some(HTTP_PRELUDE),
        respond_bridge: Some(HTTP_RESPOND_BRIDGE),
        member_gated: true,
        bare_types: &["Request", "Response", "Route", "Router"],
    },
    VirtualModule {
        module: &["Core", "Regex"],
        qualifier: "Regex",
        src: Some(REGEX_PRELUDE),
        respond_bridge: None,
        member_gated: false,
        bare_types: &[],
    },
    VirtualModule {
        module: &["Core", "Secret"],
        qualifier: "Secret",
        src: Some(SECRET_PRELUDE),
        respond_bridge: None,
        member_gated: false,
        bare_types: &[],
    },
    VirtualModule {
        module: &["Core", "Time"],
        qualifier: "Time",
        src: Some(TIME_PRELUDE),
        respond_bridge: None,
        member_gated: true,
        bare_types: &["Duration", "Date", "Instant"],
    },
    // Attribute-only modules — no prelude to inject; they exist only to gate their `#[…]` types.
    VirtualModule {
        module: &["Core", "Runtime", "Integer"],
        qualifier: "Runtime.Integer",
        src: None,
        respond_bridge: None,
        member_gated: false,
        bare_types: &["UncheckedOverflow"],
    },
    VirtualModule {
        module: &["Core", "Runtime"],
        qualifier: "Runtime",
        src: None,
        respond_bridge: None,
        member_gated: false,
        bare_types: &["Attribute"],
    },
    VirtualModule {
        module: &["Core", "DI"],
        qualifier: "DI",
        src: None,
        respond_bridge: None,
        member_gated: false,
        bare_types: &["Injectable", "Provides", "Transient"],
    },
];

/// The injected member type → owning module qualifier (UA-L2: the registry-derived replacement for
/// the hand-synced `module_of` match). Reused by the injected-type discipline
/// (`checker::enforce_injected`) and the qualified-construction dispatch (`checker::calls`/`expr`).
pub(crate) fn core_module_of(name: &str) -> Option<&'static str> {
    CORE_MODULES
        .iter()
        .find(|m| m.bare_types.contains(&name))
        .map(|m| m.qualifier)
}

/// Inject every applicable `Core.*` prelude at the program head, in registry order. Replaces the
/// eight chained `inject_*_prelude` fns with one uniform fold (UA-L2). For each module whose import
/// is present, each prelude item is prepended only if absent (imports by path; classes/enums/fns by
/// name), injected enums are marked `injected` (qualified-variant discipline), and Http's `respond`
/// bridge is appended when the program defines `handle` and no `respond`. A no-op (borrowed) for a
/// program that imports no injected Core module — such programs stay byte-for-byte unchanged.
fn inject_core_modules(prog: &Program) -> std::borrow::Cow<'_, Program> {
    use crate::ast::Item;
    let mut cur: std::borrow::Cow<'_, Program> = std::borrow::Cow::Borrowed(prog);
    for m in CORE_MODULES {
        let Some(src) = m.src else { continue };
        let p = cur.as_ref();
        let gated_in = if m.member_gated {
            imports_module_or_member(p, m.module)
        } else {
            p.items.iter().any(|it| {
                matches!(it, Item::Import { path, .. }
                    if path.len() == m.module.len()
                        && path.iter().zip(m.module).all(|(a, b)| a == b))
            })
        };
        if !gated_in {
            continue;
        }
        let Ok(parsed) = lex_parse(src) else {
            continue; // unreachable: registry preludes are valid
        };
        let mut prepend: Vec<Item> = Vec::new();
        for it in parsed.items {
            let absent = match &it {
                Item::Import { path, .. } => !p.items.iter().any(|x| {
                    matches!(x, Item::Import { path: xp, .. } if xp.join(".") == path.join("."))
                }),
                Item::Enum(e) => !p
                    .items
                    .iter()
                    .any(|x| matches!(x, Item::Enum(y) if y.name == e.name)),
                Item::Class(c) => !p
                    .items
                    .iter()
                    .any(|x| matches!(x, Item::Class(y) if y.name == c.name)),
                Item::Function(f) => !p
                    .items
                    .iter()
                    .any(|x| matches!(x, Item::Function(y) if y.name == f.name)),
                _ => false,
            };
            if absent {
                let mut it = it;
                if let Item::Enum(e) = &mut it {
                    e.injected = true;
                }
                prepend.push(it);
            }
        }
        // Http serve bridge: synthesize `respond` wrapping a user `handle`, when no `respond` exists.
        if let Some(bridge_src) = m.respond_bridge {
            let has_fn = |n: &str| {
                p.items
                    .iter()
                    .any(|x| matches!(x, Item::Function(f) if f.name == n))
            };
            if has_fn("handle") && !has_fn("respond") {
                if let Ok(bridge) = lex_parse(bridge_src) {
                    prepend.extend(
                        bridge
                            .items
                            .into_iter()
                            .filter(|it| matches!(it, Item::Function(f) if f.name == "respond")),
                    );
                }
            }
        }
        if prepend.is_empty() {
            continue;
        }
        let mut items = Vec::with_capacity(p.items.len() + prepend.len());
        items.extend(prepend);
        items.extend(p.items.iter().cloned());
        cur = std::borrow::Cow::Owned(Program {
            package: p.package.clone(),
            items,
            span: p.span,
        });
    }
    cur
}

pub fn check_and_expand(prog: &Program, diag_src: &str) -> Result<Program, String> {
    check_and_expand_reified(prog, diag_src).map(|(p, _)| p)
}

/// Like [`check_and_expand`], but also returns the checker's **reified-operand side-table** (S2.1-broad):
/// `expr span.start -> resolved Ty` for `Call`/`Member`/`Index` results, fed to the VM compiler
/// ([`crate::compiler::compile_with`]) so a generic method result / field read specializes as the
/// arithmetic operand the checker proved. The interpreter paths use the map-dropping wrapper above.
#[allow(clippy::type_complexity)]
pub fn check_and_expand_reified(
    prog: &Program,
    diag_src: &str,
) -> Result<(Program, std::collections::HashMap<usize, crate::types::Ty>), String> {
    // Import-redesign S2 stage C: enforce injected-type import discipline on the RAW user program,
    // BEFORE any prelude injection or the S1 qualifier collapse — so the preludes' own bare internals
    // are never scanned and bare-vs-qualified is still distinguishable. A bare injected member type
    // (`Router`, `Duration`, …) or `#[Route]` used without a member-import is `E-INJECTED-TYPE-BARE`.
    let injected_violations = crate::checker::enforce_injected_discipline(prog);
    if !injected_violations.is_empty() {
        let lines: Vec<String> = injected_violations
            .iter()
            .map(|e| e.render(diag_src))
            .collect();
        return Err(lines.join("\n"));
    }
    // DEC-196 Q3: fault-intrinsic import discipline (`Core.Assert`/`Core.Abort`). On the RAW program
    // (bare-vs-qualified still distinguishable): validate that every intrinsic call is covered by the
    // matching import (`E-UNIMPORTED` otherwise) AND normalize the qualified form `Assert.assert(x)`
    // down to the bare intrinsic `assert(x)` every backend already lowers. A no-op unless an intrinsic
    // module is touched, so intrinsic-free programs are byte-for-byte unchanged.
    let intrinsic_rewritten = match crate::checker::resolve_intrinsic_imports(prog.clone()) {
        Ok(p) => p,
        Err(ds) => {
            let lines: Vec<String> = ds.iter().map(|e| e.render(diag_src)).collect();
            return Err(lines.join("\n"));
        }
    };
    let prog = &intrinsic_rewritten;
    // UA-L2 (registry-unification): one fold over `CORE_MODULES` replaces the former eight chained
    // `inject_*_prelude` calls — a no-op for programs that import no injected Core module. Byte-identical
    // to the old chain (proven over the whole corpus at cutover); adding a Core module is now one row.
    let injected = inject_core_modules(prog);
    // M6 W2: lower `Http.autoRouter()` into explicit `Router` construction from the `#[Route]`-
    // annotated handlers — BEFORE the checker, so the generated registration type-checks like
    // hand-written code (a no-op unless `Core.Http` is imported). The `#[Route]` attrs survive for the
    // checker's validation pass, then are inert for the backends.
    let routed = crate::checker::desugar_auto_router(injected.into_owned());
    // Import-redesign S1: collapse qualified injected-type references (`Http.Router`, `Time.Duration`,
    // `Decimal.RoundingMode`) in type-annotation position down to their bare injected type — so both the
    // checker AND every backend see the plain `Router`/`Duration`/`RoundingMode` the preludes declare.
    // Runs after `desugar_auto_router` (its generated `Router` construction is bare already) and before
    // `check_resolutions`.
    let routed = crate::checker::collapse_injected_type_qualifiers(routed);
    // Wave B B-2c (DEC-186): resolve imported injected-enum variants (`import Core.Result.Success;` /
    // `… as X;` / grouped) to their qualified `Enum.Variant` form, so the proven qualified construction +
    // pattern paths handle them byte-identically. A no-op unless a variant import is present. Runs after
    // the qualifier collapse (its output is bare injected TYPE names, disjoint from variant heads) and
    // before `check_resolutions`.
    let routed = crate::checker::resolve_variant_imports(routed);
    // DI v1 (`docs/plans/di-attributes.plan.md`): expand `inject<T>()` composition roots into plain
    // `new` construction (a synthesized `__phorj_di_<T>()` factory per requested type). Pre-check, so
    // the generated graph type-checks like hand-written code and every backend sees explicit
    // construction (Inv-5). A no-op unless `inject` is used; compile errors (E-DI-*/E-INJECT-NO-TYPE)
    // surface here exactly like the other pre-check passes.
    let routed = match crate::checker::desugar_di(routed) {
        Ok(p) => p,
        Err(ds) => {
            let lines: Vec<String> = ds.iter().map(|e| e.render(diag_src)).collect();
            return Err(lines.join("\n"));
        }
    };
    let prog = &routed;
    match crate::checker::check_resolutions(prog) {
        Ok((warnings, html, ufcs, overload_renames, reified)) => {
            for w in &warnings {
                eprintln!("warning: {}", w.render(diag_src));
            }
            // De-alias types, erase `html"…"` literals into their `Html.concat([…])` kernel calls
            // (built by the checker, keyed by span), then erase generic type parameters — all three
            // are front-end sugar removed before any backend runs (M-RT S7 adds the last).
            // Feature C: `unwrap_new` strips the `Expr::New` construction wrapper after the type sugar
            // is gone, so every backend sees the plain construction `Call`. Slice 6: `rewrite_ufcs`
            // runs last, rewriting each resolved `x.f(a)` member call into the ordinary free/native
            // call `f(x, a)` the checker chose — by then the receiver/args are fully de-sugared.
            // Batch D: inject `= null` defaults for optional instance fields (after aliases are
            // expanded, so an aliased optional is already `Type::Optional`) — a front-end desugar so
            // every backend initializes them identically.
            // Slice C1: rename each return-overload member's *definition* to its mangled name (by decl
            // span); the resolved selector *call sites* were already merged into `ufcs` above and are
            // rewritten to the same mangled names by `rewrite_ufcs`. A no-op when no function is
            // return-overloaded (so single-overload programs stay byte-identical).
            // B1b: inline `parent.constructor(…)` LAST, so the cloned parent body is already fully
            // de-sugared (aliases/html/generics/new/UFCS/overload-renames all applied). A no-op unless
            // a constructor forwards to its parent — programs without it stay byte-identical.
            Ok((
                crate::checker::inline_parent_ctors(crate::checker::rename_overload_defs(
                    crate::checker::rewrite_ufcs(
                        crate::checker::unwrap_new(crate::checker::erase_generics(
                            crate::checker::resolve_html(
                                crate::checker::inject_optional_field_defaults(
                                    crate::checker::expand_aliases(prog),
                                ),
                                &html,
                            ),
                        )),
                        &ufcs,
                    ),
                    &overload_renames,
                )),
                reified,
            ))
        }
        Err(errs) => {
            let lines: Vec<String> = errs.iter().map(|e| e.render(diag_src)).collect();
            Err(lines.join("\n"))
        }
    }
}

/// lex + parse + type-check (the gate). Renders every type error, one per line.
fn parse_checked(src: &str) -> Result<Program, String> {
    let prog = lex_parse(src)?;
    check_and_expand(&prog, src)
}

/// Like [`parse_checked`], but also returns the checker's **reified-operand side-table** so a
/// VM-running caller can [`compile_with`] it — the byte-identical path [`cmd_run`] uses. Without it,
/// a method-call/field-read result used as an arithmetic operand (`a.join() + b.join()`,
/// `box.get() + 1`) is rejected by the VM compiler (`ctype` falls through to `method_rets`) while the
/// interpreter accepts it — a `run ≠ runvm` divergence. Any inline-source path that builds a
/// `BytecodeProgram` (`disasm`, `bench`) MUST use this, not `parse_checked` + `compile`.
#[allow(clippy::type_complexity)]
fn parse_checked_reified(
    src: &str,
) -> Result<(Program, std::collections::HashMap<usize, crate::types::Ty>), String> {
    let prog = lex_parse(src)?;
    check_and_expand_reified(&prog, src)
}

/// Public lex + parse + check of a single source string into a checked, alias-expanded `Program`.
/// Exposes the private [`parse_checked`] pipeline for callers that need a ready-to-run program from
/// inline source — e.g. `tests/serve.rs`, which builds a serve program then drives it through
/// [`crate::serve::serve`] over an in-memory transport.
pub fn parse_checked_program(src: &str) -> Result<Program, String> {
    parse_checked(src)
}

/// Like [`parse_checked_program`], but also returns the reified-operand side-table — so a caller (e.g.
/// `tests/serve.rs`) can build the VM serve factory ([`crate::serve::vm_factory`]) on the exact
/// byte-identical path the CLI uses.
#[allow(clippy::type_complexity)]
pub fn parse_checked_program_reified(
    src: &str,
) -> Result<(Program, std::collections::HashMap<usize, crate::types::Ty>), String> {
    parse_checked_reified(src)
}

/// `run`: lex -> parse -> check (gate) -> interpret -> captured stdout.
/// M8.5 interop: refuse to *execute* a program that uses foreign `declare` symbols. The Rust backends
/// (interpreter/VM) have no PHP runtime, so a foreign call cannot run — the program is PHP-target-only.
/// `check`/`transpile` work fully; only `run`/`runvm` hit this one clean pre-flight gate (no per-call
/// fault machinery in the execution paths). A single scan after type-checking, before any backend.
fn foreign_runtime_gate(prog: &Program) -> Result<(), String> {
    use crate::ast::Item;
    let has_foreign = prog.items.iter().any(|it| {
        matches!(it, Item::Function(f) if f.foreign) || matches!(it, Item::Class(c) if c.foreign)
    });
    if has_foreign {
        return Err(
            "error[E-FOREIGN-RUNTIME]: this program declares foreign PHP symbols (`declare`), \
             which require the PHP runtime to execute. The Rust backends (run/runvm) have no PHP \
             runtime — transpile it instead: `phg transpile <file> > out.php && php out.php`.\n"
                .to_string(),
        );
    }
    Ok(())
}

/// Check + de-sugar a program for the interactive debugger (M-DX S5): the same `check_and_expand`
/// the run backends use, plus the foreign-runtime gate (the debugger is interpreter-only, so a
/// `declare`d foreign-PHP program can't be stepped). Shared by the REPL and DAP frontends.
pub fn check_and_expand_for_debug(prog: &Program, diag_src: &str) -> Result<Program, String> {
    let checked = check_and_expand(prog, diag_src)?;
    foreign_runtime_gate(&checked)?;
    Ok(checked)
}

pub fn cmd_treewalk(src: &str) -> Result<String, String> {
    on_deep_stack(|| {
        let prog = parse_checked(src)?;
        foreign_runtime_gate(&prog)?;
        // S4.3 cutover: a program that uses `spawn` runs on the cooperative green-thread driver (real
        // task interleaving); every other program stays on the unchanged synchronous interpreter. wasm
        // (and a `--no-default-features` build without `green`) keeps the eager path — the cfg gate
        // makes the cooperative driver absent there. Byte-identical to `runvm` via the shared scheduler.
        #[cfg(all(feature = "green", not(target_arch = "wasm32")))]
        if crate::ast::uses_concurrency(&prog) {
            return crate::interpreter::run_cooperative_interp(&prog)
                .map(|(out, _exit)| out)
                .map_err(|e| e.to_string());
        }
        interpret(&prog).map_err(|e| e.to_string())
    })
}

/// `runvm`: lex -> parse -> check (gate) -> compile to bytecode -> VM -> captured stdout.
/// The bytecode backend; must produce byte-identical output to `cmd_treewalk` (differential).
/// Build a `Vm` for `program`, attaching a fresh JIT hot-function cache when the `jit` feature is on
/// (b3b — wire `phg run` to the JIT). A fresh cache per program run keeps the compile-once cache
/// correctly scoped to the one program's bytecode. On a non-jit build this is exactly `Vm::new`.
fn vm_for(program: &BytecodeProgram) -> Vm<'_> {
    #[cfg(feature = "jit")]
    {
        // JIT by default; `phg run --no-jit` (crate::vm::set_jit_enabled(false)) falls back to the
        // pure VM without a rebuild — the byte-identical oracle path, an escape hatch for a suspected
        // JIT issue.
        if crate::vm::jit_enabled() {
            Vm::new(program).with_jit(std::rc::Rc::new(std::cell::RefCell::new(
                crate::vm::JitCache::new(),
            )))
        } else {
            Vm::new(program)
        }
    }
    #[cfg(not(feature = "jit"))]
    {
        Vm::new(program)
    }
}

pub fn cmd_run(src: &str) -> Result<String, String> {
    on_deep_stack(|| {
        let parsed = lex_parse(src)?;
        let (prog, reified) = check_and_expand_reified(&parsed, src)?;
        foreign_runtime_gate(&prog)?;
        let program = compile_with(&prog, &reified).map_err(|e| e.to_string())?;
        #[cfg(all(feature = "green", not(target_arch = "wasm32")))]
        if crate::ast::uses_concurrency(&prog) {
            return crate::vm::run_cooperative_vm(&program)
                .map(|(out, _exit)| out)
                .map_err(|e| e.to_string());
        }
        vm_for(&program).run().map_err(|e| e.to_string())
    })
}

/// Like [`cmd_treewalk`], but also returns `main`'s exit code (Batch-1 B). The string source path
/// (`-e`/stdin and standalone built binaries); the project-loader path is [`treewalk_program_exit`].
pub fn cmd_treewalk_exit(src: &str) -> Result<(String, i64), String> {
    on_deep_stack(|| {
        let prog = parse_checked(src)?;
        foreign_runtime_gate(&prog)?;
        #[cfg(all(feature = "green", not(target_arch = "wasm32")))]
        if crate::ast::uses_concurrency(&prog) {
            return crate::interpreter::run_cooperative_interp(&prog).map_err(|e| e.to_string());
        }
        interpret_main(&prog).map_err(|e| e.to_string())
    })
}

/// Like [`cmd_run`], but also returns `main`'s exit code (Batch-1 B).
pub fn cmd_run_exit(src: &str) -> Result<(String, i64), String> {
    on_deep_stack(|| {
        let parsed = lex_parse(src)?;
        let (prog, reified) = check_and_expand_reified(&parsed, src)?;
        foreign_runtime_gate(&prog)?;
        let program = compile_with(&prog, &reified).map_err(|e| e.to_string())?;
        #[cfg(all(feature = "green", not(target_arch = "wasm32")))]
        if crate::ast::uses_concurrency(&prog) {
            return crate::vm::run_cooperative_vm(&program).map_err(|e| e.to_string());
        }
        vm_for(&program).run_main().map_err(|e| e.to_string())
    })
}

/// `check`: lex -> parse -> check; report success or the type errors.
pub fn cmd_check(src: &str) -> Result<String, String> {
    on_deep_stack(|| {
        parse_checked(src)?;
        Ok("OK (type-checks clean)\n".to_string())
    })
}

// --- Program-taking runners (M5 S2b) -----------------------------------------------------------
// The project loader (`crate::loader`) resolves a file path to a single, possibly multi-file-merged
// `Program`; these run/check/transpile it. They mirror the `cmd_*(&str)` pipelines exactly (same
// check -> de-alias -> backend), so a loose single-file program routed through `loader` produces
// byte-identical output. `diag_src` carries the source for error carets (`""` for a merged unit).

/// `run` on a loaded [`Unit`] (interpreter backend). A runtime fault is rendered **with its stack
/// trace** (error-handling slice 1): frames are attributed to files via the unit's `fn_files`, and the
/// caret is drawn against the innermost frame's source (project mode) or the single `diag_src` (loose).
pub fn treewalk_program(unit: &crate::loader::Unit) -> Result<String, String> {
    on_deep_stack(|| {
        let checked = check_and_expand(&unit.program, &unit.diag_src)?;
        foreign_runtime_gate(&checked)?;
        #[cfg(all(feature = "green", not(target_arch = "wasm32")))]
        if crate::ast::uses_concurrency(&checked) {
            return crate::interpreter::run_cooperative_interp(&checked)
                .map(|(out, _exit)| out)
                .map_err(|mut e| {
                    let src = unit.attribute_frames(&mut e);
                    e.render(&src)
                });
        }
        interpret(&checked).map_err(|mut e| {
            let src = unit.attribute_frames(&mut e);
            e.render(&src)
        })
    })
}

/// `runvm` on a loaded [`Unit`] (bytecode + VM backend). Same trace rendering as [`treewalk_program`].
pub fn run_program(unit: &crate::loader::Unit) -> Result<String, String> {
    on_deep_stack(|| {
        let (checked, reified) = check_and_expand_reified(&unit.program, &unit.diag_src)?;
        foreign_runtime_gate(&checked)?;
        let program = compile_with(&checked, &reified).map_err(|e| e.to_string())?;
        #[cfg(all(feature = "green", not(target_arch = "wasm32")))]
        if crate::ast::uses_concurrency(&checked) {
            return crate::vm::run_cooperative_vm(&program)
                .map(|(out, _exit)| out)
                .map_err(|mut e| {
                    let src = unit.attribute_frames(&mut e);
                    e.render(&src)
                });
        }
        vm_for(&program).run().map_err(|mut e| {
            let src = unit.attribute_frames(&mut e);
            e.render(&src)
        })
    })
}

/// Like [`treewalk_program`], but also returns `main`'s exit code (Batch-1 B). `phg run <file>` uses this
/// to set the process exit status; the stdout-only [`treewalk_program`] stays for the differential.
pub fn treewalk_program_exit(unit: &crate::loader::Unit) -> Result<(String, i64), String> {
    on_deep_stack(|| {
        let checked = check_and_expand(&unit.program, &unit.diag_src)?;
        foreign_runtime_gate(&checked)?;
        #[cfg(all(feature = "green", not(target_arch = "wasm32")))]
        if crate::ast::uses_concurrency(&checked) {
            return crate::interpreter::run_cooperative_interp(&checked).map_err(|mut e| {
                let src = unit.attribute_frames(&mut e);
                e.render(&src)
            });
        }
        interpret_main(&checked).map_err(|mut e| {
            let src = unit.attribute_frames(&mut e);
            e.render(&src)
        })
    })
}

/// Like [`run_program`], but also returns `main`'s exit code (Batch-1 B).
pub fn run_program_exit(unit: &crate::loader::Unit) -> Result<(String, i64), String> {
    on_deep_stack(|| {
        let (checked, reified) = check_and_expand_reified(&unit.program, &unit.diag_src)?;
        foreign_runtime_gate(&checked)?;
        let program = compile_with(&checked, &reified).map_err(|e| e.to_string())?;
        #[cfg(all(feature = "green", not(target_arch = "wasm32")))]
        if crate::ast::uses_concurrency(&checked) {
            return crate::vm::run_cooperative_vm(&program).map_err(|mut e| {
                let src = unit.attribute_frames(&mut e);
                e.render(&src)
            });
        }
        vm_for(&program).run_main().map_err(|mut e| {
            let src = unit.attribute_frames(&mut e);
            e.render(&src)
        })
    })
}

/// `check` on an already-loaded program.
pub fn check_program(prog: &Program, diag_src: &str) -> Result<String, String> {
    on_deep_stack(|| {
        check_and_expand(prog, diag_src)?;
        Ok("OK (type-checks clean)\n".to_string())
    })
}

/// `check --json` on an already-loaded program: machine-readable diagnostics for editor / LSP
/// integration (the seam `diagnostic.rs` calls out). Returns the JSON array (errors then warnings; see
/// [`crate::diagnostic::diagnostics_json`]) and whether any *error* was present, so the caller prints
/// the array to **stdout** and exits 0 (clean / warnings only) or 1 (errors) — `check`'s exit
/// semantics, but the array is always the output and nothing goes to stderr. Positions ride on each
/// diagnostic, so no `diag_src` is needed.
pub fn check_json_program(prog: &Program) -> (String, bool) {
    on_deep_stack(|| match crate::checker::check_resolutions(prog) {
        Ok((warnings, _html, _ufcs, _ovl, _reified)) => {
            (crate::diagnostic::diagnostics_json(&[], &warnings), false)
        }
        Err(errs) => (crate::diagnostic::diagnostics_json(&errs, &[]), true),
    })
}

/// `transpile` on an already-loaded program (emit PHP). Multi-namespace emission for a multi-package
/// project is S2c; S2b emits the existing flat form (correct for `package Main` / single-package).
pub fn transpile_program(prog: &Program, diag_src: &str) -> Result<String, String> {
    on_deep_stack(|| {
        let checked = check_and_expand(prog, diag_src)?;
        crate::transpile::emit(&checked)
    })
}

/// `serve` on an already-loaded program (M6 W4): type-check, build the request handler factory, then
/// run the blocking HTTP serve loop ([`crate::serve::serve_tcp`]) until the process is killed. Defaults
/// to the **bytecode VM** (faster than the tree-walker — measured ~2.3× lower end-to-end latency on a
/// representative handler; byte-identical via [`Vm::run_entry`] ≡ `call_named`); `tree_walker` selects
/// the interpreter oracle (`phg serve --tree-walker`, the
/// exact pre-VM behaviour). The single-threaded path runs on the 256 MB deep-stack worker (native-stack
/// headroom for re-entrant natives / the interpreter's deep recursion). Note: `--workers N` pool
/// threads are plain `std::thread::spawn` (default ~8 MB stack), not the deep-stack worker — the VM is
/// iterative so it is far less exposed than the tree-walker was, but a `--tree-walker` pool worker has
/// less headroom than the single-threaded path (pre-existing; unchanged by this slice).
pub fn serve_program(
    prog: &Program,
    diag_src: &str,
    addr: &str,
    timeout: Option<std::time::Duration>,
    profile: crate::profile::Profile,
    workers: usize,
    tree_walker: bool,
) -> Result<String, String> {
    on_deep_stack(|| {
        // Reified side-table is threaded into the VM compile (Invariant 6); the interp path ignores it.
        let (checked, reified) = check_and_expand_reified(prog, diag_src)?;
        let checked = std::sync::Arc::new(checked);
        let factory = if tree_walker {
            crate::serve::interp_factory(checked)
        } else {
            crate::serve::vm_factory(checked, std::sync::Arc::new(reified))
                .map_err(|e| e.to_string())?
        };
        crate::serve::serve_tcp(factory, addr, timeout, profile, workers)
            .map_err(|e| format!("serve: {e}"))?;
        Ok(String::new())
    })
}

/// Build a standalone executable for the host from `src`. `input_path` names the source (used to
/// derive the default output name); `out_path` overrides it. Validates the program first (never emits
/// a broken binary), then delegates to `bundle::cross::build_host`, which reuses this phg binary as
/// the stub and embeds `src` as a `.phorj` section. Returns a one-line success message.
pub fn cmd_build(
    input_path: &str,
    src: &str,
    out_path: Option<&str>,
    profile: crate::profile::Profile,
) -> Result<String, String> {
    cmd_check(src)?; // validate; emit nothing on failure
    let out = match out_path {
        Some(p) => std::path::PathBuf::from(p),
        None => {
            let stem = std::path::Path::new(input_path)
                .file_stem()
                .and_then(|s| s.to_str())
                .ok_or_else(|| format!("cannot derive output name from {input_path}"))?;
            std::path::PathBuf::from(stem)
        }
    };
    crate::bundle::cross::build_host(src, &out, profile)
}

/// `parse`: lex -> parse; dump the AST.
pub fn cmd_parse(src: &str) -> Result<String, String> {
    on_deep_stack(|| {
        let prog = lex_parse(src)?;
        Ok(format!("{prog:#?}\n"))
    })
}

/// `lex`: dump the token stream.
pub fn cmd_tokenize(src: &str) -> Result<String, String> {
    let tokens = lex(src).map_err(|e| e.to_string())?;
    let mut out = String::new();
    for t in tokens {
        out.push_str(&format!("{:?} @ {}:{}\n", t.kind, t.span.line, t.span.col));
    }
    Ok(out)
}

/// `lift`: read PHP source, emit a Phorj **draft** (the inverse of `transpile`). Best-effort and
/// review-required — the output is prefixed with a `// lifted (verify)` banner so the contract is
/// visible in the file itself. Anything outside the Tier-1 lift subset is a clear `lift …` error
/// (never a silent guess). No `on_deep_stack`: the lift parser has its own depth guard.
pub fn cmd_lift(src: &str) -> Result<String, String> {
    let phorj = crate::lift::lifter::lift_source(src)?;
    Ok(format!(
        "// lifted (verify) — a best-effort PHP->Phorj draft; review before trusting it.\n{phorj}"
    ))
}

/// `transpile`: lex -> parse -> check (gate) -> emit PHP source.
pub fn cmd_transpile(src: &str) -> Result<String, String> {
    on_deep_stack(|| {
        let prog = parse_checked(src)?;
        crate::transpile::emit(&prog)
    })
}

/// `disasm`: lex -> parse -> check (gate) -> compile -> dump the bytecode the VM will execute.
/// A read-only window onto the backend: per-function instruction listings and the program-level
/// descriptor tables. The op mnemonic is `Op`'s own `Debug`, *not* a hand-written match — so a new
/// `Op` variant appears here automatically with no second match surface to drift out of lockstep
/// (see memory `op-variant-match-coupling`); the per-op annotation is display-only with a `_`
/// fall-through, so an un-annotated new op simply shows no comment rather than failing to compile.
pub fn cmd_disassemble(src: &str) -> Result<String, String> {
    on_deep_stack(|| {
        let (prog, reified) = parse_checked_reified(src)?;
        let program = compile_with(&prog, &reified).map_err(|e| e.to_string())?;
        Ok(disasm_program(&program))
    })
}

/// Resolve a human-readable annotation for an index-carrying op (the value a `Const` loads, the
/// callee of a `Call`, the field/method/variant/class a member op names). Display-only: the `_`
/// arm covers every op that needs no comment, so this never has to track the full `Op` set.
fn annotate(op: &Op, chunk: &Chunk, p: &BytecodeProgram) -> Option<String> {
    match op {
        Op::Const(i) => chunk.consts.get(*i).map(|v| format!("{v:?}")),
        Op::Call(idx) => p
            .functions
            .get(*idx)
            .map(|f| format!("-> {}/{}", f.name, f.arity)),
        Op::GetField(i) => p.names.get(*i).map(|n| format!(".{n}")),
        Op::CallMethod(i, argc) => p.names.get(*i).map(|n| format!(".{n}(argc={argc})")),
        Op::CallNative(i, argc) => crate::native::registry()
            .get(*i)
            .map(|n| format!("-> {}.{}(argc={argc})", n.module, n.name)),
        Op::MakeEnum(i) | Op::MatchTag(i) => p
            .enum_descs
            .get(*i)
            .map(|d| format!("{}::{}", d.ty, d.variant)),
        Op::GetEnumField(i) => Some(format!("payload #{i}")),
        Op::MakeInstance(i) => p.class_descs.get(*i).map(|d| d.class.to_string()),
        _ => None,
    }
}

/// Format a whole [`BytecodeProgram`] as a disassembly listing. Descriptor tables are emitted only
/// when non-empty; the method table is sorted (HashMap iteration order is non-deterministic —
/// invariant #8) so the output is stable across runs.
fn disasm_program(p: &BytecodeProgram) -> String {
    let mut out = format!(
        "phg disassemble — {} function(s), main = #{}\n",
        p.functions.len(),
        p.main
    );
    if !p.enum_descs.is_empty() {
        out.push_str("\nenum descriptors:\n");
        for (i, d) in p.enum_descs.iter().enumerate() {
            out.push_str(&format!("  #{i} {}::{}/{}\n", d.ty, d.variant, d.arity));
        }
    }
    if !p.class_descs.is_empty() {
        out.push_str("\nclass descriptors:\n");
        for (i, d) in p.class_descs.iter().enumerate() {
            out.push_str(&format!(
                "  #{i} {} {{ {} }}\n",
                d.class,
                d.fields.join(", ")
            ));
        }
    }
    if !p.methods.is_empty() {
        out.push_str("\nmethods:\n");
        let mut entries: Vec<_> = p.methods.iter().collect();
        entries.sort();
        for ((class, name), idx) in entries {
            out.push_str(&format!("  {class}::{name} -> #{idx}\n"));
        }
    }
    for (fi, f) in p.functions.iter().enumerate() {
        out.push_str(&format!("\nfn #{fi} {}/{}:\n", f.name, f.arity));
        for (ip, op) in f.chunk.code.iter().enumerate() {
            let line = f.chunk.lines.get(ip).copied().unwrap_or(0);
            match annotate(op, &f.chunk, p) {
                Some(a) => out.push_str(&format!("  {ip:>4}  L{line:<4} {op:?}  ; {a}\n")),
                None => out.push_str(&format!("  {ip:>4}  L{line:<4} {op:?}\n")),
            }
        }
    }
    out
}

#[cfg(test)]
mod tests;
