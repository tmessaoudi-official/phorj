//! PHP-lift parser — expressions: precedence climbing, interpolation scanning, chains.

use super::*;

impl PParser {
    pub(super) fn parse_expr(&mut self) -> Result<PhpExpr, String> {
        self.parse_assign()
    }

    /// Assignment level (lowest, right-associative): `=` and the compound forms `+= .= ??= …`.
    pub(super) fn parse_assign(&mut self) -> Result<PhpExpr, String> {
        let lhs = self.parse_ternary()?;
        if self.at(&PTok::Assign) {
            if !is_lvalue(&lhs) {
                return Err(self.err("invalid assignment target"));
            }
            self.advance();
            let value = self.parse_assign()?;
            return Ok(PhpExpr::Assign {
                target: Box::new(lhs),
                value: Box::new(value),
            });
        }
        if let Some(op) = compound_op(self.peek()) {
            if !is_lvalue(&lhs) {
                return Err(self.err("invalid assignment target"));
            }
            self.advance();
            let value = self.parse_assign()?;
            return Ok(PhpExpr::CompoundAssign {
                target: Box::new(lhs),
                op,
                value: Box::new(value),
            });
        }
        Ok(lhs)
    }

    /// Ternary `cond ? then : els` and the elvis form `cond ?: els` (then = `None`).
    pub(super) fn parse_ternary(&mut self) -> Result<PhpExpr, String> {
        let cond = self.parse_coalesce()?;
        if self.eat(&PTok::Question) {
            let then = if self.at(&PTok::Colon) {
                None
            } else {
                Some(Box::new(self.parse_assign()?))
            };
            self.expect(&PTok::Colon, "`:` in ternary")?;
            let els = self.parse_assign()?;
            return Ok(PhpExpr::Ternary {
                cond: Box::new(cond),
                then,
                els: Box::new(els),
            });
        }
        Ok(cond)
    }

    /// Null-coalesce `??` (right-associative, below the left-assoc binary operators).
    pub(super) fn parse_coalesce(&mut self) -> Result<PhpExpr, String> {
        let left = self.parse_binary(0)?;
        if self.eat(&PTok::Coalesce) {
            let right = self.parse_coalesce()?;
            return Ok(PhpExpr::Binary {
                op: PhpBinOp::Coalesce,
                left: Box::new(left),
                right: Box::new(right),
            });
        }
        Ok(left)
    }

