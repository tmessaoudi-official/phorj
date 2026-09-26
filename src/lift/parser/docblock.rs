//! PHP-lift parser — docblock generics type the bare `array` (Lane R-4, 2026-09-05).
//!
//! PHP's `array` says nothing about its shape, and the lifter refuses to guess between `List`,
//! `Map` and `Set`. A strict codebase carries the answer in its docblocks — scout annotates 155 of
//! its 192 `array` parameters and 82 of 112 `array` returns with `list<T>` / `array<K, V>` — and
//! the lexer already records every `/** … */` by the token that follows it. So, at every function,
//! member and constructor-parameter boundary, a declared `array` / `?array` whose `@param` /
//! `@return` / `@var` carries a generic is replaced by [`PhpType::Generic`]. That is a faithful
//! reading of the program's own (unenforced) claim, under the draft's `// lifted (verify)` header —
//! not an inference. A bare `array` with no annotation stays refused, and the refusal names the fix.
//!
//! The docblock TYPE grammar itself lives in [`super::doc_types`]; this file finds the tag and decides
//! where its type is substituted.

use super::*;

/// The type written after `tag` in `doc` — for `@param`, the one followed by `$name`. Bracket-aware,
/// so `array<string, int>` and `(callable(A): R)|null` (with their spaces) are one type, and so is
/// `callable(A): R`.
fn doc_tag(doc: &str, tag: &str, name: Option<&str>) -> Option<String> {
    for line in doc.lines() {
        let l = line.trim_start_matches([' ', '*']).trim_start();
        let Some(rest) = l.strip_prefix(tag) else {
            continue;
        };
        if !rest.starts_with(' ') {
            continue;
        }
        let rest = rest.trim_start();
        let (mut depth, mut end) = (0usize, rest.len());
        for (i, c) in rest.char_indices() {
            match c {
                '<' | '{' | '(' => depth += 1,
                '>' | '}' | ')' => depth = depth.saturating_sub(1),
                // Row 4e: a callable signature's return type follows `): ` — the space after the
                // colon is inside the type. Anchored on `):`, which only a signature can produce.
                ' ' if depth == 0 && rest[..i].ends_with("):") => {}
                ' ' if depth == 0 => {
                    end = i;
                    break;
                }
                _ => {}
            }
        }
        let ty = &rest[..end];
        match name {
            Some(n) => {
                let var = format!("${n}");
                // `$row` must not answer for `$rows`: the name ends at a non-identifier character.
                let after = rest[end..].trim_start();
                let named = after.strip_prefix(&var).is_some_and(|tail| {
                    !tail.starts_with(|c: char| c.is_alphanumeric() || c == '_')
                });
                if named {
                    return Some(ty.to_string());
                }
            }
            None => return Some(ty.to_string()),
        }
    }
    None
}

impl PParser {
    /// The docblock immediately before the current token, if any.
    pub(super) fn doc_here(&self) -> Option<super::Doc> {
        let line = self.line();
        self.docs.get(&self.pos).map(|text| super::Doc {
            text: text.clone(),
            line,
        })
    }

    /// A member's docblock: `@param`/`@return` on a method (promoted constructor parameters
    /// included), `@var` on a property or a constant (DEC-533).
    pub(super) fn apply_doc_member(
        &mut self,
        doc: Option<&super::Doc>,
        m: &mut PhpMember,
    ) -> Result<(), String> {
        self.with_doc(doc, |p, doc| match m {
            PhpMember::Method(me) => p.apply_doc_signature(doc, &mut me.params, &mut me.ret),
            PhpMember::Prop { ty, .. } | PhpMember::Const { ty, .. } => p.apply_doc_var(doc, ty),
        })
    }

    pub(super) fn apply_doc_item(
        &mut self,
        doc: Option<&super::Doc>,
        it: &mut PhpItem,
    ) -> Result<(), String> {
        self.with_doc(doc, |p, doc| match it {
            PhpItem::Function(f) => p.apply_doc_signature(doc, &mut f.params, &mut f.ret),
            PhpItem::Stmt(st) => p.apply_doc_local(doc, st),
            _ => Ok(()),
        })
    }

    /// `{ stmt* }` — lives here (not in `items.rs`) so every statement passes the `@var` hook.
    pub(super) fn parse_block(&mut self) -> Result<Vec<PhpStmt>, String> {
        self.expect(&PTok::LBrace, "`{`")?;
        let mut stmts = Vec::new();
        while !self.at(&PTok::RBrace) && !self.at(&PTok::Eof) {
            let doc = self.doc_here();
            let mut st = self.parse_stmt()?;
            self.with_doc(doc.as_ref(), |p, doc| p.apply_doc_local(doc, &mut st))?;
            stmts.push(st);
        }
        self.expect(&PTok::RBrace, "`}`")?;
        Ok(stmts)
    }

