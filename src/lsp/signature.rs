//! `textDocument/signatureHelp` — the parameter hint shown while typing a call's arguments.
//!
//! Invariant 17 names signature help explicitly in its 100% RULE ("completion, hover,
//! go-to-definition, find-usages, document symbols, diagnostics with the right LSP tags, signature
//! help"), and it was the one named capability the server did not advertise. The four editors'-eye
//! surfaces are not interchangeable: hover answers *what is this symbol*, signature help answers
//! *which argument am I typing right now*, and no amount of the former substitutes for the latter
//! inside a three-argument call.
//!
//! Two halves, deliberately separated so each is testable alone:
//!
//! * [`call_at`] is pure text analysis — which call encloses the cursor, and which argument index it
//!   sits in. It scans FORWARD from the start of the buffer rather than backward from the cursor,
//!   because a backward scan cannot tell an unclosed `(` from one closed further left without
//!   re-deriving the same nesting anyway, and it cannot skip a string literal correctly at all (the
//!   scanner would have to know whether the quote it lands on opens or closes).
//! * [`native_signature`] renders a `native::registry()` row as a signature; a user declaration's
//!   signature is the text hover already renders, and [`split_params`] slices either into the LSP
//!   `ParameterInformation` list.
//!
//! The parameter list is derived by slicing the SAME signature text hover shows, so the two can never
//! disagree about what a function's parameters are.

use crate::native;

/// The call enclosing the cursor: the callee as written, and the 0-based index of the argument the
/// cursor is inside.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct CallSite {
    /// The callee exactly as written — `helper`, or a dotted path such as `Output.printLine`.
    pub callee: String,
    /// 0-based argument index, i.e. the number of top-level commas between `(` and the cursor.
    pub active: u32,
}

/// Skip a string literal starting at `i` (which indexes the opening quote), returning the index just
/// past its closing quote. Handles `\"` escapes and the triple-quoted form.
///
/// Getting this wrong is not cosmetic: an unskipped `"a, b)"` would contribute a phantom comma and a
/// phantom close-paren, so the cursor would be reported in the wrong argument of the wrong call.
fn skip_string(b: &[u8], i: usize) -> usize {
    let n = b.len();
    // Triple-quoted: ends at the next `"""`, with no escape processing.
    if i + 2 < n && b[i + 1] == b'"' && b[i + 2] == b'"' {
        let mut j = i + 3;
        while j + 2 < n {
            if b[j] == b'"' && b[j + 1] == b'"' && b[j + 2] == b'"' {
                return j + 3;
            }
            j += 1;
        }
        return n;
    }
    let mut j = i + 1;
    while j < n {
        match b[j] {
            b'\\' => j += 2,
            b'"' => return j + 1,
            _ => j += 1,
        }
    }
    n
}

