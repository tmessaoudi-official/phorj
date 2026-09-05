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
//! Grammar read here: `?T`, `T|null`, `list<T>`, `non-empty-list<T>`, `array<T>`, `array<K, V>`,
//! `non-empty-array<…>`, `T[]`, scalars and class names (a `\`-rooted one becomes an implicit
//! `use`, exactly like an inline name in `names.rs`). Refused by name: `array{…}` shapes, `mixed`,
//! `callable`, `iterable`, `object`, other generics, unions other than `|null`.

use super::*;

struct Cursor<'a> {
    s: &'a str,
    i: usize,
}

impl Cursor<'_> {
    fn skip_ws(&mut self) {
        while self.s[self.i..].starts_with(' ') {
            self.i += 1;
        }
    }
    fn eat(&mut self, c: char) -> bool {
        self.skip_ws();
        if self.s[self.i..].starts_with(c) {
            self.i += c.len_utf8();
            true
        } else {
            false
        }
    }
    fn eat_str(&mut self, t: &str) -> bool {
        if self.s[self.i..].starts_with(t) {
            self.i += t.len();
            true
        } else {
            false
        }
    }
    /// A type head: letters, digits, `_`, `\` (paths) and `-` (`non-empty-list`).
    fn ident(&mut self) -> String {
        self.skip_ws();
        let start = self.i;
        while let Some(c) = self.s[self.i..].chars().next() {
            if c.is_alphanumeric() || matches!(c, '_' | '\\' | '-') {
                self.i += c.len_utf8();
            } else {
                break;
            }
        }
        self.s[start..self.i].to_string()
    }
}

