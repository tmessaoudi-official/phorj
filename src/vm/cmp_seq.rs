//! `Op::CmpSeq` — the FUSED tuple comparison (DEC-513).
//!
//! The generic path builds two `Value::List`s solely to compare them and drop them; on the
//! comparator-heavy `spaceshipsort` bench that materialization was ~65% of the time. When the
//! compiler sees a tuple literal on BOTH sides it emits the elements followed by this Op instead,
//! and nothing is built.
//!
//! **The no-allocation property is the whole point, and it lives here.** Popping the `2n` operands
//! into a `Vec` would trade two `Rc<Vec<Value>>` for one `Vec` — the allocation would move, not
//! vanish, and the win would be given straight back. So the two element runs are borrowed IN PLACE
//! off the operand stack and handed to the kernel as slices.
//!
//! Both projections are shared, not re-implemented (Invariant 4): the ordering itself comes from
//! `value::compare_seq`, `<=>` projects through `value::project_three_way`, and the four ordering
//! kinds go through [`project_bool`] — which `vm::compare` also calls, so the `None -> false` NaN
//! rule exists exactly once in this backend.

use super::*;
use std::cmp::Ordering;

/// The `Lt`/`Gt`/`Le`/`Ge` projection: NaN (`None`) compares `false`.
///
/// Shared by the fused Op and the scalar `vm::compare`, so the NaN rule cannot drift between them.
/// `Spaceship` is not a bool projection — it goes through `value::project_three_way`, which maps
/// `None` to `1` instead; routing it here (or here through a compare against zero) is the leg-split
/// both that function and `Op::CmpSeq` warn about.
pub(super) fn project_bool(o: Option<Ordering>, kind: SeqOrd) -> bool {
    match o {
        Some(ord) => match kind {
            SeqOrd::Lt => ord == Ordering::Less,
            SeqOrd::Gt => ord == Ordering::Greater,
            SeqOrd::Le => ord != Ordering::Greater,
            SeqOrd::Ge => ord != Ordering::Less,
            SeqOrd::Spaceship => {
                unreachable!("`<=>` projects through value::project_three_way, not project_bool")
            }
        },
        None => false, // NaN compares false
    }
}

/// Execute `Op::CmpSeq(n, kind)`: the top `2n` stack values are the left tuple's `n` elements then
/// the right tuple's `n`, in source order. Compares them lexicographically and replaces all `2n`
/// with the single projected result.
///
/// The operands are dropped before the `?` on the kernel's result, so a comparability fault leaves
/// the stack exactly as the scalar `Op::Cmp` arm does (which pops both operands before projecting)
/// — a handler catching the fault sees no leftover elements.
pub(super) fn exec(stack: &mut Vec<Value>, n: usize, kind: SeqOrd) -> Result<(), String> {
    let base = stack.len() - 2 * n;
    // Borrowed in place — see the module note: materializing here would undo the optimization.
    let computed = {
        let (xs, ys) = stack[base..].split_at(n);
        crate::value::compare_seq(xs, ys)
    };
    stack.truncate(base);
    let ord = computed?;
    stack.push(match kind {
        SeqOrd::Spaceship => Value::Int(crate::value::project_three_way(ord)),
        k => Value::Bool(project_bool(ord, k)),
    });
    Ok(())
}
