//! `Core.DebugModule` (DEC-238) — the beautiful dumper. ONE function carrying both products (developer-
//! ruled): `Debug.dump(x)` renders deeply (the versioned v1 format in `ext/debug/natives.rs`), PRINTS,
//! and returns `Dumped<T>` — `.value()` is the pass-through, `.text()` the captured rendering.
//! `Debug.dd(x)` (dump + exit 1) and `Runtime.exit` land in slice 2. Nothing in the wind: only
//! reachable through `import Core.DebugModule`.
//!
//! DEC-273 wave 2: colocated with the `debug` extension. Compiled UNCONDITIONALLY (the
//! `CORE_MODULES` const array references it on every build; on a gated build the disabled-import
//! gate rejects the import before this prelude could matter).

pub const PRELUDE: &str = r#"
import Core.Native.Debug as NativeDebug;
import Core.Output;
import Core.Runtime;

// The dump result: BOTH the pass-through value and the rendering, explicitly.
class Dumped<T> {
  constructor(private T v, private string s) {}
  function value(): T { return this.v; }
  function text(): string { return this.s; }
}

class Debug {
  // Render + PRINT + carry: `int t = Debug.dump(price).value() * qty;` flows on;
  // `string snap = Debug.dump(cfg).text();` captures (already printed).
  static function dump<T>(T v): Dumped<T> {
    string s = NativeDebug.render(v);
    Output.printLine(s);
    return new Dumped(v, s);
  }
  // dump-and-die (the debugging convention): print the rendering, then a CLEAN exit 1 (deliberate
  // abort — never a stack trace; that's `panic`'s job).
  static function dd<T>(T v): never {
    discard Debug.dump(v);
    Runtime.exit(1);
  }
}
"#;
