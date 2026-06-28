# Foreign PHP interop (`declare`) — M8.5

These examples are **PHP-target-only**. They use `declare` to describe existing PHP functions/classes
so Phorge can type-check calls into them and transpile to idiomatic PHP that uses them directly. This is
the **migration bridge** — adopt Phorge incrementally over a PHP codebase.

Because foreign PHP only exists in the PHP runtime, the Rust backends (`phg run` / `phg runvm`) **refuse**
a program that uses `declare` (`E-FOREIGN-RUNTIME`). Run them by transpiling:

```sh
phg transpile builtins.phg > out.php && php out.php
```

The pure-Phorge byte-identity spine (`run ≡ runvm ≡ real PHP`) is untouched: these programs are
quarantined from the `differential.rs` example gate and validated instead by `tests/interop.rs`
(transpile → real PHP → golden output, the sibling `.out`).

| file | shows |
|------|-------|
| `builtins.phg` | foreign PHP free functions: `declare function strtoupper(string) -> string;` etc.; calls transpile to `\strtoupper(...)`; the `declare` lines emit no PHP. Foreign results compose with native Phorge + `Core.*`. |

## How it works

- `declare function name(params) -> ret;` — a bodyless signature for an existing PHP function. Its name
  is the **real PHP name** (snake_case like `str_repeat` is fine — the camelCase rule is waived for
  foreign symbols, since the name is emitted verbatim). It produces no PHP definition; a call emits the
  global form `\name(...)` so it resolves to the PHP builtin even inside a namespace.
- `check` type-checks calls against the declared signatures; `transpile` emits the PHP; `run`/`runvm`
  refuse (foreign code needs the PHP runtime).

`declare class` (foreign PHP classes) and `.d.phg` declaration files are later M8.5 slices.
