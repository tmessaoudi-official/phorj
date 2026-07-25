//! Tests: import-driven method-position sugar and injected-error namespacing.
use super::super::*;
use super::wp;

#[test]
fn qualified_injected_error_types_resolve_everywhere() {
    // DEC-234 member-error namespacing: `catch (UriModule.UriMalformedError e)`, `throws UriModule.UriError`, and
    // `throw new UriModule.UriMalformedError(…)` — the module-qualified spelling for EVERY injected module's
    // members (routed through the UA-L2 module_of registry; the old hardcoded collapse table knew
    // only Http/Time/Decimal). Bare member-imported names stay the alias.
    let src = wp(r#"import Core.Output;
import Core.UriModule.Uri;
function boom(): never throws UriModule.UriError { throw new UriModule.UriMalformedError("m"); }
function main(): void {
    try {
        Uri u = Uri.parse("http://exa mple/");
        Output.printLine(u.toString());
    } catch (UriModule.UriMalformedError e) {
        Output.printLine("caught: {e.message}");
    } catch (UriModule.UriError e) {
        Output.printLine("base: {e.message}");
    }
    try { boom(); } catch (UriModule.UriError e) { Output.printLine("boom: {e.message}"); }
}"#);
    let expected = "caught: The specified URI is malformed\nboom: m\n";
    assert_eq!(cmd_run(&src).unwrap(), expected);
    assert_eq!(cmd_treewalk(&src).unwrap(), expected);
}

#[test]
fn function_import_enables_method_position_sugar() {
    // DEC-274 sugar gate, function level: `import Core.String.upperCase;` enables BOTH the bare
    // call (DEC-197) and the method form; an ALIASED import matches on the alias and rewrites to
    // the native's real name (`List.rev` exists on no backend).
    let src = wp(r#"import Core.Output;
import Core.String.upperCase;
import Core.List.reverse as rev;
function main(): void {
    Output.printLine("abc".upperCase());
    Output.printLine(upperCase("xyz"));
    List<int> xs = [3, 1, 2];
    List<int> r = xs.rev();
    Output.printLine("{r[0]}");
}"#);
    let expected = "ABC\nXYZ\n2\n";
    assert_eq!(cmd_run(&src).unwrap(), expected);
    assert_eq!(cmd_treewalk(&src).unwrap(), expected);
}

#[test]
fn bare_fn_import_survives_user_class_named_like_module_leaf() {
    // Regression (DEC-277 build): the checker rewrites a bare member-imported native call to the
    // leaf-qualified form (`sqrt(4.0)` → `Math.sqrt(4.0)`, no import item), which the backends
    // resolve by leaf. A user class merely NAMED `Math` must not capture that fallback — an early
    // class-name guard in `index_of_qualified` made both Rust backends reject this type-checked
    // program ("class `Math` has no static method `sqrt`") while the PHP leg ran it. The guard is
    // now scoped to `Core.Native.*` leaves only (whose leaf == a prelude class BY DESIGN).
    let src = wp(r#"import Core.Output;
import Core.Math.sqrt;
class Math {}
function main(): void { Output.printLine("{sqrt(4.0)}"); }"#);
    // Float display is PHP-faithful: `2`, not `2.0`.
    let expected = "2\n";
    assert_eq!(cmd_run(&src).unwrap(), expected);
    assert_eq!(cmd_treewalk(&src).unwrap(), expected);
}
