//! Checker tests — turbofish call-site type arguments (DEC-208 slice A). Explicit type arguments
//! pre-seed the substitution; the arguments must still agree, the arity must match the callee's
//! type-parameter count (`E-TYPE-ARG-COUNT`), and a non-generic callee rejects them
//! (`E-TURBOFISH-NON-GENERIC`).

use super::support::*;

fn has_code(src: &str, code: &str) -> bool {
    errors_of(src).iter().any(|d| d.code == Some(code))
}

#[test]
fn generic_free_fn_turbofish_binds_and_checks() {
    // Explicit `<int>` binds `T`; the argument agrees; the result types as `int`.
    let e = errors_of(
        "function identity<T>(T x): T { return x; }
         function main(): void { int y = identity<int>(5); }",
    );
    assert!(e.is_empty(), "expected well-typed, got {e:?}");
}

#[test]
fn turbofish_binds_the_return_type() {
    // The result of `identity<int>(5)` is `int`, so assigning it to a `string` is a type error —
    // proving turbofish flows into the return type (not just the argument check).
    assert!(
        !errors_of(
            "function identity<T>(T x): T { return x; }
             function main(): void { string y = identity<int>(5); }",
        )
        .is_empty(),
        "assigning an int result to a string should fail"
    );
}

#[test]
fn turbofish_and_argument_must_agree() {
    // `T` is seeded `int` by the turbofish; a `string` argument disagrees with that binding.
    assert!(
        !errors_of(
            "function identity<T>(T x): T { return x; }
             function main(): void { var y = identity<int>(\"hi\"); }",
        )
        .is_empty(),
        "a string argument against turbofish `<int>` should fail"
    );
}

#[test]
fn wrong_type_arg_count_is_e_type_arg_count() {
    assert!(
        has_code(
            "function identity<T>(T x): T { return x; }
             function main(): void { var y = identity<int, string>(5); }",
            "E-TYPE-ARG-COUNT",
        ),
        "two type args for a one-parameter generic should be E-TYPE-ARG-COUNT"
    );
}

#[test]
fn turbofish_on_non_generic_fn_is_rejected() {
    assert!(
        has_code(
            "function plain(int x): int { return x; }
             function main(): void { var y = plain<int>(5); }",
            "E-TURBOFISH-NON-GENERIC",
        ),
        "turbofish on a non-generic function should be E-TURBOFISH-NON-GENERIC"
    );
}

#[test]
fn return_only_type_param_needs_turbofish_and_binds_from_it() {
    // The headline case (the `queryInto<User>()` shape): `T` appears ONLY in the return type and there
    // is NO value argument to infer it from — so the turbofish is the sole source of `T`. Assigning a
    // `List<int>` result (from `makeEmpty<int>()`) to a `List<string>` must fail, which is only
    // possible if the turbofish actually bound `T = int` into the return type.
    assert!(
        !errors_of(
            "function makeEmpty<T>(): List<T> { return new List<T>(); }
             function main(): void { List<string> xs = makeEmpty<int>(); }",
        )
        .is_empty(),
        "a List<int> result assigned to List<string> must fail — turbofish must bind the return T"
    );
    // The matching positive: the same turbofish typed against the right sink is well-typed.
    assert!(
        errors_of(
            "function makeEmpty<T>(): List<T> { return new List<T>(); }
             function main(): void { List<int> xs = makeEmpty<int>(); }",
        )
        .is_empty(),
        "makeEmpty<int>() should type as List<int>"
    );
}

#[test]
fn inference_without_turbofish_still_works() {
    // The pure-inference path is unchanged (byte-identical AST → no turbofish).
    assert!(
        errors_of(
            "function identity<T>(T x): T { return x; }
             function main(): void { int y = identity(5); }",
        )
        .is_empty(),
        "inference without turbofish must still type-check"
    );
}

#[test]
fn generic_method_turbofish_binds_and_checks() {
    let e = errors_of(
        "class Box {
             function wrap<T>(T x): List<T> { return [x]; }
         }
         function main(): void {
             var b = new Box();
             List<int> xs = b.wrap<int>(7);
         }",
    );
    assert!(e.is_empty(), "expected well-typed, got {e:?}");
}

#[test]
fn generic_method_turbofish_argument_must_agree() {
    assert!(
        !errors_of(
            "class Box {
                 function wrap<T>(T x): List<T> { return [x]; }
             }
             function main(): void {
                 var b = new Box();
                 var xs = b.wrap<int>(\"nope\");
             }",
        )
        .is_empty(),
        "a string argument against method turbofish `<int>` should fail"
    );
}

#[test]
fn turbofish_on_non_generic_method_is_rejected() {
    assert!(
        has_code(
            "class Box {
                 function get(): int { return 1; }
             }
             function main(): void {
                 var b = new Box();
                 var n = b.get<int>();
             }",
            "E-TURBOFISH-NON-GENERIC",
        ),
        "turbofish on a non-generic method should be E-TURBOFISH-NON-GENERIC"
    );
}

#[test]
fn generic_static_method_turbofish_binds() {
    let e = errors_of(
        "class Maker {
             static function of<T>(T x): List<T> { return [x]; }
         }
         function main(): void {
             List<int> xs = Maker.of<int>(3);
         }",
    );
    assert!(e.is_empty(), "expected well-typed, got {e:?}");
}
