//! `phg explain` sub-catalog: parent dispatch, attributes & routes, totality, fault intrinsics, exceptions
//! (M-Decomp, Invariant 13 — dispatched from `explain/mod.rs`; same
//! `text(code) -> Option<&'static str>` contract as `explain_config`).

/// Explanation text for a code in this band, or `None` when `code` is not this catalog's.
pub(super) fn text(code: &str) -> Option<&'static str> {
    Some(match code {
        "E-USING-NOT-CLOSABLE" => {
            "E-USING-NOT-CLOSABLE — a `using` header names a type that is not `Closable`.\n\n\
             `using (T h = …) { … }` (DEC-364) releases `h` on EVERY exit path from the block —\n\
             normal fall-through, `return`, `break`/`continue`, and a throw — by calling `h.close()`\n\
             in a synthesized `finally`. Nothing probes for that method at runtime, so the type must\n\
             prove it has one up front by implementing `Core.ClosableModule`'s `Closable`:\n\n\
             \x20 import Core.ClosableModule;\n\
             \x20 class Handle implements Closable {\n\
             \x20     function close(): void { … }\n\
             \x20 }\n\n\
             If the type is not yours to change, release it by hand with `try`/`finally`. If the\n\
             message says no `Closable` is in scope, the interface itself is missing — add the\n\
             `import Core.ClosableModule;` line.\n"
        }
        "E-USING-INFER" => {
            "E-USING-INFER — a `using` binding was written without an explicit type.\n\n\
             `using (var h = …)` is rejected: the declared type is what proves the binding can be\n\
             released (see `E-USING-NOT-CLOSABLE`), so unlike `var` elsewhere it cannot be inferred\n\
             from the initializer. Spell the type — `using (Connection db = new Connection(dsn)) { … }`.\n"
        }
        "E-USING-CLOSE-THROWS" => {
            "E-USING-CLOSE-THROWS — the released type's `close()` declares a checked exception that is\n\
             neither caught nor declared here.\n\n\
             A `using` block calls `close()` in a synthesized `finally`, so a fault that `close()`\n\
             declares can leave the enclosing function just like any other throwing call — and the\n\
             same rule applies: catch it, or declare it.\n\n\
             \x20 try { using (Handle h = open()) { … } } catch (IoError e) { … }   // caught\n\
             \x20 function f(): void throws IoError { using (Handle h = open()) { … } }  // declared\n\n\
             `Closable.close()` itself declares no `throws`, so this only arises when an implementor\n\
             adds them (interface conformance compares parameters and the return type, not `throws`).\n\
             This is the same auto-propagation rule DEC-257 applies to a throwing iterator's `foreach`.\n"
        }
        "E-PARENT-OUTSIDE-METHOD" => {
            "E-PARENT-OUTSIDE-METHOD — `parent` used outside an instance method or constructor.\n\n\
             `parent.m(…)` / `parent(A).m(…)` dispatch to an inherited method relative to the class that\n\
             *declares* the calling body. Outside an instance method or constructor — in a free\n\
             function, a `static` method, or a field/static initializer — there is no such context.\n\
             Move the call into an instance method, or pass the value you need as a parameter.\n"
        }
        "E-PARENT-NO-PARENT" => {
            "E-PARENT-NO-PARENT — `parent` in a class with no parents.\n\n\
             The enclosing class does not `extends` anything, so `parent` has nothing to dispatch to.\n\
             Add a parent class, or call the method directly.\n"
        }
        "E-PARENT-NOT-ANCESTOR" => {
            "E-PARENT-NOT-ANCESTOR — `parent(A)` names a class that is not an ancestor.\n\n\
             The qualified form `parent(A).m(…)` jumps to the ancestor `A`'s `m`. `A` must be a class\n\
             the current one transitively `extends`. Name a real ancestor, or use the immediate form\n\
             `parent.m(…)` (the nearest ancestor that declares `m`).\n"
        }
        "E-PARENT-NO-METHOD" => {
            "E-PARENT-NO-METHOD — no ancestor declares the named method.\n\n\
             `parent.m(…)` / `parent(A).m(…)` found no ancestor (resp. no `A`-reachable ancestor) that\n\
             declares or inherits a method `m`. Check the method name and the ancestor. (Parent\n\
             *constructor* forwarding is `parent.constructor(…)` — see `E-PARENT-CTOR-*`.)\n"
        }
        "E-PARENT-AMBIGUOUS" => {
            "E-PARENT-AMBIGUOUS — bare `parent.m()` is ambiguous under multiple inheritance.\n\n\
             The class has ≥2 parents that each resolve `m` to a different method, so the immediate\n\
             `parent.m(…)` cannot pick one. Qualify the ancestor you mean: `parent(SomeParent).m(…)`.\n"
        }
        "E-PARENT-CTOR-OUTSIDE" => {
            "E-PARENT-CTOR-OUTSIDE — `parent.constructor(…)` used outside a constructor body.\n\n\
             Forwarding to the parent constructor only makes sense while constructing the instance.\n\
             Call `parent.constructor(…);` from inside this class's `constructor(…)` body.\n"
        }
        "E-PARENT-CTOR-STMT" => {
            "E-PARENT-CTOR-STMT — `parent.constructor(…)` used as a value.\n\n\
             A constructor produces no value, so `parent.constructor(…)` must stand alone as a\n\
             statement (`parent.constructor(args);`) — it cannot be assigned, returned, or nested in\n\
             an expression.\n"
        }
        "E-PARENT-CTOR-MI" => {
            "E-PARENT-CTOR-MI — `parent.constructor(…)` under multiple inheritance.\n\n\
             The class has ≥2 parents, so the immediate `parent.constructor(…)` cannot pick one.\n\
             Per-parent constructor forwarding (`parent(P).constructor(…)` for each parent) lands with\n\
             multiple-inheritance support in a follow-up slice.\n"
        }
        "E-OVERLOAD-STATIC-MIX" => {
            "E-OVERLOAD-STATIC-MIX — overloads of one name mix `static` and instance declarations.\n\n\
             Every overload of a method name must be either all `static` or all instance methods. A\n\
             mixed set has no sound call form: `ClassName.m(args)` dispatches only the static overloads\n\
             while `x.m(args)` dispatches only the instance ones, so the checker would accept calls the\n\
             runtime rejects. (PHP also forbids a static and an instance method sharing a name.) Make\n\
             every overload `static`, or none of them, or rename one declaration.\n"
        }
        "E-ATTRIBUTE-ARG-TYPE" => {
            "E-ATTRIBUTE-ARG-TYPE — a user attribute argument has the wrong type.\n\n\
             A user attribute (`#[Tag(\"api\")]`) is applied like its constructor, so each argument must be\n\
             assignable to the matching `#[Attribute]` class constructor parameter — checked at COMPILE\n\
             time (PHP only fails when the attribute is reflected). e.g. `#[Tag(123)]` where `Tag` takes a\n\
             `string` is rejected here; pass a `string`.\n"
        }
        "E-ATTRIBUTE-ARITY" => {
            "E-ATTRIBUTE-ARITY — a user attribute was applied with the wrong number of arguments.\n\n\
             A user-defined attribute (a class marked `#[Attribute]`) is applied like a constructor call:\n\
             `#[Tag(\"api\")]` runs `Tag`'s constructor. The argument count must match the attribute class's\n\
             constructor parameters — this is checked at compile time (a stronger guarantee than PHP, which\n\
             only fails when the attribute is reflected at runtime).\n"
        }
        "E-ATTRIBUTE-ARGS" => {
            "E-ATTRIBUTE-ARGS — the `#[Attribute]` marker was given arguments it does not accept yet.\n\n\
             `#[Attribute]` (import Core.Runtime.Attribute) declares the class it sits on as a user-defined\n\
             attribute (DEC-194). The bare marker is accepted now — the class becomes an attribute valid on\n\
             all targets, non-repeatable. The `targets: […]` and `repeatable` arguments arrive in a later\n\
             slice; until then, use the bare `#[Attribute]`.\n"
        }
        "E-ATTR-TARGET" => {
            "E-ATTR-TARGET — an attribute is attached to an unsupported target.\n\n\
             A `#[…]` attribute may sit above a top-level `function` or `class` (DEC-194 slice 2a) — and\n\
             a `#[Route]` above a static method. Attributes on an enum, interface, trait, or import are\n\
             rejected at parse stage (their target slices are not built yet). A class attribute now\n\
             PARSES, but no attribute *targets* a class yet, so it is rejected at check stage until\n\
             user-declarable attributes land in a later DEC-194 slice.\n"
        }
        "E-FOREIGN-RUNTIME" => {
            "E-FOREIGN-RUNTIME — a program using foreign PHP `declare` symbols was run on a Rust backend.\n\n\
             `declare function …;` (M8.5 interop) describes an existing PHP function so Phorj can\n\
             type-check calls into it and transpile to real PHP. But foreign PHP only exists in the PHP\n\
             runtime — the interpreter and VM (`phg run`) have no PHP runtime, so they\n\
             cannot execute it. Such a program is PHP-target-only: `phg check` and `phg transpile` work,\n\
             but to run it, transpile and execute under PHP:\n\n    \
             phg transpile app.phg > app.php && php app.php\n\n\
             Pure Phorj (no `declare`) runs on all three backends byte-identically, as always.\n"
        }
        "E-UNKNOWN-ATTRIBUTE" => {
            "E-UNKNOWN-ATTRIBUTE — an unrecognized attribute name.\n\n\
             Only `#[Route(\"METHOD\", \"/path\")]` is given meaning today (M6 W2). The attribute grammar\n\
             accepts any `#[Name(args)]`, but every name other than `Route` is rejected so a typo can\n\
             never be silently ignored. Remove the attribute or correct the name.\n"
        }
        "E-ROUTE-ARGS" => {
            "E-ROUTE-ARGS — `#[Route]` has the wrong arguments.\n\n\
             `#[Route]` takes exactly two string-literal arguments: an HTTP method and a path —\n\
             `#[Route(\"GET\", r\"/users/{id}\")]`. A pattern containing `{name}` must be a RAW string\n\
             (`r\"…\"`); a normal string would interpolate `{name}` as a variable. Non-literal or\n\
             interpolated arguments are rejected (the route is read at compile time).\n"
        }
        "E-ROUTE-SPEC" => {
            "E-ROUTE-SPEC — `#[Route]`'s method or path is malformed.\n\n\
             The method must be a non-empty string and the path must start with `/` —\n\
             `#[Route(\"GET\", \"/health\")]`. This is a light sanity check, not a full URL grammar.\n"
        }
        "E-ROUTE-METHOD-STATIC" => {
            "E-ROUTE-METHOD-STATIC — a `#[Route]` method is not `static`.\n\n\
             A `#[Route]` on a class method requires `static`: `Http.autoRouter()` lowers it to\n\
             `function(req) => ClassName.method(req)`, a static call. An instance method has no routable\n\
             receiver yet (there is no controller-instance lifecycle this slice). Mark the handler\n\
             `static function …`, or move it to a free function.\n"
        }
        "E-ROUTE-HANDLER" => {
            "E-ROUTE-HANDLER — a `#[Route]` handler has the wrong shape.\n\n\
             A routed handler must take exactly one parameter (the `Request`) and declare a return type\n\
             (the `Response`): `function show(Request req) -> Response { … }`. The precise\n\
             `(Request) -> Response` typing is enforced where `Http.autoRouter()` lowers the route into\n\
             a `.route(…)` registration; this check catches the gross shape at the declaration.\n"
        }
        "E-MISSING-RETURN" => {
            "E-MISSING-RETURN — a function does not return a value on every path.\n\n\
             A function whose declared return type carries a value (`-> int`, `-> Shape`, …) must\n\
             `return` (or diverge) on *every* control-flow path. The classic leak is an `if` with no\n\
             `else`: the false branch falls through to the end. Add a trailing `return`, give the `if`\n\
             an `else` that also returns, or diverge (an infinite loop / a `-> never` call). A `-> void`\n\
             or `-> empty` function carries no value and is exempt.\n"
        }
        "E-MISSING-RETURN-TYPE" => {
            "E-MISSING-RETURN-TYPE — a function or method declares no return type.\n\n\
             Every function and method must declare its return type — including `main`. Add `-> void`\n\
             for a side-effecting function that returns nothing, `-> empty` to return the holdable\n\
             empty value, or the concrete type it returns (`-> int`, `-> Shape`, …). Constructors have\n\
             no return slot and property hooks are typed by their property, so neither needs one;\n\
             expression-body lambdas (`function(x) => e`) infer their return from the expression.\n"
        }
        "E-VOID-CAPTURE" => {
            "E-VOID-CAPTURE — a `void` value cannot be captured.\n\n\
             `void` is the type of an expression that produces *nothing* (a side-effecting call like\n\
             `Output.printLine(…)`), so there is nothing to bind: `var x = note(\"hi\");` is rejected.\n\
             Call it as a statement instead (drop the binding). If you genuinely need to hold the\n\
             empty value — e.g. to satisfy a generic slot — annotate it `empty` (`empty x = note(…);`):\n\
             `void` widens to the holdable `empty`.\n"
        }
        "E-NEVER-RETURN" => {
            "E-NEVER-RETURN — a `-> never` function can return normally.\n\n\
             `never` is the bottom type: a function annotated `-> never` promises it never returns —\n\
             it must diverge on every path (today, an infinite loop or a call to another `never`\n\
             function; once `throw` lands, also by throwing). This body can fall through and return.\n\
             Make it diverge, or drop the `never` return type.\n"
        }
        "W-UNREACHABLE" => {
            "W-UNREACHABLE — a statement can never be reached (warning).\n\n\
             A preceding statement always returns or diverges (a `return`, an infinite loop, or a call\n\
             to a `-> never` function), so the flagged statement is dead code. This is a non-fatal\n\
             lint — remove the unreachable statements. It never blocks the build.\n"
        }
        "W-MATCH-UNREACHABLE" => {
            "W-MATCH-UNREACHABLE — a `match` arm can never be reached (warning).\n\n\
             Either an earlier arm is a catch-all (`_` or a bare identifier binding, which matches\n\
             everything) so later arms are dead, or this arm duplicates an earlier literal/variant/type\n\
             pattern. Reorder so the catch-all is last, or remove the duplicate. Non-fatal lint.\n"
        }
        "E-PROPAGATE-POSITION" => {
            "E-PROPAGATE-POSITION — `?` used outside a let-initializer.\n\n\
             The `?` error-propagation operator is allowed only as the *whole* initializer of a binding\n\
             (`int a = mayFail()?;`). It is not allowed nested in a larger expression (`g(f()?)`) or in a\n\
             `return` — PHP, the transpile target, cannot return from the caller inside an expression.\n\
             Bind the call's result to a local first, then handle it (M-faults).\n"
        }
        "E-PROPAGATE-CONTEXT" => {
            "E-PROPAGATE-CONTEXT — `?` in a function that can't propagate the error.\n\n\
             `?` unwraps an `Ok` or early-returns the `Err`, so it requires a `Result`-shaped operand\n\
             (an enum with `Ok`/`Err` variants) AND an enclosing function that returns that same\n\
             `Result`. Declare the function to return `Result<…>`, or handle the value with a `match`.\n"
        }
        "E-PROPAGATE-ERR" => {
            "E-PROPAGATE-ERR — `?` propagates an incompatible error type.\n\n\
             The operand's `Err` payload type must be assignable to the enclosing function's `Err`\n\
             payload type (it is the value `?` early-returns). Widen the function's error type, or map\n\
             the error before propagating.\n"
        }
        "E-RESERVED-INTRINSIC" => {
            "E-RESERVED-INTRINSIC — a reserved built-in name was redefined.\n\n\
             `panic`, `todo`, `unreachable`, and `assert` are built-in fault intrinsics (M-faults) and\n\
             cannot be declared as user functions. Rename your function.\n"
        }
        "E-INTRINSIC-LITERAL" => {
            "E-INTRINSIC-LITERAL — a fault intrinsic's message must be a string literal.\n\n\
             `panic(\"…\")` and `assert(cond, \"…\")` bake their message at compile time, so it must be a\n\
             plain string literal — no interpolation or computed expression (yet). Use a literal, or\n\
             compute the message into a local for a future dynamic form.\n"
        }
        "E-THROW-TYPE" => {
            "E-THROW-TYPE — only an `Error` value may be thrown or declared.\n\n\
             `throw e` requires `e` to be a value whose type implements the built-in `Error` marker\n\
             (`class Foo implements Error { … }`), and a `throws T` declaration requires the same of\n\
             `T`. You cannot throw a primitive, enum, or arbitrary object.\n"
        }
        "E-THROW-UNDECLARED" => {
            "E-THROW-UNDECLARED — a thrown exception is neither caught nor declared.\n\n\
             A checked exception must be discharged: wrap the `throw` (or the throwing call) in a\n\
             `try { … } catch (T e) { … }`, or add `throws T` to the enclosing function so callers\n\
             handle it. Phorj enforces this at compile time — nothing leaks silently.\n"
        }
        "E-CALL-UNHANDLED" => {
            "E-CALL-UNHANDLED — a call can throw a checked exception that isn't handled.\n\n\
             Calling a `throws T` function obliges the caller to handle `T`: catch it in an enclosing\n\
             `try`/`catch`, or propagate it with `?` AND declare `throws T` on the enclosing function.\n\
             A bare call may not silently let the exception escape.\n"
        }
        "E-UNCAUGHT-THROW" => {
            "E-UNCAUGHT-THROW — an exception escapes `main`.\n\n\
             `main` is the program entry point: it may not declare `throws`, and every exception it\n\
             (or anything it calls) can raise must be caught before it escapes. Wrap the throwing code\n\
             in a `try { … } catch (T e) { … }` inside `main`.\n"
        }
        "E-THROWS-TOO-BROAD" => {
            "E-THROWS-TOO-BROAD — `throws Error` is too broad.\n\n\
             Declare the *specific* exception type(s) a function throws (`throws BadInput`), not the\n\
             bare `Error` root, so callers know exactly what to catch. A `catch (Error e)` is still\n\
             allowed — catching broad is fine; declaring broad is not.\n"
        }
        "E-CATCH-TYPE" => {
            "E-CATCH-TYPE — a `catch` clause names a non-`Error` type.\n\n\
             A `catch (T e)` requires `T` (or every member of a union `catch (A | B e)`) to implement\n\
             the built-in `Error` marker — you can only catch what can be thrown. Catching the `Error`\n\
             base itself is allowed (it matches every exception).\n"
        }
        "W-CATCH-UNREACHABLE" => {
            "W-CATCH-UNREACHABLE — a `catch` clause can never run (warning).\n\n\
             An earlier clause in the same `try` already catches this type (it is the same as, or a\n\
             supertype of, this one), so control never reaches it. Remove the dead clause, or reorder\n\
             so the more specific type comes first. This is a lint — it never fails the build.\n"
        }
        _ => return None,
    })
}
