//! Item parsing — class declarations + header helpers (`implements`/name lists).

use super::*;

impl Parser {
    /// `[open] class Name<T> [extends A, B] [implements I1, I2] { member* }` — assumes current token
    /// is `class`. The `open` flag is parsed at the item level (`parse_item`) and threaded in.
    pub(in crate::parser) fn parse_class(
        &mut self,
        sp: Span,
        open: bool,
        is_abstract: bool,
        sealed: bool,
        attrs: Vec<Attribute>,
    ) -> Result<ClassDecl, Diagnostic> {
        self.expect(&TokenKind::Class, "'class'")?;
        let name = self.expect_ident("a class name")?;
        // Optional generic parameter list `<T, U>` immediately after the class name (M-RT
        // generics-all), before `extends`/`implements` — `class Box<T> extends … implements … { … }`.
        let (type_params, type_param_bounds) = self.parse_type_params()?;
        // Optional `extends A, B` parent-class list (M-RT S6) — before `implements`.
        let extends = if self.eat(&TokenKind::Extends) {
            self.parse_name_list("a class name after 'extends'")?
        } else {
            Vec::new()
        };
        let (implements, implements_args) = if self.eat(&TokenKind::Implements) {
            self.parse_implements_list()?
        } else {
            (Vec::new(), Vec::new())
        };
        self.expect(&TokenKind::LBrace, "'{' to open class body")?;
        let mut members = Vec::new();
        let mut resolutions = Vec::new();
        let mut uses = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.check(&TokenKind::Eof) {
            // A leading contextual `use`/`rename`/`exclude` (lexed as identifiers, never reserved)
            // introduces a clause rather than a member. Types are PascalCase, so these lowercase
            // leaders are unambiguous in member position. M-RT S8 dot-lookahead: `use P.m` (a `.`
            // after the name) is an S6b resolution clause; `use T;` / `use A, B;` is trait composition.
            let leader = if let TokenKind::Ident(kw) = self.peek() {
                Some(kw.clone())
            } else {
                None
            };
            if let Some(kw) = leader {
                match kw.as_str() {
                    "use" => {
                        let is_resolution = matches!(
                            self.tokens.get(self.pos + 2).map(|t| &t.kind),
                            Some(&TokenKind::Dot)
                        );
                        if is_resolution {
                            resolutions.push(self.parse_resolution()?);
                        } else {
                            uses.extend(self.parse_use_traits()?);
                        }
                        continue;
                    }
                    "rename" | "exclude" => {
                        resolutions.push(self.parse_resolution()?);
                        continue;
                    }
                    _ => {}
                }
            }
            members.push(self.parse_class_member()?);
        }
        self.expect(&TokenKind::RBrace, "'}' to close class")?;
        Ok(ClassDecl {
            vis: Visibility::Public,
            attrs,
            name,
            type_params,
            type_param_bounds,
            extends,
            implements,
            implements_args,
            open,
            is_abstract,
            sealed,
            resolutions,
            uses,
            members,
            foreign: false,
            span: sp,
        })
    }

    /// A comma-separated `implements` list where each name may carry type arguments
    /// (`implements Iterator<int>, Named` — DEC-257 generic interfaces). Returns the names plus a
    /// parallel per-name argument list (empty for an unparameterized name).
    pub(in crate::parser) fn parse_implements_list(
        &mut self,
    ) -> Result<(Vec<String>, Vec<Vec<crate::ast::Type>>), Diagnostic> {
        let mut names = Vec::new();
        let mut args: Vec<Vec<crate::ast::Type>> = Vec::new();
        loop {
            names.push(self.expect_ident("an interface name after 'implements'")?);
            // A `<` here is unambiguous (no expression context in an implements clause): parse
            // `< Type[, Type]* >` directly. `>>` tokenizes as two `Gt`, so nested generics close
            // correctly (`Iterator<List<int>>`).
            if self.eat(&TokenKind::Lt) {
                let mut targs = vec![self.parse_type()?];
                while self.eat(&TokenKind::Comma) {
                    targs.push(self.parse_type()?);
                }
                self.expect(&TokenKind::Gt, "'>' to close the interface type arguments")?;
                args.push(targs);
            } else {
                args.push(Vec::new());
            }
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        Ok((names, args))
    }

    /// A comma-separated list of one-or-more identifiers (no trailing comma), used for a class's
    /// `implements` list and an interface's `extends` list.
    pub(in crate::parser) fn parse_name_list(
        &mut self,
        what: &str,
    ) -> Result<Vec<String>, Diagnostic> {
        let mut names = vec![self.expect_ident(what)?];
        while self.eat(&TokenKind::Comma) {
            names.push(self.expect_ident(what)?);
        }
        Ok(names)
    }
}
