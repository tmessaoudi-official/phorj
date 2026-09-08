//! PHP parser — the INTERPOLATION cluster (C-1): a raw double-quoted token is scanned into literal
//! runs and embedded `$`-rooted access chains. Split out of `exprs.rs` under Invariant 13 (lane
//! L1c, 2026-09-07).

use super::*;

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
    // An expression fragment declares nothing, so it carries no PHPDoc.
    let mut p = PParser::new(toks, std::collections::HashMap::new());
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
