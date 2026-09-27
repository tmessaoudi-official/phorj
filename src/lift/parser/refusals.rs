//! PHP-lift parser — row 4o (DEC-543, DEC-544): two constructs refused BY NAME where the parser used
//! to report the token it tripped on (`found Amp`, `found Bar`).
//!
//! * A by-reference PARAMETER `&$m` — the parameter half of DEC-506: references contradict the
//!   value/handle split and are ruled out, not merely unbuilt. The one census site was hand-ported to
//!   a returned value (DEC-543).
//! * A union type with a `false` member (`string|false|null`) — PHP's sentinel return. Collapsing it
//!   would merge outcomes the caller tells apart, so the port declares an enum (DEC-544). Any OTHER
//!   union is refused too, but as a gap: phorj has `A | B`, the lift does not yet map onto it.

use super::*;

impl PParser {
    /// At a `&` before a parameter's `$name`: the DEC-543 refusal, naming the parameter.
    pub(super) fn refuse_by_ref_param(&self) -> String {
        let name = match self.peek_at(1) {
            PTok::Var(v) => v.clone(),
            _ => "?".to_string(),
        };
        self.err_reason(&format!(
            "the parameter `${name}` is taken by reference (`&${name}`), which has no phorj form — \
             references contradict the value/handle split and are RULED OUT (DEC-506, DEC-543), not \
             merely unimplemented. Return the value instead of writing through the parameter"
        ))
    }

    /// `first` was just parsed and a `|` follows: read the remaining members, then refuse the union
    /// by name — DEC-544's wording when one member is `false`.
    pub(super) fn refuse_union_type(&mut self, first: PhpType) -> String {
        let mut members = vec![first];
        while self.eat(&PTok::Bar) {
            match self.parse_type_member() {
                Ok(t) => members.push(t),
                Err(e) => return e,
            }
        }
        let text = members.iter().map(show).collect::<Vec<_>>().join("|");
        if members
            .iter()
            .any(|t| matches!(t, PhpType::Named(n) if n.eq_ignore_ascii_case("false")))
        {
            return self.err_reason(&format!(
                "the union type `{text}` has a `false` member — PHP's sentinel return has no phorj \
                 form (DEC-544): write `?T` when `false` is the only sentinel, or an enum with one \
                 variant per outcome when `false` and `null` mean different things"
            ));
        }
        self.err_reason(&format!(
            "the union type `{text}` has no lift yet — phorj's `A | B` exists, but the lifter does \
             not map PHP unions onto it; only `?T` lifts"
        ))
    }
}

fn show(t: &PhpType) -> String {
    match t {
        PhpType::Named(n) => n.clone(),
        PhpType::Nullable(inner) => format!("?{}", show(inner)),
        other => format!("{other:?}"),
    }
}