/// The innermost call whose argument list contains `offset`, or `None` when the cursor is not inside
/// one.
pub(super) fn call_at(text: &str, offset: usize) -> Option<CallSite> {
    let b = text.as_bytes();
    let end = offset.min(b.len());
    // (byte of the opening bracket, top-level commas seen inside it).
    let mut stack: Vec<(u8, usize, u32)> = Vec::new();
    let mut i = 0usize;
    while i < end {
        match b[i] {
            b'"' => {
                i = skip_string(b, i);
                continue;
            }
            b'/' if i + 1 < b.len() && b[i + 1] == b'/' => {
                while i < end && b[i] != b'\n' {
                    i += 1;
                }
                continue;
            }
            b'/' if i + 1 < b.len() && b[i + 1] == b'*' => {
                i += 2;
                while i + 1 < end && !(b[i] == b'*' && b[i + 1] == b'/') {
                    i += 1;
                }
                i += 2;
                continue;
            }
            c @ (b'(' | b'[' | b'{') => stack.push((c, i, 0)),
            b')' | b']' | b'}' => {
                stack.pop();
            }
            b',' => {
                if let Some(top) = stack.last_mut() {
                    top.2 += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    // The innermost frame that is a PAREN. A cursor inside `f(xs[|])` is still typing `f`'s first
    // argument, so an intervening `[` or `{` must be skipped, not treated as the call.
    let (_, open, commas) = *stack.iter().rev().find(|(c, _, _)| *c == b'(')?;
    let callee = callee_before(text, open)?;
    Some(CallSite {
        callee,
        active: commas,
    })
}

/// The callee immediately left of the `(` at `open`: an identifier, or a dotted path such as
/// `Output.printLine`. `None` when what precedes is not a name — a grouping paren, a `while (`, or
/// the parameter list of a declaration.
fn callee_before(text: &str, open: usize) -> Option<String> {
    let b = text.as_bytes();
    let mut e = open;
    while e > 0 && (b[e - 1] as char).is_whitespace() {
        e -= 1;
    }
    let mut s = e;
    while s > 0 {
        let c = b[s - 1] as char;
        if c.is_alphanumeric() || c == '_' || c == '.' {
            s -= 1;
        } else {
            break;
        }
    }
    if s == e {
        return None;
    }
    let name = text.get(s..e)?.trim_matches('.');
    if name.is_empty() {
        return None;
    }
    // A keyword followed by `(` is control flow, not a call. `function` is here because a
    // declaration's own parameter list would otherwise report the function as calling itself.
    const KEYWORDS: [&str; 9] = [
        "if", "while", "for", "foreach", "switch", "match", "catch", "return", "function",
    ];
    if KEYWORDS.contains(&name) {
        return None;
    }
    // A name preceded by `function` is the DECLARATION's own parameter list, not a call to it.
    // Checking the name alone missed this: `function helper(` reported `helper` calling itself.
    let mut p = s;
    while p > 0 && (b[p - 1] as char).is_whitespace() {
        p -= 1;
    }
    if text[..p].ends_with("function") {
        return None;
    }
    Some(name.to_string())
}

/// Split a rendered signature's parameter list into its individual parameters, at top level.
///
/// Derived from the same signature text hover renders, so hover and signature help cannot disagree
/// about a function's parameters. Nested generics (`Map<string, int> m`) must NOT split on their
/// inner comma, which is why this tracks depth over `<`/`(`/`[` rather than splitting on `,`.
pub(super) fn split_params(signature: &str) -> Vec<String> {
    let Some(open) = signature.find('(') else {
        return Vec::new();
    };
    let b = signature.as_bytes();
    let (mut depth, mut i, mut start) = (0i32, open, open + 1);
    let mut out = Vec::new();
    while i < b.len() {
        match b[i] {
            b'(' | b'[' | b'<' => depth += 1,
            b')' | b']' | b'>' => {
                depth -= 1;
                if depth == 0 {
                    let p = signature[start..i].trim();
                    if !p.is_empty() {
                        out.push(p.to_string());
                    }
                    return out;
                }
            }
            b',' if depth == 1 => {
                let p = signature[start..i].trim();
                if !p.is_empty() {
                    out.push(p.to_string());
                }
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    out
}

/// The `native::registry()` row a dotted callee names, if any — `Output.printLine` → the
/// `Core.Output`/`printLine` row.
///
/// Matched on the module's LAST segment, which is how a leaf qualifier is written after an import,
/// and `Core.Native.*` twins are excluded for the reason `catalog::module_members` gives: their leaf
/// collides with the friendly class name and they are internals users must never call.
pub(super) fn native_signature(callee: &str) -> Option<String> {
    let (qualifier, name) = callee.rsplit_once('.')?;
    let qualifier = qualifier.rsplit('.').next()?;
    let n = native::registry()
        .iter()
        .filter(|n| !n.module.starts_with("Core.Native."))
        .find(|n| n.module.rsplit('.').next() == Some(qualifier) && n.name == name)?;
    let params: Vec<String> = n.params.iter().map(|t| t.to_string()).collect();
    Some(format!(
        "function {}.{}({}): {}",
        qualifier,
        n.name,
        params.join(", "),
        n.ret
    ))
}