/// The type written after `tag` in `doc` — for `@param`, the one followed by `$name`. Bracket-aware,
/// so `array<string, int>` (with its space) is one type.
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
                '<' | '{' => depth += 1,
                '>' | '}' => depth = depth.saturating_sub(1),
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
                if rest[end..].trim_start().starts_with(&var) {
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
    pub(super) fn doc_here(&self) -> Option<String> {
        self.docs.get(&self.pos).cloned()
    }

    /// A member's docblock: `@param`/`@return` on a method (promoted constructor parameters
    /// included), `@var` on a property.
    pub(super) fn apply_doc_member(
        &mut self,
        doc: Option<&str>,
        m: &mut PhpMember,
    ) -> Result<(), String> {
        match m {
            PhpMember::Method(me) => self.apply_doc_signature(doc, &mut me.params, &mut me.ret),
            PhpMember::Prop { ty, .. } => self.apply_doc_var(doc, ty),
            _ => Ok(()),
        }
    }

    pub(super) fn apply_doc_item(
        &mut self,
        doc: Option<&str>,
        it: &mut PhpItem,
    ) -> Result<(), String> {
        match it {
            PhpItem::Function(f) => self.apply_doc_signature(doc, &mut f.params, &mut f.ret),
            PhpItem::Stmt(st) => self.apply_doc_local(doc, st),
            _ => Ok(()),
        }
    }

    /// `{ stmt* }` — lives here (not in `items.rs`) so every statement passes the `@var` hook.
    pub(super) fn parse_block(&mut self) -> Result<Vec<PhpStmt>, String> {
        self.expect(&PTok::LBrace, "`{`")?;
        let mut stmts = Vec::new();
        while !self.at(&PTok::RBrace) && !self.at(&PTok::Eof) {
            let doc = self.doc_here();
            let mut st = self.parse_stmt()?;
            self.apply_doc_local(doc.as_deref(), &mut st)?;
            stmts.push(st);
        }
        self.expect(&PTok::RBrace, "`}`")?;
        Ok(stmts)
    }

    /// `/** @var list<T> $xs */ $xs = [];` (Lane R-6, 54 such docblocks in scout): the empty literal
    /// becomes [`PhpExpr::EmptyColl`] carrying the declared type — phorj needs an empty collection's
    /// type, and the program wrote it down. Any other statement, or a non-empty literal, is untouched.
    fn apply_doc_local(&mut self, doc: Option<&str>, st: &mut PhpStmt) -> Result<(), String> {
        let Some(doc) = doc else {
            return Ok(());
        };
        if let PhpStmt::Expr(PhpExpr::Assign { target, value }) = st {
            if let (PhpExpr::Var(name), PhpExpr::Array(items)) = (target.as_ref(), value.as_ref()) {
                if items.is_empty() {
                    let ty = match doc_tag(doc, "@var", Some(name))
                        .or_else(|| doc_tag(doc, "@var", None))
                    {
                        Some(t) => t,
                        None => return Ok(()),
                    };
                    let ty = self.parse_doc_type(&ty)?;
                    if matches!(ty, PhpType::Generic { .. }) {
                        *value = Box::new(PhpExpr::EmptyColl(ty));
                    }
                }
            }
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

    /// Only a bare `array` / `?array` is substituted. A `@param non-empty-string $s` on a `string`
    /// is a refinement phorj cannot express and is left exactly as declared.
    fn substitute(&mut self, ty: &mut PhpType, doc_ty: &str) -> Result<(), String> {
        let is_array = |t: &PhpType| matches!(t, PhpType::Named(n) if n == "array");
        match ty {
            t if is_array(t) => *t = self.parse_doc_type(doc_ty)?,
            PhpType::Nullable(inner) if is_array(inner) => {
                *inner = Box::new(self.parse_doc_type(doc_ty)?);
            }
            _ => {}
        }
        Ok(())
    }

    pub(super) fn parse_doc_type(&mut self, s: &str) -> Result<PhpType, String> {
        let mut c = Cursor { s: s.trim(), i: 0 };
        let t = self.doc_type(&mut c)?;
        c.skip_ws();
        if c.i < c.s.len() {
            return Err(self.err(&format!(
                "unexpected `{}` in the docblock type `{s}`",
                &c.s[c.i..]
            )));
        }
        Ok(t)
    }

    fn doc_type(&mut self, c: &mut Cursor) -> Result<PhpType, String> {
        let mut nullable = c.eat('?');
        let mut t = self.doc_atom(c)?;
        while c.eat_str("[]") {
            t = PhpType::Generic {
                name: "list".into(),
                args: vec![t],
            };
        }
        while c.eat('|') {
            let alt = c.ident();
            if alt != "null" {
                return Err(self.err(&format!(
                    "a `…|{alt}` union in the docblock type `{}` is Tier-2 (only `|null` lifts)",
                    c.s
                )));
            }
            nullable = true;
        }
        Ok(if nullable {
            PhpType::Nullable(Box::new(t))
        } else {
            t
        })
    }

    fn doc_atom(&mut self, c: &mut Cursor) -> Result<PhpType, String> {
        let name = c.ident();
        if name.is_empty() {
            return Err(self.err(&format!("a type name in the docblock type `{}`", c.s)));
        }
        if c.eat('{') {
            return Err(self.err(&format!(
                "the array shape `{name}{{…}}` has no phorj type — declare a class for it (Tier-2)"
            )));
        }
        if c.eat('<') {
            let mut args = Vec::new();
            loop {
                args.push(self.doc_type(c)?);
                if !c.eat(',') {
                    break;
                }
            }
            if !c.eat('>') {
                return Err(self.err(&format!("`>` in the docblock type `{}`", c.s)));
            }
            let head = name.trim_start_matches("non-empty-");
            return match (head, args.len()) {
                ("list", 1) | ("array", 1 | 2) => Ok(PhpType::Generic {
                    name: head.to_string(),
                    args,
                }),
                _ => Err(self.err(&format!(
                    "the docblock generic `{name}<…>` is Tier-2 — `list<T>`, `array<T>`, `array<K, V>` and `T[]` lift"
                ))),
            };
        }
        Ok(match name.as_str() {
            "mixed" | "callable" | "iterable" | "object" | "resource" | "never" => {
                return Err(self.err(&format!("`{name}` in a docblock type is Tier-2")))
            }
            "integer" => PhpType::Named("int".into()),
            "boolean" => PhpType::Named("bool".into()),
            "double" => PhpType::Named("float".into()),
            _ => PhpType::Named(self.doc_class_name(&name)),
        })
    }

    /// `\A\B\C` in a docblock resolves like an inline root-qualified name: last segment + implicit `use`.
    fn doc_class_name(&mut self, name: &str) -> String {
        let path: Vec<String> = name
            .trim_start_matches('\\')
            .split('\\')
            .map(str::to_string)
            .collect();
        let local = path.last().cloned().unwrap_or_default();
        if path.len() > 1 {
            self.note_implicit_use(path);
        }
        local
    }
}