    /// Precedence-climbing over the left-associative binary operators (PHP-8 table — see [`infix_op`]).
    pub(super) fn parse_binary(&mut self, min_bp: u8) -> Result<PhpExpr, String> {
        let mut left = self.parse_unary()?;
        while let Some((bp, op)) = infix_op(self.peek()) {
            if bp < min_bp {
                break;
            }
            self.advance();
            let right = self.parse_binary(bp + 1)?;
            left = PhpExpr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    pub(super) fn parse_unary(&mut self) -> Result<PhpExpr, String> {
        self.depth += 1;
        if self.depth > MAX_NEST_DEPTH {
            return Err(self.err("expression nests too deeply"));
        }
        let e = self.parse_unary_inner()?;
        self.depth -= 1;
        Ok(e)
    }

    pub(super) fn parse_unary_inner(&mut self) -> Result<PhpExpr, String> {
        if self.eat(&PTok::Not) {
            return Ok(PhpExpr::Unary {
                op: PhpUnOp::Not,
                expr: Box::new(self.parse_unary()?),
            });
        }
        if self.eat(&PTok::Minus) {
            return Ok(PhpExpr::Unary {
                op: PhpUnOp::Neg,
                expr: Box::new(self.parse_unary()?),
            });
        }
        if self.eat(&PTok::Tilde) {
            return Ok(PhpExpr::Unary {
                op: PhpUnOp::BitNot,
                expr: Box::new(self.parse_unary()?),
            });
        }
        // Prefix increment/decrement.
        if self.at(&PTok::Inc) || self.at(&PTok::Dec) {
            let inc = self.at(&PTok::Inc);
            self.advance();
            let target = self.parse_unary()?;
            if !is_lvalue(&target) {
                return Err(self.err("invalid increment/decrement target"));
            }
            return Ok(PhpExpr::IncDec {
                target: Box::new(target),
                inc,
                prefix: true,
            });
        }
        self.parse_postfix()
    }

    pub(super) fn parse_postfix(&mut self) -> Result<PhpExpr, String> {
        let mut e = self.parse_primary()?;
        loop {
            if self.at(&PTok::LParen) {
                let args = self.parse_args()?;
                e = PhpExpr::Call {
                    callee: Box::new(e),
                    args,
                };
            } else if self.at(&PTok::Arrow) || self.at(&PTok::NullArrow) {
                let nullsafe = self.at(&PTok::NullArrow);
                self.advance();
                let name = self.expect_ident("member name")?;
                if self.at(&PTok::LParen) {
                    let args = self.parse_args()?;
                    e = PhpExpr::MethodCall {
                        recv: Box::new(e),
                        name,
                        args,
                        nullsafe,
                    };
                } else {
                    e = PhpExpr::Member {
                        recv: Box::new(e),
                        name,
                        nullsafe,
                    };
                }
            } else if self.at(&PTok::DoubleColon) {
                e = self.parse_static_access(e)?;
            } else if self.at(&PTok::LBracket) {
                self.advance();
                if self.at(&PTok::RBracket) {
                    return Err(self.err("empty `[]` (array append) is Tier-2"));
                }
                let index = self.parse_expr()?;
                self.expect(&PTok::RBracket, "`]`")?;
                e = PhpExpr::Index {
                    base: Box::new(e),
                    index: Box::new(index),
                };
            } else if self.at(&PTok::Inc) || self.at(&PTok::Dec) {
                let inc = self.at(&PTok::Inc);
                if !is_lvalue(&e) {
                    return Err(self.err("invalid increment/decrement target"));
                }
                self.advance();
                e = PhpExpr::IncDec {
                    target: Box::new(e),
                    inc,
                    prefix: false,
                };
            } else {
                break;
            }
        }
        // C-46: `value instanceof ClassName` — a single, non-associative trailing clause at the
        // postfix level (binds tighter than the `!`/`-`/`~` unary layer above). A dynamic RHS
        // (`$x instanceof $cls`) has no Phorj equivalent and is refused loudly.
        if matches!(self.peek(), PTok::Ident(w) if w == "instanceof") {
            self.advance();
            if matches!(self.peek(), PTok::Var(_)) {
                return Err(self.err("dynamic `instanceof $var` is Tier-2"));
            }
            let class = self.expect_ident("a class name after `instanceof`")?;
            e = PhpExpr::InstanceOf {
                value: Box::new(e),
                class,
            };
        }
        Ok(e)
    }

    /// `Class::CONST` / `Class::$prop` / `Class::method(args)`. The left side must be a class name
    /// (`Name`) — a dynamic `$obj::…` is Tier-3 and rejected.
    pub(super) fn parse_static_access(&mut self, lhs: PhpExpr) -> Result<PhpExpr, String> {
        let class = match lhs {
            PhpExpr::Name(n) => n,
            _ => return Err(self.err("dynamic `::` access is Tier-3")),
        };
        self.advance(); // `::`
        if let PTok::Var(prop) = self.peek().clone() {
            self.advance();
            return Ok(PhpExpr::StaticProp { class, name: prop });
        }
        let name = self.expect_ident("static member name")?;
        if self.at(&PTok::LParen) {
            let args = self.parse_args()?;
            Ok(PhpExpr::StaticCall { class, name, args })
        } else {
            Ok(PhpExpr::ClassConst { class, name })
        }
    }

    /// `( expr, expr, … )` — tolerates a trailing comma.
    pub(super) fn parse_args(&mut self) -> Result<Vec<PhpExpr>, String> {
        self.expect(&PTok::LParen, "`(`")?;
        let mut args = Vec::new();
        while !self.at(&PTok::RParen) {
            args.push(self.parse_expr()?);
            if !self.eat(&PTok::Comma) {
                break;
            }
        }
        self.expect(&PTok::RParen, "`)`")?;
        Ok(args)
    }

    pub(super) fn parse_primary(&mut self) -> Result<PhpExpr, String> {
        match self.peek().clone() {
            PTok::Int(n) => {
                self.advance();
                Ok(PhpExpr::Int(n))
            }
            PTok::Float(f) => {
                self.advance();
                Ok(PhpExpr::Float(f))
            }
            PTok::Str(s) => {
                self.advance();
                Ok(PhpExpr::Str(s))
            }
            PTok::InterpStr(raw) => {
                let raw = raw.clone();
                self.advance();
                Ok(PhpExpr::Interp(parse_interp(&raw)?))
            }
            PTok::Var(name) => {
                self.advance();
                Ok(PhpExpr::Var(name))
            }
            PTok::LParen => {
                // Reject a C-style cast `(int)$x` rather than misparsing it.
                if let PTok::Ident(t) = self.peek_at(1) {
                    if CAST_TYPES.contains(&t.as_str()) && matches!(self.peek_at(2), PTok::RParen) {
                        return Err(self.err("cast expressions are Tier-2"));
                    }
                }
                self.advance();
                let inner = self.parse_expr()?;
                self.expect(&PTok::RParen, "`)`")?;
                Ok(inner)
            }
            PTok::LBracket => self.parse_array(),
            PTok::Ident(word) => self.parse_ident_primary(&word),
            _ => Err(self.err("expected an expression")),
        }
    }

    pub(super) fn parse_ident_primary(&mut self, word: &str) -> Result<PhpExpr, String> {
        match word {
            "true" => {
                self.advance();
                Ok(PhpExpr::Bool(true))
            }
            "false" => {
                self.advance();
                Ok(PhpExpr::Bool(false))
            }
            "null" => {
                self.advance();
                Ok(PhpExpr::Null)
            }
            "new" => self.parse_new(),
            "match" => self.parse_match(),
            "function" | "fn" => Err(self.err("closures and arrow functions are Tier-2")),
            "clone" | "print" | "yield" | "throw" | "include" | "require" | "include_once"
            | "require_once" => Err(self.err(&format!("`{word}` is Tier-2/Tier-3"))),
            _ => {
                self.advance();
                Ok(PhpExpr::Name(word.to_string()))
            }
        }
    }

    pub(super) fn parse_new(&mut self) -> Result<PhpExpr, String> {
        self.advance(); // `new`
        if matches!(self.peek(), PTok::Var(_)) {
            return Err(self.err("dynamic `new $class` is Tier-3"));
        }
        let class = self.expect_ident("class name after `new`")?;
        let args = if self.at(&PTok::LParen) {
            self.parse_args()?
        } else {
            Vec::new()
        };
        Ok(PhpExpr::New { class, args })
    }

    pub(super) fn parse_array(&mut self) -> Result<PhpExpr, String> {
        self.advance(); // `[`
        let mut elems = Vec::new();
        while !self.at(&PTok::RBracket) {
            let first = self.parse_expr()?;
            let elem = if self.eat(&PTok::FatArrow) {
                PhpArrayElem {
                    key: Some(first),
                    value: self.parse_expr()?,
                }
            } else {
                PhpArrayElem {
                    key: None,
                    value: first,
                }
            };
            elems.push(elem);
            if !self.eat(&PTok::Comma) {
                break;
            }
        }
        self.expect(&PTok::RBracket, "`]`")?;
        Ok(PhpExpr::Array(elems))
    }

    pub(super) fn parse_match(&mut self) -> Result<PhpExpr, String> {
        self.advance(); // `match`
        self.expect(&PTok::LParen, "`(`")?;
        let subject = self.parse_expr()?;
        self.expect(&PTok::RParen, "`)`")?;
        self.expect(&PTok::LBrace, "`{`")?;
        let mut arms = Vec::new();
        while !self.at(&PTok::RBrace) {
            let conds = if self.is_kw("default") {
                self.advance();
                None
            } else {
                let mut cs = vec![self.parse_expr()?];
                while self.eat(&PTok::Comma) {
                    if self.at(&PTok::FatArrow) {
                        break; // tolerate a trailing comma before `=>`
                    }
                    cs.push(self.parse_expr()?);
                }
                Some(cs)
            };
            self.expect(&PTok::FatArrow, "`=>` in match arm")?;
            let body = self.parse_expr()?;
            arms.push(PhpMatchArm { conds, body });
            if !self.eat(&PTok::Comma) {
                break;
            }
        }
        self.expect(&PTok::RBrace, "`}`")?;
        Ok(PhpExpr::Match {
            subject: Box::new(subject),
            arms,
        })
    }
}

/// Left binding power + `PhpBinOp` for an infix operator token (the left-associative subset).
/// `??`, ternary, and assignment are handled in their own recursive layers, so they are absent here.
/// **PHP 8 precedence** (higher binds tighter): `* / %` (11) > `+ -` (10) > `<< >>` (9) > `.` (8) >
/// comparison (7) > equality (6) > `&` (5) > `^` (4) > `|` (3) > `&&` (2) > `||` (1). (C-47 inserts
/// the bitwise/shift levels; the prior ops keep their relative order.)
pub(super) fn infix_op(tok: &PTok) -> Option<(u8, PhpBinOp)> {
    Some(match tok {
        PTok::OrOr => (1, PhpBinOp::Or),
        PTok::AndAnd => (2, PhpBinOp::And),
        PTok::Bar => (3, PhpBinOp::BitOr),
        PTok::Caret => (4, PhpBinOp::BitXor),
        PTok::Amp => (5, PhpBinOp::BitAnd),
        PTok::EqEq => (6, PhpBinOp::Eq),
        PTok::EqEqEq => (6, PhpBinOp::Identical),
        PTok::NotEq => (6, PhpBinOp::NotEq),
        PTok::NotEqEq => (6, PhpBinOp::NotIdentical),
        PTok::Lt => (7, PhpBinOp::Lt),
        PTok::Le => (7, PhpBinOp::Le),
        PTok::Gt => (7, PhpBinOp::Gt),
        PTok::Ge => (7, PhpBinOp::Ge),
        PTok::Dot => (8, PhpBinOp::Concat),
        PTok::Shl => (9, PhpBinOp::Shl),
        PTok::Shr => (9, PhpBinOp::Shr),
        PTok::Plus => (10, PhpBinOp::Add),
        PTok::Minus => (10, PhpBinOp::Sub),
        PTok::Star => (11, PhpBinOp::Mul),
        PTok::Slash => (11, PhpBinOp::Div),
        PTok::Percent => (11, PhpBinOp::Rem),
        _ => return None,
    })
}

/// Map a compound-assignment token to the `PhpBinOp` it combines with (`+=` → `Add`, `??=` →
/// `Coalesce`, …). `None` for any non-compound token.
pub(super) fn compound_op(tok: &PTok) -> Option<PhpBinOp> {
    Some(match tok {
        PTok::PlusEq => PhpBinOp::Add,
        PTok::MinusEq => PhpBinOp::Sub,
        PTok::StarEq => PhpBinOp::Mul,
        PTok::SlashEq => PhpBinOp::Div,
        PTok::PercentEq => PhpBinOp::Rem,
        PTok::DotEq => PhpBinOp::Concat,
        PTok::CoalesceEq => PhpBinOp::Coalesce,
        _ => return None,
    })
}

/// A valid assignment / increment target: a variable, an index, an instance/static property.
pub(super) fn is_lvalue(e: &PhpExpr) -> bool {
    matches!(
        e,
        PhpExpr::Var(_)
            | PhpExpr::Index { .. }
            | PhpExpr::Member { .. }
            | PhpExpr::StaticProp { .. }
    )
}

// ── C-1: string interpolation ──
//
// PHP's double-quoted interpolation grammar is exactly a `$`-rooted *access chain* — a variable
// followed by `->prop` / `[idx]` / method-call steps; a top-level operator is a PHP parse error
// (verified against 8.5: `"{$a + $b}"` errors with `expecting "->" or "?->" or "["`). That is also
// precisely Phorj's `"{…}"` hole grammar, so the faithful subset round-trips 1:1. Anything richer
// (variable-variable `${…}`, dynamic `{$o->$p}`, a bareword simple subscript whose key silently
// coerces to a string) is rejected loudly — never lifted to a guess.

/// Parse the raw (undecoded) body of an interpolating double-quoted string into literal runs and
/// embedded access-chain expressions.
pub(super) fn parse_interp(raw: &str) -> Result<Vec<PhpStrPart>, String> {
    let chars: Vec<char> = raw.chars().collect();
    let mut parts: Vec<PhpStrPart> = Vec::new();
    let mut lit = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        // Escape: decode like the lexer's plain-`Str` path (`\$`→`$`, `\{`→`{` for an escaped hole).
        if c == '\\' && i + 1 < chars.len() {
            match chars[i + 1] {
                'n' => lit.push('\n'),
                't' => lit.push('\t'),
                'r' => lit.push('\r'),
                '\\' => lit.push('\\'),
                '"' => lit.push('"'),
                '$' => lit.push('$'),
                '{' => lit.push('{'),
                '0' => lit.push('\0'),
                e => {
                    lit.push('\\');
                    lit.push(e);
                }
            }
            i += 2;
            continue;
        }
        // `${…}` — variable-variable interpolation, removed in PHP 8.2. Reject loudly.
        if c == '$' && chars.get(i + 1) == Some(&'{') {
            return Err(
                "lift parse error: `${…}` interpolation was removed in PHP 8.2 (Tier-2)".into(),
            );
        }
        // Complex form `{$…}` — a full access chain up to the matching `}`.
        if c == '{' && chars.get(i + 1) == Some(&'$') {
            flush_lit(&mut lit, &mut parts);
            let (inner, consumed) = scan_braced(&chars[i..])?;
            parts.push(PhpStrPart::Expr(Box::new(parse_interp_chain(&inner)?)));
            i += consumed;
            continue;
        }
        // Simple form `$name[...]?` / `$name->prop?` — one optional access step (PHP simple syntax).
        if c == '$'
            && chars
                .get(i + 1)
                .is_some_and(|n| n.is_alphabetic() || *n == '_')
        {
            flush_lit(&mut lit, &mut parts);
            let (expr, consumed) = parse_simple_interp(&chars[i..])?;
            parts.push(PhpStrPart::Expr(Box::new(expr)));
            i += consumed;
            continue;
        }
        lit.push(c);
        i += 1;
    }
    flush_lit(&mut lit, &mut parts);
    if parts.is_empty() {
        parts.push(PhpStrPart::Lit(String::new()));
    }
    Ok(parts)
}

pub(super) fn flush_lit(lit: &mut String, parts: &mut Vec<PhpStrPart>) {
    if !lit.is_empty() {
        parts.push(PhpStrPart::Lit(std::mem::take(lit)));
    }
}

/// Scan a balanced `{ … }` run (quote-aware) starting at `chars[0] == '{'`. Returns the inner text
/// (without the braces) and the number of chars consumed (including both braces).
pub(super) fn scan_braced(chars: &[char]) -> Result<(String, usize), String> {
    let mut depth = 0usize;
    let mut quote: Option<char> = None;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if let Some(q) = quote {
            if c == '\\' {
                i += 2;
                continue;
            }
            if c == q {
                quote = None;
            }
            i += 1;
            continue;
        }
        match c {
            '\'' | '"' => quote = Some(c),
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    let inner: String = chars[1..i].iter().collect();
                    return Ok((inner, i + 1));
                }
            }
            _ => {}
        }
        i += 1;
    }
    Err("lift parse error: unterminated `{…}` interpolation".into())
}

