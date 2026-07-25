//! Item parsing — foreign `declare function` / `declare class` interop (M8.5).

use super::*;

impl Parser {
    /// Parse a `declare …` foreign-symbol declaration (M8.5 interop). Currently `declare function
    /// name(params) -> ret;` — a bodyless signature describing an existing PHP function. The result is a
    /// `FunctionDecl` with `foreign: true` and an empty body; the checker validates calls against it but
    /// skips the body, interp/VM refuse the program (`E-FOREIGN-RUNTIME`), and the transpiler emits
    /// `\name(…)`. (`declare class` is M8.5 S2.)
    pub(in crate::parser) fn parse_declare(&mut self, sp: Span) -> Result<Item, Diagnostic> {
        self.expect_ident("'declare'")?; // consume the contextual `declare`
        if self.check(&TokenKind::Class) {
            return self.parse_declare_class(sp);
        }
        self.expect(&TokenKind::Function, "'function' after 'declare'")?;
        let name = self.expect_ident("a foreign function name")?;
        let (type_params, type_param_bounds) = self.parse_type_params()?;
        self.expect(&TokenKind::LParen, "'(' after function name")?;
        let params = self.parse_params()?;
        self.expect(&TokenKind::RParen, "')' to close parameters")?;
        let ret = if self.eat(&TokenKind::Colon) || self.eat(&TokenKind::Arrow) {
            Some(self.parse_type()?)
        } else {
            None
        };
        self.expect(
            &TokenKind::Semicolon,
            "';' after a foreign function declaration (it has no body)",
        )?;
        Ok(Item::Function(FunctionDecl {
            modifiers: Vec::new(),
            attrs: Vec::new(),
            vis: Visibility::Public,
            name,
            type_params,
            type_param_bounds,
            params,
            ret,
            throws: Vec::new(),
            body: Vec::new(),
            foreign: true,
            generic_ret_from_param: None,
            span: sp,
        }))
    }

    /// Parse a `declare class Name { … }` foreign-PHP class (M8.5 S2). Members are bodyless signatures
    /// terminated by `;`: a `constructor(params);`, `[static] function name(params) -> ret;`, and
    /// `[public] Type name;` fields. The result is a `ClassDecl` with `foreign: true`; each method is
    /// also `foreign: true` so the checker skips body/totality/casing for it. The transpiler emits
    /// references as the global PHP form (`new \Name`, `\Name::s`, `$o->m`) and no class definition.
    pub(in crate::parser) fn parse_declare_class(&mut self, sp: Span) -> Result<Item, Diagnostic> {
        self.expect(&TokenKind::Class, "'class' after 'declare'")?;
        let name = self.expect_ident("a foreign class name")?;
        // S3a: an optional `extends`/`implements` header describes the *PHP* hierarchy — a foreign
        // exception writes `implements Error` (the built-in marker), making it catchable; a foreign
        // class may also `extends` another foreign class. Purely a type-checker input (no body).
        let extends = if self.eat(&TokenKind::Extends) {
            self.parse_name_list("a class name after 'extends'")?
        } else {
            Vec::new()
        };
        let implements = if self.eat(&TokenKind::Implements) {
            self.parse_name_list("an interface name after 'implements'")?
        } else {
            Vec::new()
        };
        self.expect(&TokenKind::LBrace, "'{' to open the foreign class body")?;
        let mut members = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.check(&TokenKind::Eof) {
            let msp = self.peek_span();
            let modifiers = self.parse_modifiers();
            match self.peek() {
                TokenKind::Constructor => {
                    self.advance();
                    self.expect(&TokenKind::LParen, "'(' after 'constructor'")?;
                    let params = self.parse_ctor_params()?;
                    self.expect(&TokenKind::RParen, "')' to close constructor parameters")?;
                    // DEC-221: a foreign constructor may declare `throws` too (between `)` and `;`),
                    // describing the PHP constructor's failure surface for the checker.
                    let throws = if self.eat(&TokenKind::Throws) {
                        self.parse_throws_clause()?
                    } else {
                        Vec::new()
                    };
                    self.expect(
                        &TokenKind::Semicolon,
                        "';' after a foreign constructor signature",
                    )?;
                    members.push(ClassMember::Constructor {
                        modifiers,
                        params,
                        throws,
                        body: Vec::new(),
                        span: msp,
                    });
                }
                TokenKind::Function => {
                    self.advance();
                    let mname = self.expect_ident("a method name")?;
                    self.expect(&TokenKind::LParen, "'(' after method name")?;
                    let params = self.parse_params()?;
                    self.expect(&TokenKind::RParen, "')' to close parameters")?;
                    let ret = if self.eat(&TokenKind::Colon) || self.eat(&TokenKind::Arrow) {
                        Some(self.parse_type()?)
                    } else {
                        None
                    };
                    self.expect(
                        &TokenKind::Semicolon,
                        "';' after a foreign method signature",
                    )?;
                    members.push(ClassMember::Method(FunctionDecl {
                        modifiers,
                        attrs: Vec::new(),
                        vis: Visibility::Public,
                        name: mname,
                        type_params: Vec::new(),
                        type_param_bounds: Vec::new(),
                        params,
                        ret,
                        throws: Vec::new(),
                        body: Vec::new(),
                        // The enclosing class is foreign; the *method's* own flag stays false so it is
                        // not mistaken for a free `declare function` by the formatter. The checker skips
                        // its body/totality wholesale (the foreign class is not body-checked), and the
                        // formatter prints it via the `declare class` path.
                        foreign: false,
                        generic_ret_from_param: None,
                        span: msp,
                    }));
                }
                _ => {
                    // A field: `[public] Type name;` — the type describes a readable PHP property.
                    let ty = self.parse_type()?;
                    let fname = self.expect_ident("a foreign field name")?;
                    self.expect(&TokenKind::Semicolon, "';' after a foreign field")?;
                    members.push(ClassMember::Field {
                        modifiers,
                        ty,
                        name: fname,
                        init: None,
                        span: msp,
                    });
                }
            }
        }
        self.expect(&TokenKind::RBrace, "'}' to close the foreign class")?;
        Ok(Item::Class(ClassDecl {
            vis: Visibility::Public,
            attrs: Vec::new(), // a foreign `declare` rejects attributes (checked above)
            name,
            type_params: Vec::new(),
            type_param_bounds: Vec::new(),
            extends,
            implements_args: vec![Vec::new(); implements.len()],
            implements,
            open: false,
            is_abstract: false,
            sealed: false,
            resolutions: Vec::new(),
            uses: Vec::new(),
            members,
            foreign: true,
            span: sp,
        }))
    }
}