    /// `/** @var list<T> $xs */ $xs = [];` (Lane R-6, 54 such docblocks in scout): the empty literal
    /// becomes [`PhpExpr::EmptyColl`] carrying the declared type — phorj needs an empty collection's
    /// type, and the program wrote it down. Any other value assigned under a `@var` for the same
    /// variable becomes [`PhpExpr::Declared`] (DEC-515, row 5d), so the lifter can keep the type on the
    /// local's declaration. Any other statement is untouched.
    ///
    /// The `@var` must be FOR this variable: named `$x`, or unnamed in a docblock that names no
    /// variable at all. A docblock naming `$other` says nothing about `$x`.
    fn apply_doc_local(&mut self, doc: Option<&str>, st: &mut PhpStmt) -> Result<(), String> {
        let Some(doc) = doc else {
            return Ok(());
        };
        let PhpStmt::Expr(PhpExpr::Assign { target, value }) = st else {
            return Ok(());
        };
        let PhpExpr::Var(name) = target.as_ref() else {
            return Ok(());
        };
        let Some(ty) = doc_tag(doc, "@var", Some(name)).or_else(|| {
            if doc.contains('$') {
                None
            } else {
                doc_tag(doc, "@var", None)
            }
        }) else {
            return Ok(());
        };
        if matches!(value.as_ref(), PhpExpr::Array(items) if items.is_empty()) {
            let ty = self.parse_doc_type(&ty)?;
            if matches!(ty, PhpType::Generic { .. }) {
                **value = PhpExpr::EmptyColl(ty);
            }
            return Ok(());
        }
        // A type the docblock parser cannot read leaves the local exactly as it was: the declaration
        // is an aid here, not a requirement, so it never fails the lift.
        if let Ok(ty) = self.parse_doc_type(&ty) {
            let inner = std::mem::replace(value.as_mut(), PhpExpr::Null);
            **value = PhpExpr::Declared {
                ty,
                value: Box::new(inner),
            };
        }
        Ok(())
    }

    fn apply_doc_signature(
        &mut self,
        doc: Option<&str>,
        params: &mut [PhpParam],
        ret: &mut Option<PhpType>,
    ) -> Result<(), String> {
        let Some(doc) = doc else {
            return Ok(());
        };
        for p in params.iter_mut() {
            if let (Some(ty), Some(t)) = (&mut p.ty, doc_tag(doc, "@param", Some(&p.name))) {
                self.substitute(ty, &t)?;
            }
        }
        if let (Some(ty), Some(t)) = (ret, doc_tag(doc, "@return", None)) {
            self.substitute(ty, &t)?;
        }
        Ok(())
    }

    fn apply_doc_var(&mut self, doc: Option<&str>, ty: &mut Option<PhpType>) -> Result<(), String> {
        if let (Some(doc), Some(ty)) = (doc, ty) {
            if let Some(t) = doc_tag(doc, "@var", None) {
                self.substitute(ty, &t)?;
            }
        }
        Ok(())
    }

    /// Only a bare `array` / `?array` is substituted — or, since row 4e, a `callable` / `Closure`
    /// (either nullable) whose docblock spells a signature. A `@param non-empty-string $s` on a
    /// `string` is a refinement phorj cannot express and is left exactly as declared; a `callable`
    /// docblock with no `(` is left too, so the native's own refusal names the fix. The parsed type
    /// must match the slot: a function never replaces an `array`, nor anything else a `callable`.
    fn substitute(&mut self, ty: &mut PhpType, doc_ty: &str) -> Result<(), String> {
        let named =
            |t: &PhpType, ns: &[&str]| matches!(t, PhpType::Named(n) if ns.contains(&n.as_str()));
        let slot = match ty {
            PhpType::Nullable(inner) => &mut **inner,
            t => t,
        };
        if named(slot, &["array"]) {
            let t = self.parse_doc_type(doc_ty)?;
            if is_function(&t) {
                return Err(self.err(&format!(
                    "the docblock type `{doc_ty}` does not describe the declared `array`"
                )));
            }
            *slot = t;
        } else if named(slot, &["callable", "Closure"]) && doc_ty.contains('(') {
            let t = self.parse_doc_type(doc_ty)?;
            if !is_function(&t) {
                return Err(self.err(&format!(
                    "the docblock type `{doc_ty}` does not describe the declared `callable`"
                )));
            }
            *slot = t;
        }
        Ok(())
    }
}

/// A function type, or a nullable one — what may replace a `callable` slot, and never an `array` one.
fn is_function(t: &PhpType) -> bool {
    match t {
        PhpType::Function { .. } => true,
        PhpType::Nullable(inner) => matches!(**inner, PhpType::Function { .. }),
        _ => false,
    }
}
