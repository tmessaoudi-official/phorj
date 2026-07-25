//! `phg explain` sub-catalog: decimals, variadics, named args, default params, interfaces, class inheritance, traits, abstract/override
//! (M-Decomp, Invariant 13 — dispatched from `explain/mod.rs`; same
//! `text(code) -> Option<&'static str>` contract as `explain_config`).

/// Explanation text for a code in this band, or `None` when `code` is not this catalog's.
pub(super) fn text(code: &str) -> Option<&'static str> {
    Some(match code {
        "E-DECIMAL-FLOAT-MIX" => {
            "E-DECIMAL-FLOAT-MIX — `decimal` and `float` were mixed in one operation.\n\n\
             `decimal` is exact fixed-point (money/fixed-point math); `float` is binary IEEE-754\n\
             (inexact for values like `0.1`). Phorj keeps them as **distinct** types with NO\n\
             implicit coercion — mixing a `float` into money is exactly the bug `decimal` exists to\n\
             prevent. So `1.50d + 1.5`, or comparing a `decimal` with a `float`, is rejected.\n\n\
             The one ergonomic edge is `int`: `decimal + int` (either order) widens the int to a\n\
             scale-0 `decimal` and stays `decimal` (qty/count math). To combine with a `float`,\n\
             convert explicitly first — there is no silent bridge.\n"
        }
        "E-DECIMAL-DIV" => {
            "E-DECIMAL-DIV — decimal division semantics (informational; no longer a compile error).\n\n\
             As of 2026-06-27, `decimal` supports both `%` and `/` as operators:\n\n\
             \t• `%` (remainder) is always exact — no rounding, result scale = max(operand scales).\n\
             \t• `/` is *exact-or-fault*: it returns the exact quotient when it terminates\n\
             \t  (`10.0d / 4.0d → 2.5`, `1d / 8d → 0.125`, minimal form), and FAULTS at runtime when\n\
             \t  the quotient does not terminate (`1d / 3d`) — no silent precision loss.\n\n\
             For a *rounded* quotient, name the scale and rounding mode explicitly:\n\n\
             \timport Core.Decimal;\n\
             \tdecimal unit = Decimal.div(10.00d, 3d, 2, new HalfEven());  // 3.33\n\
             \tdecimal cents = Decimal.round(2.345d, 2, new HalfUp());     // 2.35\n\n\
             `mode` is a `RoundingMode` (`HalfUp`/`HalfDown`/`HalfEven`/`Up`/`Down`/`Ceiling`/`Floor`,\n\
             injected when you import `Core.Decimal`). A zero divisor faults; so does a result past\n\
             i128 range.\n"
        }
        "E-DECIMAL-LITERAL" => {
            "E-DECIMAL-LITERAL — a `decimal` literal is malformed or out of range.\n\n\
             A `decimal` literal is digits with an optional fractional part and a `d` suffix\n\
             (`19.99d`, `100d`, `1.500d`); the scale is the count of fractional digits in the text\n\
             (so `1.50d` is scale 2 and `1.500d` is scale 3). An exponent is not allowed (`1e3d` is\n\
             rejected — write the digits out), and a literal whose unscaled value exceeds the\n\
             i128 range is a compile-time error (not a runtime fault). For dynamic/string input,\n\
             use `Decimal.of(s)` (returns `decimal?`, `null` on a bad string).\n"
        }
        "E-VARIADIC-UNSUPPORTED" => {
            "E-VARIADIC-UNSUPPORTED — a variadic parameter (`...`) on a method or lambda.\n\n\
             Variadic parameters (`int ...nums`, DEC-298) are supported on FREE FUNCTIONS in v1; a\n\
             call gathers the trailing arguments into a `List<int>`. Methods and lambdas are a\n\
             follow-on slice — for them, declare an explicit `List<int>` parameter and pass a list.\n"
        }
        "E-VARIADIC-NOT-LAST" => {
            "E-VARIADIC-NOT-LAST — a variadic parameter is not the last parameter.\n\n\
             Only the FINAL parameter may be variadic (`...`), because it collects every remaining\n\
             argument. Move it to the end: `function log(string tag, int ...codes)`.\n"
        }
        "E-VARIADIC-DEFAULT" => {
            "E-VARIADIC-DEFAULT — a variadic parameter has a default value.\n\n\
             A variadic parameter (`int ...nums`) already defaults to an empty list when no trailing\n\
             arguments are passed, so an explicit `= …` default is redundant and not allowed.\n"
        }
        "E-NAMED-ARG-UNKNOWN" => {
            "E-NAMED-ARG-UNKNOWN — a named argument names no parameter (DEC-297).\n\n\
             `f(colour: …)` requires `f` to declare a parameter `colour`. Check the spelling against\n\
             the function/constructor's parameter names.\n"
        }
        "E-NAMED-ARG-DUPLICATE" => {
            "E-NAMED-ARG-DUPLICATE — a parameter is supplied twice (DEC-297).\n\n\
             Each parameter may be given once — either positionally or by name, not both, and not by\n\
             two `name:` arguments. Remove the duplicate.\n"
        }
        "E-NAMED-ARG-POSITIONAL-AFTER" => {
            "E-NAMED-ARG-POSITIONAL-AFTER — a positional argument follows a named one (DEC-297).\n\n\
             Once you start naming arguments, the rest must be named too (a trailing positional has no\n\
             unambiguous slot). Put all positional arguments before every `name:` argument.\n"
        }
        "E-NAMED-ARG-MISSING" => {
            "E-NAMED-ARG-MISSING — a required parameter got no value (DEC-297).\n\n\
             A named call must still supply every required (non-defaulted) parameter, positionally or\n\
             by name. Add the missing one.\n"
        }
        "E-NAMED-ARG-MISPLACED" => {
            "E-NAMED-ARG-MISPLACED — `name: value` outside a call's argument list (DEC-297).\n\n\
             Named arguments are only meaningful in a function/constructor/method call. Write just the\n\
             value in other positions.\n"
        }
        "E-NAMED-ARG-UNSUPPORTED" => {
            "E-NAMED-ARG-UNSUPPORTED — named arguments in an unsupported position (DEC-297).\n\n\
             v1 supports named arguments on non-generic, non-overloaded free functions, constructors,\n\
             and methods. They are not yet supported on stdlib natives, generic/overloaded calls, or\n\
             together with a variadic parameter — call those positionally.\n"
        }
        "E-DEFAULT-PARAM-ORDER" => {
            "E-DEFAULT-PARAM-ORDER — a required parameter follows a defaulted one.\n\n\
             A parameter with a default value (`int y = 10`) makes that argument optional, so every\n\
             parameter after it must also have a default — otherwise a call that omits the default\n\
             would leave a later required argument unfilled. Move all defaulted parameters to the end:\n\
             `function f(int x, int y = 1, int z = 2)`.\n"
        }
        "E-DEFAULT-PARAM-EXPR" => {
            "E-DEFAULT-PARAM-EXPR — a default value is not a literal constant.\n\n\
             A default parameter value must be a literal — a number, string, bool, bytes, `null` — or\n\
             a ZERO-payload enum variant construction (`new Mode.Fast()`, DEC-258): both are\n\
             compile-time-known. Arbitrary or side-effecting expressions (a function call, a field\n\
             read, a payload-carrying variant) are not allowed: the default is inlined at each call\n\
             site, so a constant keeps it predictable and byte-identical across the backends. Use a\n\
             literal/variant, or compute the value inside the body.\n"
        }
        "E-DEFAULT-PARAM-TYPE" => {
            "E-DEFAULT-PARAM-TYPE — a default value's type does not match the parameter.\n\n\
             The default literal must be assignable to the parameter's declared type (`int x = 3` ok;\n\
             `int x = \"no\"` is not). `null` is allowed only for an optional parameter (`int? x = null`).\n"
        }
        "E-CTOR-DEFAULT-GENERIC" => {
            "E-CTOR-DEFAULT-GENERIC — a generic class constructor cannot take default parameters yet.\n\n\
             Construction of a generic class infers its type arguments FROM the constructor call's\n\
             arguments, and the default fill runs before that inference — a defaulted (omittable)\n\
             argument could leave a type parameter unconstrained. Drop the default or use a static\n\
             factory on the generic class. (A documented deferral — DEC-236 covers non-generic\n\
             classes; the generic case needs fill-aware inference.)\n"
        }
        "E-DEFAULT-PARAM-CONTEXT" => {
            "E-DEFAULT-PARAM-CONTEXT — a default value on a method/constructor parameter.\n\n\
             Default parameter values are supported on **free functions** in v1; methods and\n\
             constructors are a documented follow-up (the call-fill pass resolves free/native calls,\n\
             not method dispatch). Drop the default, or overload / call with all arguments explicitly.\n"
        }
        "E-IFACE-IMPL" => {
            "E-IFACE-IMPL — a name in `implements`/`extends` is not an interface.\n\n\
             A class `implements` declared interfaces, and an interface `extends` other interfaces. A\n\
             name that resolves to a class, enum, or nothing cannot appear there. Declare the missing\n\
             `interface`, or remove the name.\n"
        }
        "E-IFACE-UNIMPL" => {
            "E-IFACE-UNIMPL — a class does not implement an interface method.\n\n\
             A class that `implements I` must provide every method of `I` and its `extends` chain. PHP\n\
             would fatal at class-declaration time, so Phorj rejects it up front. Add the missing\n\
             method (matching the interface's signature) to the class.\n"
        }
        "E-IFACE-SIG" => {
            "E-IFACE-SIG — a class method's signature does not match the interface's.\n\n\
             An implementing method must match the interface method's parameter types and return type\n\
             exactly (no variance this slice). Align the class method's signature with the interface\n\
             declaration.\n"
        }
        "E-IFACE-VIS" => {
            "E-IFACE-VIS — a class implements an interface method with reduced visibility.\n\n\
             Interface methods are public, so an implementing method must be public too — declaring it\n\
             `private` or `protected` REDUCES the method's visibility, which PHP fatals on at class\n\
             declaration and which would otherwise let the method be reached (and its visibility\n\
             bypassed) through an intersection-typed receiver (DEC-251). Make the implementing method\n\
             public.\n"
        }
        "E-IFACE-CYCLE" => {
            "E-IFACE-CYCLE — interfaces form an `extends` cycle.\n\n\
             `interface A extends B` while `B extends A` (directly or transitively) has no well-founded\n\
             method set. Break the cycle so every interface's `extends` chain bottoms out.\n"
        }
        "E-EXTEND-FINAL" => {
            "E-EXTEND-FINAL — a class extends a non-`open` class.\n\n\
             Phorj is final-by-default (M-RT S6): a class can only be a parent if it is declared\n\
             `open class`. Mark the parent `open` to allow extension, or remove the `extends`. (This is\n\
             the inheritance dual of the `mutable` opt-in — safe by default, opt into the power.)\n"
        }
        "E-EXTEND-UNKNOWN" => {
            "E-EXTEND-UNKNOWN — a class extends a name that is not a class.\n\n\
             `extends` lists parent *classes*; the name resolved to an interface, enum, or nothing.\n\
             Use `implements` for interfaces, or declare the missing parent class.\n"
        }
        "E-MI-CYCLE" => {
            "E-MI-CYCLE — classes form an `extends` cycle.\n\n\
             `class A extends B` while `B extends A` (directly or transitively) has no well-founded\n\
             member set. Break the cycle so every class's `extends` chain bottoms out at a root class.\n"
        }
        "E-MI-CONFLICT" => {
            "E-MI-CONFLICT — a method is inherited from more than one parent.\n\n\
             Under multiple inheritance (`class C extends A, B`, M-RT S6b), if two parents each supply a\n\
             method of the same name Phorj will not silently pick one. Resolve it in C's body with a\n\
             clause: `use P.m` (pick parent P's `m`), `rename P.m as n` (keep both under a new name),\n\
             `exclude P.m` (drop one), or override by declaring `function m(…)` in C. A diamond where\n\
             both arms reach the *same* declaring method auto-merges and is never a conflict.\n"
        }
        "E-USE-UNKNOWN" => {
            "E-USE-UNKNOWN — a `use` clause names something that is not a declared trait.\n\n\
             A class composes a trait with `use T;` (M-RT S8). The name must resolve to a `trait`, not a\n\
             class, interface, or undeclared name. If you meant to inherit a class, use `extends` (a\n\
             class is an *is-a* supertype); `use` is for *has-the-behavior-of* horizontal reuse. Declare\n\
             the trait with `trait T { … }`.\n"
        }
        "E-USE-AS-TYPE" => {
            "E-USE-AS-TYPE — a trait was used where a type is expected.\n\n\
             A trait (M-RT S8) is horizontal reuse, NOT a type: you cannot type a variable/parameter/\n\
             field as a trait, and `instanceof T` on a trait is rejected. Compose it into a class with\n\
             `use T;` and type values by the class (or by an interface the class implements).\n"
        }
        "E-TRAIT-CTOR-COLLISION" => {
            "E-TRAIT-CTOR-COLLISION — a class composes constructors from two or more traits.\n\n\
             A `use`d trait's constructor becomes the class's constructor (M-RT S8). A class can adopt at\n\
             most one — two trait constructors would collide (PHP fatals on this). Resolve it by giving\n\
             the class its own `constructor(…)` (which wins and runs the trait initializers explicitly),\n\
             or by composing only one ctor-bearing trait.\n"
        }
        "W-TRAIT-CTOR-SHADOWED" => {
            "W-TRAIT-CTOR-SHADOWED — a class's own constructor shadows a `use`d trait's constructor.\n\n\
             When a class declares its own `constructor` AND composes a trait that also has one, the\n\
             class's ctor wins and the trait's never runs (PHP P1). This is a warning, not an error —\n\
             intentional if you meant to override. If the trait's initializer must run, call it from the\n\
             class ctor or drop the class ctor.\n"
        }
        "W-TRAIT-CTOR-PARENT-SKIPPED" => {
            "W-TRAIT-CTOR-PARENT-SKIPPED — a trait constructor runs instead of the parent's.\n\n\
             When a class `extends` a parent that has a constructor AND composes a trait that also has\n\
             one (with no class ctor of its own), the trait's constructor wins and the parent's is NOT\n\
             auto-run (PHP P2). A warning so the silent skip is visible: give the class its own ctor that\n\
             initializes the parent if that matters.\n"
        }
        "E-MI-FIELD-CONFLICT" => {
            "E-MI-FIELD-CONFLICT — a field is inherited from more than one parent.\n\n\
             Under multiple inheritance (`class C extends A, B`, M-RT S6c), if two parents each declare\n\
             an instance field of the same name Phorj will not silently pick one. Unlike a method\n\
             collision there are no `use`/`rename`/`exclude` clauses — PHP has no `insteadof` for\n\
             properties. Resolve it by redeclaring the field in C (or renaming it in a parent). A\n\
             diamond where both arms reach the *same* declaring field auto-merges and is never a\n\
             conflict.\n"
        }
        "E-ABSTRACT-INSTANTIATE" => {
            "E-ABSTRACT-INSTANTIATE — an abstract class cannot be instantiated.\n\n\
             An `abstract class` (M-RT S6b) may have bodyless `abstract function` methods, so it has no\n\
             complete behavior to construct. Instantiate a concrete subclass that implements every\n\
             abstract method instead.\n"
        }
        "E-ABSTRACT-UNIMPL" => {
            "E-ABSTRACT-UNIMPL — a concrete class leaves an abstract method unimplemented.\n\n\
             A non-`abstract` class must provide a body for every `abstract` method it declares or\n\
             inherits. Implement the method (`function name(…) -> void { … }`), or declare the class itself\n\
             `abstract` so a further subclass implements it.\n"
        }
        "E-OPEN-STATIC" => {
            "E-OPEN-STATIC — a method is both `open` and `static`.\n\n\
             Static methods are resolved by name, not by an instance's runtime class, so they are not\n\
             virtual and cannot be overridden. Drop `open` (the method stays callable) or drop `static`\n\
             (the method becomes a normal, overridable instance method).\n"
        }
        "E-OVERRIDE-FINAL" => {
            "E-OVERRIDE-FINAL — a method overrides a non-`open` ancestor method.\n\n\
             Methods are final-by-default (M-RT S6): a subclass may only redefine a parent method that\n\
             the parent declared `open function`. Mark the parent method `open` to allow the override,\n\
             or rename the subclass method so it does not shadow the inherited one.\n"
        }
        _ => return None,
    })
}
