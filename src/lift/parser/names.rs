//! PHP-lift parser — root-qualified names and primitive casts (Lane R-3, 2026-09-05).
//!
//! Both were measured on a real app before being built: scout writes `\Scout\Rent\Config\X::m()`
//! and `\count(…)` inline in 16 of its 120 files and `(float) $x`-style casts in 38.
//!
//! A root-qualified inline name is, in PHP, exactly a `use` plus a reference to its last segment —
//! it binds no local name and resolves the same way. So the parser records it as an IMPLICIT `use`
//! and hands the lifter the last segment; `parse_program` merges the implicit list into the explicit
//! one, and the lifter's single import mechanism (LIFT-NS: emit only what the draft references,
//! refuse two imports under one local name) covers both. A single-segment `\count` / `\Closure` only
//! loses the root marker — that changes meaning solely when a same-named class exists in the file's
//! own namespace, which is disclosed here rather than engineered around.

use super::*;

impl PParser {
    /// `(int)` / `(integer)` / `(float)` / … at the cursor — the C-style cast shape `( type )`.
    pub(super) fn at_cast(&self) -> bool {
        self.at(&PTok::LParen)
            && matches!(self.peek_at(1), PTok::Ident(t) if CAST_TYPES.contains(&t.as_str()))
            && matches!(self.peek_at(2), PTok::RParen)
    }

    /// `(T) operand` → [`PhpExpr::Cast`] for the four primitives (aliases folded); `(array)`,
    /// `(object)` and the rest stay Tier-2 by name. The operand binds like a unary operator.
    pub(super) fn parse_cast(&mut self) -> Result<PhpExpr, String> {
        self.advance(); // `(`
        let PTok::Ident(t) = self.peek().clone() else {
            unreachable!("at_cast checked the token")
        };
        let ty = match t.as_str() {
            "int" | "integer" => "int",
            "float" | "double" => "float",
            "string" => "string",
            "bool" | "boolean" => "bool",
            other => {
                return Err(self.err(&format!(
                    "a `({other})` cast is Tier-2 — only `(int)`, `(float)`, `(string)` and `(bool)` lift"
                )))
            }
        }
        .to_string();
        self.advance();
        self.expect(&PTok::RParen, "`)` after the cast type")?;
        let value = Box::new(self.parse_unary()?);
        Ok(PhpExpr::Cast { ty, value })
    }

    /// A `\`-rooted name in expression position: `\A\B\C` → `Name("C")` plus an implicit `use A\B\C`;
    /// `\count` → `Name("count")`.
    pub(super) fn parse_root_qualified(&mut self) -> Result<PhpExpr, String> {
        Ok(PhpExpr::Name(self.root_qualified_local()?))
    }

    /// The same, in TYPE position (`private \Closure $f`, `\App\Money $m`).
    pub(super) fn parse_root_qualified_type(&mut self) -> Result<PhpType, String> {
        Ok(PhpType::Named(self.root_qualified_local()?))
    }

    pub(super) fn root_qualified_local(&mut self) -> Result<String, String> {
        let full = self.parse_qualified_name()?;
        let path: Vec<String> = full
            .trim_start_matches('\\')
            .split('\\')
            .map(str::to_string)
            .collect();
        let local = path.last().cloned().unwrap_or_default();
        if path.len() > 1 {
            self.note_implicit_use(path);
        }
        Ok(local)
    }

    /// Record a multi-segment path as an implicit `use` (once per path). Shared with the docblock
    /// type reader, so `@return list<\\A\\B\\C>` imports `C` exactly like an inline `\\A\\B\\C`.
    pub(super) fn note_implicit_use(&mut self, path: Vec<String>) {
        // `\App\Scorer` written INSIDE `namespace App` names a symbol of this very file. PHP
        // resolves it without a `use`, and lifting it to `import App.Scorer;` produced a package
        // importing itself — `E-MODULE-NOT-FOUND: no package App.Scorer (or App)`, which reads as a
        // missing dependency rather than as the self-reference it is. Found by extending
        // `examples/lift/real-shapes.php` with an `instanceof` of its own interface (2026-09-05).
        if path.len() > 1 && path[..path.len() - 1] == self.namespace[..] {
            return;
        }
        if !self.implicit_uses.iter().any(|u| u.path == path) {
            let line = self.line();
            self.implicit_uses.push(PhpUse {
                path,
                alias: None,
                line,
            });
        }
    }

    /// One call/construction/attribute argument: `name: value` (PHP 8.0 named argument — phorj
    /// accepts them in the same positions, DEC-297 / DEC-435, so they lift 1:1) or an expression.
    /// The `name :` lookahead cannot collide with a static access — `::` is its own token.
    pub(super) fn parse_arg(&mut self) -> Result<PhpExpr, String> {
        if let PTok::Ident(n) = self.peek().clone() {
            if matches!(self.peek_at(1), PTok::Colon) {
                self.advance(); // name
                self.advance(); // `:`
                return Ok(PhpExpr::NamedArg {
                    name: n,
                    value: Box::new(self.parse_expr()?),
                });
            }
        }
        self.parse_expr()
    }

    /// Fold the implicit `use`s into the explicit list — an explicit `use` of the same path wins, so a
    /// hand-written alias is never shadowed by a synthesized plain import.
    pub(super) fn merge_implicit_uses(&mut self, uses: &mut Vec<PhpUse>) {
        for u in std::mem::take(&mut self.implicit_uses) {
            if !uses.iter().any(|x| x.path == u.path) {
                uses.push(u);
            }
        }
    }
}
