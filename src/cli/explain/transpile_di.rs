//! `phg explain` sub-catalog: destructuring & tuples, transpile ladder, concurrency, loops, new, dependency injection
//! (M-Decomp, Invariant 13 — dispatched from `explain/mod.rs`; same
//! `text(code) -> Option<&'static str>` contract as `explain_config`).

/// Explanation text for a code in this band, or `None` when `code` is not this catalog's.
pub(super) fn text(code: &str) -> Option<&'static str> {
    Some(match code {
        "E-DESTRUCTURE-TYPE" => {
            "E-DESTRUCTURE-TYPE — a struct destructuring's value is not the named class.\n\n\
             `var Point { x, y } = p;` (Phase 1 slice 5) requires `p` to be a `Point` (or a subtype) so\n\
             the binding always succeeds. Destructure the value at its own type, or `match` on it if it\n\
             is a union/interface whose concrete type isn't statically known.\n"
        }
        "E-DESTRUCTURE-NOT-CLASS" => {
            "E-DESTRUCTURE-NOT-CLASS — a struct destructuring's head is not a class.\n\n\
             `var Name { … } = e;` destructures a class instance's fields, so `Name` must be a declared\n\
             class. To destructure a list, use the list form `var [a, b] = e else { … };`.\n"
        }
        "E-DESTRUCTURE-FIELD-UNKNOWN" => {
            "E-DESTRUCTURE-FIELD-UNKNOWN — a struct destructuring names a field the class does not have.\n\n\
             Each `field` (or `field: binding`) in `var Point { x, y } = p;` must be a field declared on\n\
             the class (including inherited fields). Bind only declared fields.\n"
        }
        "E-DESTRUCTURE-NOT-LIST" => {
            "E-DESTRUCTURE-NOT-LIST — a list destructuring's value is not a list.\n\n\
             `var [a, b] = e else { … };` requires `e` to be a `List<T>` or a fixed-length `[T; N]`. To\n\
             destructure a class instance, use the struct form `var Type { … } = e;`.\n"
        }
        "E-DESTRUCTURE-NEEDS-ELSE" => {
            "E-DESTRUCTURE-NEEDS-ELSE — a refutable list destructuring has no `else`.\n\n\
             A `List<T>` carries no static length, so `var [a, b] = xs;` can fail at runtime. It must\n\
             carry an `else { … }` that bails out (returns / throws / breaks / continues) when the\n\
             length doesn't match — the Swift `guard let` model. (A fixed-length `[T; N]` whose length\n\
             matches the binder count is irrefutable and takes no `else`.)\n"
        }
        "E-DESTRUCTURE-ELSE-IRREFUTABLE" => {
            "E-DESTRUCTURE-ELSE-IRREFUTABLE — an irrefutable destructuring has an `else`.\n\n\
             A struct destructuring, and a list destructuring over a length-matching `[T; N]`, always\n\
             succeed — so they cannot have an `else`. Remove it; the binding is unconditional.\n"
        }
        "E-DESTRUCTURE-ELSE-FALLTHROUGH" => {
            "E-DESTRUCTURE-ELSE-FALLTHROUGH — a destructuring `else` can fall through.\n\n\
             When the refutable destructuring fails, its binders are never created, so control must not\n\
             continue past the `else`. End every path of the `else` with `return` / `throw` / `break` /\n\
             `continue` (it is a bail-out block, like a `guard let … else`).\n"
        }
        "E-DESTRUCTURE-DUP-BIND" => {
            "E-DESTRUCTURE-DUP-BIND — a destructuring binds the same name twice.\n\n\
             Each binder in a destructuring must be distinct: `var [a, a] = xs` and `var Point { x, x }\n\
             = p` are errors. Rename one binding (`var Point { x, y: x2 } = p`).\n"
        }
        "E-FIXEDLIST-DESTRUCTURE-LEN" => {
            "E-FIXEDLIST-DESTRUCTURE-LEN — a list destructuring's arity differs from the fixed length.\n\n\
             Destructuring a fixed-length `[T; N]` is irrefutable only when the pattern binds exactly\n\
             `N` elements: `var [a, b] = pair;` needs `pair: [T; 2]`. Bind exactly `N` elements, or\n\
             destructure a `List<T>` with an `else` if the length is not statically known.\n"
        }
        "E-TUPLE-DESTRUCTURE-LEN" => {
            "E-TUPLE-DESTRUCTURE-LEN — a tuple destructuring binds a different number of elements than the tuple has.\n\n\
             A tuple's arity is statically known, so `var (a, b) = t;` requires `t` to be a 2-tuple.\n\
             Bind exactly one name per tuple position — add or remove binders so the count matches the\n\
             tuple type `(A, B, …)` on the right (DEC-288).\n"
        }
        "E-DESTRUCTURE-NOT-TUPLE" => {
            "E-DESTRUCTURE-NOT-TUPLE — a tuple destructuring's value is not a tuple.\n\n\
             `var (a, b) = …` requires the right-hand side to be a tuple `(A, B)`. Destructure a list\n\
             with `var [a, b] = …` (mandatory `else` on a `List<T>`), or a class with\n\
             `var Type { x, y } = …` (DEC-288).\n"
        }
        "E-CONCURRENCY-NO-PHP" => {
            "E-CONCURRENCY-NO-PHP — green threads (`spawn` / channels) cannot be transpiled to PHP.\n\n\
             PHP has no green threads, and a synchronous lowering would make a concurrent program\n\
             behave differently under PHP than on the Phorj VM/interpreter — breaking the byte-identical\n\
             spine. So `spawn`/channel programs run on `phg run` only (byte-identically),\n\
             and `phg transpile` rejects them rather than emitting misleading PHP (M6 W4).\n"
        }
        "E-TRANSPILE-UNCHECKED" => {
            "E-TRANSPILE-UNCHECKED — an `#[UncheckedOverflow]` function cannot be transpiled to PHP.\n\n\
             `#[UncheckedOverflow]` (import Core.Runtime.Integer.UncheckedOverflow) makes a function's int `+`/`-`/`*`/unary-`-` WRAP on\n\
             overflow (two's-complement, like C/Rust) instead of faulting. PHP has no wrapping int — it\n\
             silently promotes an overflowing int to float — so a transpiled `#[UncheckedOverflow]` program would\n\
             behave differently under PHP than on the Phorj VM/interpreter, breaking the byte-identical\n\
             spine (§14 LADDER). So `#[UncheckedOverflow]` functions run on `phg run` / `phg run --tree-walker`\n\
             only (byte-identically), and `phg transpile` rejects them rather than emitting misleading\n\
             PHP. If you want PHP-transpilable code, drop `#[UncheckedOverflow]` (the default faults on overflow),\n\
             or handle overflow explicitly with `Math.tryAdd/trySub/tryMul(a, b): int?`.\n"
        }
        "E-TRANSPILE-DB" => {
            "E-TRANSPILE-DB — a program importing `Core.DatabaseModule` cannot be transpiled to PHP.\n\n\
             `Core.DatabaseModule` is native-only: it runs live database I/O through the phorj drivers (bundled\n\
             SQLite, Postgres), and live I/O cannot be byte-identical across those drivers and PHP\n\
             PDO — connection behaviour, error text, and type coercions all differ. Rather than emit\n\
             a PHP program that silently diverges from what `phg run` does, `phg transpile` refuses\n\
             (§14 LADDER: no silent semantic downgrade). Run database programs with `phg run` /\n\
             `phg run`, or serve them with `phg serve`.\n"
        }
        "E-TRANSPILE-MAIL" => {
            "E-TRANSPILE-MAIL — a program importing `Core.Mail` cannot be transpiled to PHP.\n\n\
             `Core.Mail` is native-only (DEC-223): PHP's stdlib `mail()` has no SMTP authentication,\n\
             no TLS, and is header-injection-prone, so there is no faithful safe mapping — any\n\
             attempt (e.g. text-only mails through mail()) would silently drop auth/TLS/attachments,\n\
             a forbidden semantic downgrade (§14 LADDER). Run mail programs with `phg run`, or keep\n\
             the mail-sending part native and transpile only the rest of your program.\n"
        }
        "E-TRANSPILE-SESSION" => {
            "E-TRANSPILE-SESSION — a program importing `Core.SessionModule` is native-only (PERMANENT, DEC-313).\n\n\
             Sessions cannot be byte-identically transpiled: ids are OS-entropy random (observable\n\
             via `Session.id()`), the idle TTL reads the wall clock (not the freezable `Core.Time`\n\
             one), and the persistent in-process store matches `phg serve`'s long-lived process —\n\
             PHP's per-request `$_SESSION` is a different model. Refusing beats silent divergence\n\
             (§14 LADDER). Run session programs with `phg run` / `phg serve`.\n"
        }
        "E-TRANSPILE-UNICODE" => {
            "E-TRANSPILE-UNICODE — a call to a native-only `Core.String` Unicode function cannot be transpiled to PHP.\n\n\
             `String.unicodeUpper`/`unicodeLower`/`graphemeLength`/`graphemes` need PHP's\n\
             mbstring/intl ini extensions, which the transpile rules forbid (no ini-dependent\n\
             output) — refusing beats a silently-diverging mapping (THE LADDER RULE, DEC-256).\n\
             The gate is per-FUNCTION: importing `Core.String` stays transpilable, and the\n\
             codepoint tier (`String.codepointLength`/`codepoints`, PCRE/byte-decode based)\n\
             transpiles. Run programs using the native-only tier with `phg run`.\n"
        }
        "E-TRANSPILE-HTTPCLIENT" => {
            "E-TRANSPILE-HTTPCLIENT — a program importing `Core.HttpClientModule` cannot be transpiled to PHP.\n\n\
             `Core.HttpClientModule` is native-only: live network I/O cannot be byte-identical between the\n\
             phorj client and any PHP mapping (curl/file_get_contents differ in redirects, TLS stacks,\n\
             timeout semantics and error text), so `phg transpile` refuses rather than emitting a\n\
             silently-diverging program (§14 LADDER). A faithful curl-mapping is a recorded future\n\
             lift. Run HTTP-client programs with `phg run`.\n"
        }
        "E-MODULE-UNAVAILABLE" => {
            "E-MODULE-UNAVAILABLE — RETIRED (DEC-273; superseded by E-EXTENSION-DISABLED).\n\n\
             Feature-gated modules are EXTENSIONS now: importing one on a build that compiled it\n\
             out reports `E-EXTENSION-DISABLED`, naming the extension and the cargo flag to add.\n\
             See `phg explain E-EXTENSION-DISABLED` and `phg extensions`.\n"
        }
        "E-EXTENSION-DISABLED" => {
            "E-EXTENSION-DISABLED — the imported module belongs to an extension this `phg` build\n\
             compiled out.\n\n\
             DEC-273: the minimal CORE is what the language cannot function without; everything\n\
             else — Db, Mail, HttpClient, Regex, Ini, … — is an EXTENSION: Rust + JIT exactly like\n\
             the core (never slower; the flag gates BUILD INCLUSION only). The default build is\n\
             batteries-included, so a stock `phg` has every Default-tier extension; this binary\n\
             was built with `--no-default-features` or a reduced set. The error names the cargo\n\
             flag to add (`cargo build --features <name>`); `phg extensions` lists every\n\
             extension, its tier, its flag, and whether THIS build carries it.\n"
        }
        "E-UNCHECKED-ARGS" => {
            "E-UNCHECKED-ARGS — `#[UncheckedOverflow]` was given arguments.\n\n\
             `#[UncheckedOverflow]` is a bare marker attribute — it takes no arguments. Write it as `#[UncheckedOverflow]`\n\
             directly above a top-level `function` (with `import Core.Runtime.Integer.UncheckedOverflow;`).\n"
        }
        "E-SPAWN-NOT-CALL" => {
            "E-SPAWN-NOT-CALL — `spawn` was applied to something that is not a call.\n\n\
             `spawn` starts a green task from a function/method call: `spawn work(x)`. It cannot wrap a\n\
             plain value or expression — wrap the work in a function and `spawn` the call (M6 W4).\n"
        }
        "E-SPAWN-VOID" => {
            "E-SPAWN-VOID — a `spawn`ned call returns no value.\n\n\
             `spawn f()` evaluates to a `Task<T>` whose `join()` yields the call's result, so the call\n\
             must return a value. A `void`/`never` call has nothing to join. Fire-and-forget void tasks\n\
             are a follow-up (M6 W4).\n"
        }
        "E-CHANNEL-ANNOTATION" => {
            "E-CHANNEL-ANNOTATION — `Channel.create()` needs a `Channel<T>` annotation.\n\n\
             The channel constructor takes no argument, so its element type cannot be inferred. Bind it\n\
             to an annotated local first: `Channel<int> ch = Channel.create();` (M6 W4).\n"
        }
        "E-CHANNEL-NEW-ARITY" => {
            "E-CHANNEL-NEW-ARITY — `Channel.create()` was given arguments.\n\n\
             The channel constructor takes none — `Channel<int> ch = Channel.create();`. The element\n\
             type comes from the `Channel<T>` annotation, not an argument (M6 W4).\n"
        }
        "E-CHANNEL-NEW-TYPE" => {
            "E-CHANNEL-NEW-TYPE — `Channel.create()` bound to a non-`Channel` type.\n\n\
             `Channel.create()` produces a `Channel<T>`; the binding's declared type must be a\n\
             `Channel<…>` (M6 W4).\n"
        }
        "E-CONCURRENCY-METHOD" => {
            "E-CONCURRENCY-METHOD — unknown method on a concurrency handle.\n\n\
             `Channel<T>` has `send(v)` and `receive()`; `Task<T>` has `join()`; the channel constructor is\n\
             `Channel.create()`. No other built-in method exists on these handles (M6 W4).\n"
        }
        "E-CONCURRENCY-ARITY" => {
            "E-CONCURRENCY-ARITY — a concurrency-handle method got the wrong number of arguments.\n\n\
             `ch.send(v)` takes exactly one argument; `ch.receive()` and `t.join()` take none (M6 W4).\n"
        }
        // ── M-DX S1: audit-gap codes (previously emitted with no `phg explain` entry) ──
        "E-BREAK-OUTSIDE-LOOP" => {
            "E-BREAK-OUTSIDE-LOOP — `break` was used outside a loop.\n\n\
             `break` exits the nearest enclosing `while`/`for` loop, so it is only meaningful inside\n\
             one. A `break` in a plain block, a function body, or a `match` arm has no loop to exit.\n\
             Remove it, or move the logic into a loop.\n"
        }
        "E-CONTINUE-OUTSIDE-LOOP" => {
            "E-CONTINUE-OUTSIDE-LOOP — `continue` was used outside a loop.\n\n\
             `continue` skips to the next iteration of the nearest enclosing `while`/`for` loop, so it\n\
             is only meaningful inside one. Remove it, or move the logic into a loop.\n"
        }
        "E-NEW-REQUIRED" => {
            "E-NEW-REQUIRED — a class/enum-variant construction is missing `new`.\n\n\
             Construction is explicit: write `new ClassName(…)` (and `new Variant(…)` for an enum\n\
             variant with fields). A bare `ClassName(…)` is not a call — add `new`.\n"
        }
        "E-NEW-ON-NONCONSTRUCT" => {
            "E-NEW-ON-NONCONSTRUCT — `new` was applied to something that is not constructible.\n\n\
             `new` constructs a class instance or an enum variant. Applying it to a function, a\n\
             built-in type, a variable, or an unknown name is rejected. Call a function without `new`;\n\
             construct only declared classes / enum variants.\n"
        }
        "E-DI-MISSING" => {
            "E-DI-MISSING — dependency injection could not find an `#[Injectable]` provider.\n\n\
             `inject<T>()` builds T's dependency graph from `#[Injectable]` classes at compile time.\n\
             This fires when T (or one of its constructor-parameter types) is not injectable: mark the\n\
             class `#[Injectable]`, or provide a single `#[Injectable]` implementation for an interface\n\
             dependency. In v1 every constructor parameter of an injectable must itself be injectable\n\
             (config-value provision via `#[Provides]` is a later slice).\n"
        }
        "E-DI-AMBIGUOUS" => {
            "E-DI-AMBIGUOUS — an interface dependency has more than one `#[Injectable]` implementation.\n\n\
             A single-implementation interface auto-binds to its one injectable implementor. When two or\n\
             more injectable classes implement the interface, the resolver cannot choose. In v1, provide\n\
             exactly one `#[Injectable]` implementation (binding qualifiers to disambiguate multiple\n\
             implementations are a later slice).\n"
        }
        "E-DI-CYCLE" => {
            "E-DI-CYCLE — the injection dependency graph has a cycle.\n\n\
             Constructor injection requires an acyclic graph (each type is built once, dependencies\n\
             first). A cycle (A needs B, B needs A) cannot be constructed. Break the cycle — e.g. extract\n\
             a shared dependency, or invert one edge. (Field-injection cycle-breaking is not in v1.)\n"
        }
        "E-INJECT-NO-TYPE" => {
            "E-INJECT-NO-TYPE — `inject()` could not infer a target type from its position.\n\n\
             The annotation-driven `inject()` draws its target from a typed declaration\n\
             (`App app = inject();`), a typed `return`, or a lambda return type. It has no source in a\n\
             `var` binding, a discard, or a call argument — there, name the type: `inject<App>()`.\n"
        }
        "E-TRANSIENT-ARGS" => {
            "E-TRANSIENT-ARGS — `#[Transient]` was given arguments.\n\n\
             The `#[Transient]` marker takes no arguments — write it bare on the class. It opts the class\n\
             out of the default-shared DI lifetime, so a fresh instance is built at each injection point.\n"
        }
        "E-PROVIDES-TARGET" => {
            "E-PROVIDES-TARGET — `#[Provides]` is not on a valid target.\n\n\
             A `#[Provides]` factory must be a `static` method with a declared return type — the return\n\
             type names the type it provides, and it is resolved without an instance. Make the method\n\
             `static` and annotate its return type: `static function make(): Database { … }`.\n"
        }
        "E-PROVIDES-ARGS" => {
            "E-PROVIDES-ARGS — `#[Provides]` was given arguments.\n\n\
             The `#[Provides]` marker takes no arguments — write it bare on a `static` factory method.\n\
             The provided type is the method's return type; its own parameters are autowired.\n"
        }
        "E-DI-NO-IMPORT" => {
            "E-DI-NO-IMPORT — the `inject` composition root was used without importing `Core.DependencyInjection`.\n\n\
             `inject` is a `Core.DependencyInjection` member, not a keyword — nothing is available in the wind. Import it\n\
             to use the bare form (`import Core.DependencyInjection.inject;` → `inject<App>()` / `inject()`), or write it\n\
             qualified with the module import (`import Core.DependencyInjection;` → `DependencyInjection.inject<App>()` / `DependencyInjection.inject()`).\n\
             The DI attributes follow the same rule: `#[DependencyInjection.Injectable]` with `import Core.DependencyInjection;`, or bare\n\
             `#[Injectable]` with `import Core.DependencyInjection.Injectable;`.\n"
        }
        _ => return None,
    })
}
