//! Transpiler — the statically inferred operand kind (`OpKind`), split out of `mod.rs` (Invariant 13).

#[derive(Clone, PartialEq, Eq, Debug)]
pub(super) enum OpKind {
    Str,
    Int,
    Float,
    /// `decimal` (M-NUM S1). A decimal operand routes `+ - *` to the `__phorj_dec_*` BCMath helpers
    /// (exact + i128-bounds-checked), and a decimal value erases to a PHP `string` for display.
    Decimal,
    Bool,
    /// A value of a user-defined class/enum/interface, carrying its name so a field read resolves
    /// through `class_field_kinds` (T6b). Never an arithmetic/display operand itself.
    Class(String),
    /// `List<E>` carrying its element kind, so `xs[i]` resolves to `E` (T6d) — `xs[i] + 1` / `"{xs[i]}"`.
    List(Box<OpKind>),
    /// `Map<K, V>` carrying key+value kinds, so `m[k]` resolves to `V` (T6d).
    Map(Box<OpKind>, Box<OpKind>),
    /// A TUPLE, carrying **each position's own kind** (DEC-504) — the transpiler's mirror of
    /// [`crate::compiler::CTy::Tuple`], and load-bearing for exactly the same reason. A tuple erases
    /// to a list, but a list carries ONE element kind, so resolving a tuple as `List` gives every
    /// position the kind of position 0. Here that does not merely de-specialize: it picks the wrong
    /// PHP OPERATOR. `(source: string, bp: int) t = …; t.bp + 1` becomes `$t[1] . 1` — string
    /// concatenation — and the PHP leg prints `31` where both native legs print `4`, breaking the
    /// byte-identity spine (Invariant 1) rather than just running slower.
    Tuple(Vec<OpKind>),
    /// A type that WRAPS other types the transpiler does not otherwise model: an optional's inner
    /// type, a generic instantiation's `[Class(name), args...]`, a union's or intersection's members.
    /// It never specializes an operator (every consumer treats it like `Other`); its one job is to let
    /// `==` ask whether ANY component carries a `decimal` (DEC-557), because flattening `decimal?` or
    /// `Box<decimal>` to `Other`/`Class` made a decimal look unknown and take the strict helper.
    Wrapped(Vec<OpKind>),
    Other,
}
