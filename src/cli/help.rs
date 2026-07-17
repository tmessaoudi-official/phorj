//! CLI surface text + argument shaping: version/help, `phg vendor`, source resolution.

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
                    usage:\n  phg serve <file> [--address 127.0.0.1:8080] [--timeout SECONDS] [--workers N] [--tree-walker]\n\n\
                    options:\n  \
                    --address ADDR       host:port to bind (default 127.0.0.1:8080)\n  \
                    --timeout SECONDS  per-connection read/write timeout; 0 = none (default 30)\n  \
                    --workers N        request concurrency; 1 = single-threaded (default = CPU cores)\n  \
                    --tree-walker      run requests on the interpreter oracle, not the (default) VM\n  \
                    --dev              rich HTML error page on an uncaught fault (DEV ONLY; prod = bare 500)\n\n\
                    examples:\n  \
                    phg serve examples/web/server.phg\n  \
                    phg serve app.phg --address 0.0.0.0:3000 --timeout 15 --workers 8\n"
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