/// Parse a complex-form inner (`$o->total`, `$a[$k]`, `$o->label()`) as a `$`-rooted access chain.
/// Reuses the real PHP postfix parser, then rejects anything that isn't a pure chain (a leftover
/// operator token means a top-level operator was present).
pub(super) fn parse_interp_chain(inner: &str) -> Result<PhpExpr, String> {
    let toks = lex_php(&format!("<?php {inner}"))?;
    let mut p = PParser {
        toks,
        pos: 0,
        depth: 0,
    };
    p.eat(&PTok::OpenTag);
    let e = p.parse_postfix()?;
    if !matches!(p.peek(), PTok::Eof) {
        return Err(format!(
            "lift parse error: interpolation `{{{inner}}}` must be a $-rooted access chain \
             (a top-level operator is Tier-2)"
        ));
    }
    if !is_php_access_chain(&e) {
        return Err(format!(
            "lift parse error: interpolation `{{{inner}}}` must be rooted at a variable \
             (dynamic/variable-variable forms are Tier-2)"
        ));
    }
    Ok(e)
}

/// Parse a simple-form interpolation starting at `chars[0] == '$'`: a variable, then at most ONE
/// `->prop` or `[idx]` step (PHP simple syntax). A bareword subscript silently coerces to a string
/// key in PHP — reject it loudly and nudge to the explicit `{$a['key']}` form.
pub(super) fn parse_simple_interp(chars: &[char]) -> Result<(PhpExpr, usize), String> {
    let mut i = 1; // skip `$`
    let start = i;
    while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
        i += 1;
    }
    let name: String = chars[start..i].iter().collect();
    let mut expr = if name == "this" {
        PhpExpr::Var("this".into())
    } else {
        PhpExpr::Var(name)
    };
    // One optional `->prop` (single level in simple syntax).
    if chars.get(i) == Some(&'-') && chars.get(i + 1) == Some(&'>') {
        let ps = i + 2;
        let mut j = ps;
        while j < chars.len() && (chars[j].is_alphanumeric() || chars[j] == '_') {
            j += 1;
        }
        if j > ps {
            let prop: String = chars[ps..j].iter().collect();
            expr = PhpExpr::Member {
                recv: Box::new(expr),
                name: prop,
                nullsafe: false,
            };
            i = j;
        }
        // No name after `->` ⇒ the `->` is literal text (PHP prints the value then `->`).
    } else if chars.get(i) == Some(&'[') {
        // One optional `[idx]` — integer or `$var` only (a bareword key is the coercion trap).
        let sub_start = i + 1;
        let mut j = sub_start;
        while j < chars.len() && chars[j] != ']' {
            j += 1;
        }
        if j >= chars.len() {
            return Err("lift parse error: unterminated `[…]` in interpolation".into());
        }
        let sub: String = chars[sub_start..j].iter().collect();
        let sub = sub.trim();
        let index = if let Some(var) = sub.strip_prefix('$') {
            if var.is_empty()
                || !var
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_alphabetic() || c == '_')
            {
                return Err("lift parse error: malformed `[$…]` subscript in interpolation".into());
            }
            PhpExpr::Var(var.to_string())
        } else if let Ok(n) = sub.parse::<i64>() {
            PhpExpr::Int(n)
        } else {
            return Err(format!(
                "lift parse error: simple-syntax bareword subscript `[{sub}]` coerces to a string \
                 key — use the explicit `{{$…['{sub}']}}` form (Tier-2)"
            ));
        };
        expr = PhpExpr::Index {
            base: Box::new(expr),
            index: Box::new(index),
        };
        i = j + 1;
    }
    Ok((expr, i))
}

/// A `$`-rooted access chain: a variable optionally followed by property / index / method-call
/// steps. Method-call arguments and index expressions are not part of the spine (they are lifted
/// independently), so only the receiver spine must bottom out at a variable.
pub(super) fn is_php_access_chain(e: &PhpExpr) -> bool {
    match e {
        PhpExpr::Var(_) => true,
        PhpExpr::Member { recv, .. } | PhpExpr::MethodCall { recv, .. } => {
            is_php_access_chain(recv)
        }
        PhpExpr::Index { base, .. } => is_php_access_chain(base),
        _ => false,
    }
}
