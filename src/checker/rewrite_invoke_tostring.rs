use super::*;

/// DEC-331 D9 — lower the two attribute-method sugars into ordinary method calls, on the LIVE
/// (fully-desugared, fully-filled) AST, so no backend needs to know about `#[Invoke]`/`#[ToString]`.
/// `invoke` (keyed by a `Call` node's `Span.start`): `x(args)` → `x.<chosen>(args)`, the method the
/// checker already resolved. `tostring` (keyed by any expression's `Span.start`): an object in string
/// context — an interpolation hole or a `Conversion.toString` argument — → `<expr>.<chosen>()`.
/// Runs OUTERMOST in `cli::check_and_expand_reified` on the final nodes (never a check-time clone —
/// which would drop a later default-fill, so these can't ride `rewrite_ufcs`; see `rewrite_html`).
/// Bottom-up: children first, THEN this node — so `"{x(5)}"` (both maps, one span) becomes
/// `x.add(5)` (invoke) then `(x.add(5)).toStr()` (tostring). Both maps empty ⇒ program untouched.
pub fn resolve_invoke_tostring(
    program: Program,
    invoke: &HashMap<usize, String>,
    tostring: &HashMap<usize, String>,
) -> Program {
    use crate::ast::{ClassMember, Item};
    if invoke.is_empty() && tostring.is_empty() {
        return program;
    }
    use super::rewrite_invoke_tostring_walk::{rblock, rexpr, Names};

    // Rewrite every member that can hold an invoke/tostring site. Shared by classes AND traits (a
    // trait flattens into using classes and its bodies reach both backends). Method/ctor/hook bodies
    // AND field initializers are all walked — a field init like `string s = "{obj}";` records a
    // `#[ToString]` target at check time, so it MUST be lowered here too (else interp/VM fault while
    // the PHP leg's `__toString` prints — a byte-identity break; the sibling passes recurse fields too).
    fn rmembers(members: &mut [ClassMember], inv: &Names, ts: &Names) {
        for m in members {
            match m {
                ClassMember::Method(f) => {
                    let body = std::mem::take(&mut f.body);
                    f.body = rblock(body, inv, ts);
                }
                ClassMember::Constructor { body, .. } => {
                    let b = std::mem::take(body);
                    *body = rblock(b, inv, ts);
                }
                ClassMember::Hook { get, set, .. } => {
                    if let Some(e) = get.take() {
                        *get = Some(rexpr(e, inv, ts));
                    }
                    if let Some((p, body)) = set.take() {
                        *set = Some((p, rblock(body, inv, ts)));
                    }
                }
                ClassMember::Field { init, .. } => {
                    if let Some(e) = init.take() {
                        *init = Some(rexpr(e, inv, ts));
                    }
                }
            }
        }
    }

    let items = program
        .items
        .into_iter()
        .map(|item| match item {
            Item::Function(mut f) => {
                f.body = rblock(f.body, invoke, tostring);
                Item::Function(f)
            }
            Item::Class(mut c) => {
                rmembers(&mut c.members, invoke, tostring);
                Item::Class(c)
            }
            Item::Trait(mut t) => {
                rmembers(&mut t.members, invoke, tostring);
                Item::Trait(t)
            }
            // A `test "…" { … }` body is checked and executed like a function body under `phg test`,
            // so an `#[Invoke]`/`#[ToString]` call site inside one needs the same rewrite (CD-31).
            Item::Test { name, body, span } => Item::Test {
                name,
                body: rblock(body, invoke, tostring),
                span,
            },
            // An enum's `backing_value` is scalar-checked and an interface's `methods` are
            // signatures; named rather than folded into `item_leaves!()` so neither is claimed
            // expression-free (CD-31).
            it @ (Item::Enum(..) | Item::Interface(..)) => it,
            it @ (crate::item_leaves!()) => it,
        })
        .collect();

    Program {
        package: program.package,
        items,
        span: program.span,
    }
}
