# Interactive debugger — `phg debug`

Phorj ships an **interpreter-only** interactive debugger with two frontends over one shared engine:
a terminal **REPL** and a **DAP** server for editors (VS Code / JetBrains). It is **Dev-only** — the
value-inspection machinery is gated to the Dev profile, never a `Release` `phg build` artifact.

> Debugging it interactive, so it has no byte-identical "Ok" output and isn't a runnable example
> under the differential sweep — this walkthrough is the surface.

Why interpreter-only? The bytecode VM has no source-line/local-name table, so stepping it would need
a debug-symbol subproject. The parity spine guarantees `interpreter ≡ VM ≡ real PHP`, so a debug session on
the interpreter **provably reflects** the other backends (the same rationale as the value-dump S3).

## Terminal REPL — `phg debug <file>`

The session starts paused at the first statement. Commands (stderr for the UI, program output on
stdout):

| Command | Alias | Effect |
|---|---|---|
| `step` | `s` | step into (pause at the next statement, any depth) |
| `next` | `n` | step over (skip into calls) |
| `stepout` | `o` | run until the current function returns |
| `continue` | `c` | run until the next breakpoint |
| `break <line>` | `b` | set a line breakpoint |
| `clear <line>` | `d` | remove a breakpoint |
| `locals` | `l` | print the current frame's locals (secure renderer — `Secret` redacted) |
| `backtrace` | `bt` | print the call stack |
| `quit` | `q` | detach and let the program finish |

```
$ phg debug program.phg
phg debug — interpreter debugger. Paused at the first statement; type `help` for commands.
⏸ paused at line 8 (depth 1)
(phg-dbg) s
⏸ paused at line 9 (depth 1)
(phg-dbg) s
⏸ paused at line 4 (depth 2)
(phg-dbg) l
  a = 3
  b = 4
(phg-dbg) c
y = 7
```

Locals go through the same secure renderer as the value-dump (S2/S3): a `Secret<T>` shows
`Secret(<redacted>)`, and depth/size are capped.

## Editor debugging — `phg debug --dap <file>`

`--dap` runs a **Debug Adapter Protocol** server on stdio (`Content-Length`-framed JSON, the same
transport as the LSP), so an editor's debug UI can drive it: set line breakpoints, launch, hit
breakpoints, view the call stack and locals, and step. Point your editor's DAP client at
`phg debug --dap <file>`.

v1 supports `initialize`, `launch`, `setBreakpoints`, `configurationDone`, `threads`, `stackTrace`,
`scopes`, `variables`, `continue`, `next`, `stepIn`, `stepOut`, and `disconnect`, emitting
`initialized`, `stopped`, `output`, `exited`, and `terminated`. `launch` runs to the first
breakpoint (editor convention).

## Deferred (v1 scope)

Conditional breakpoints, watchpoints, async `pause`, multiple threads, and VM stepping are out of
v1 scope (see `KNOWN_ISSUES.md`).
