//! A minimal Wadler/prettier-style document IR + fits solver — the width-canonical core of
//! `phg fmt`. The expression printer (`printer::expr_doc`) builds a [`Doc`]; [`render`] lays it out
//! against a column budget, breaking a [`Doc::Group`] only when its flat form would overflow.
//!
//! **Why this makes `phg fmt` idempotent by construction:** a `Doc` is derived purely from the parsed
//! AST (never from the source's whitespace), and [`render`] is a deterministic function of
//! `(doc, width, base_indent, start_col)`. Re-parsing the formatter's output rebuilds the *same* AST
//! → the same `Doc` → the same bytes. This is why DEC-187 dropped "preserve author breaks" (Rule 1):
//! width-canonical layout needs no source access, which the printer deliberately lacks.
//!
//! **Invariant — expression Docs contain no hard break.** Every [`Doc::Line`]/[`Doc::SoftLine`] is a
//! *soft* break, meaningful only when its enclosing [`Doc::Group`] is laid out broken. Statements own
//! the hard line breaks (in the imperative printer). Because there is no hard break, forced-flat
//! rendering (used inside string-interpolation holes, where a newline would corrupt the string value)
//! is always well defined: every soft break collapses to a space or nothing.

#[derive(Clone, Debug)]
pub enum Doc {
    /// Verbatim text. MUST NOT contain a newline (string literals escape `\n` → `\\n`, so a printed
    /// literal is always single-line).
    Text(String),
    /// A space when flat; a newline + current indent when the enclosing group breaks.
    Line,
    /// Nothing when flat; a newline + current indent when the enclosing group breaks.
    SoftLine,
    /// A sequence of docs, laid out left to right.
    Concat(Vec<Doc>),
    /// Increase the indentation of any break-line inside `.1` by `.0` columns.
    Nest(usize, Box<Doc>),
    /// A break-group: rendered flat if its flat form fits the remaining width, else broken.
    Group(Box<Doc>),
}

/// `text(s)` — a verbatim fragment.
pub fn text(s: impl Into<String>) -> Doc {
    Doc::Text(s.into())
}

/// A soft break that is a space when flat.
pub fn line() -> Doc {
    Doc::Line
}

/// A soft break that is nothing when flat.
pub fn softline() -> Doc {
    Doc::SoftLine
}

/// Concatenate a sequence of docs.
pub fn concat(ds: Vec<Doc>) -> Doc {
    Doc::Concat(ds)
}

/// Indent break-lines inside `d` by `n` columns.
pub fn nest(n: usize, d: Doc) -> Doc {
    Doc::Nest(n, Box::new(d))
}

/// Wrap `d` in a break-group (flat if it fits, else broken).
pub fn group(d: Doc) -> Doc {
    Doc::Group(Box::new(d))
}

/// Join `parts` with `sep` between each (no trailing separator).
pub fn join(parts: Vec<Doc>, sep: Doc) -> Doc {
    let mut out = Vec::with_capacity(parts.len().saturating_mul(2));
    for (i, p) in parts.into_iter().enumerate() {
        if i > 0 {
            out.push(sep.clone());
        }
        out.push(p);
    }
    Doc::Concat(out)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Flat,
    Break,
}

/// One pending render command: `(indent_columns, mode, doc)`. `&Doc` + `usize` + `Mode` are all
/// `Copy`, so the whole tuple is `Copy` — [`fits`] can cheaply snapshot the remaining stack.
type Cmd<'a> = (usize, Mode, &'a Doc);

/// Would the flat layout of `group_inner` (followed by whatever `rest` holds, in its own modes) reach
/// the end of the current line — i.e. hit a broken newline, or run out of docs — WITHOUT exceeding
/// `remaining` columns? Mirrors the classic Wadler/prettier `fits`: nested groups are measured flat,
/// and the first break encountered in an outer (already-`Break`) context ends the line successfully.
fn fits(mut remaining: isize, group_inner: Cmd, rest: &[Cmd]) -> bool {
    let mut local: Vec<Cmd> = vec![group_inner];
    let mut rest_top = rest.len();
    loop {
        if remaining < 0 {
            return false;
        }
        let (indent, mode, doc) = match local.pop() {
            Some(c) => c,
            None if rest_top > 0 => {
                rest_top -= 1;
                rest[rest_top]
            }
            // Everything up to end-of-input fit within budget.
            None => return remaining >= 0,
        };
        match doc {
            Doc::Text(s) => remaining -= s.chars().count() as isize,
            Doc::Line => match mode {
                Mode::Flat => remaining -= 1,
                Mode::Break => return true,
            },
            Doc::SoftLine => match mode {
                Mode::Flat => {}
                Mode::Break => return true,
            },
            Doc::Concat(ds) => {
                for d in ds.iter().rev() {
                    local.push((indent, mode, d));
                }
            }
            Doc::Nest(n, d) => local.push((indent + n, mode, d)),
            Doc::Group(d) => local.push((indent, Mode::Flat, d)),
        }
    }
}

/// The column budget for the width-canonical formatter (rustfmt's default). A line is broken to keep
/// its rendered width within this many columns where a group's break points allow.
pub const MAX_WIDTH: usize = 100;

fn pad(out: &mut String, cols: usize) {
    for _ in 0..cols {
        out.push(' ');
    }
}

/// Render `doc` to a (possibly multi-line) string.
///
/// * `width` — the column budget (normally [`MAX_WIDTH`]).
/// * `base_indent` — the absolute column every broken continuation line starts at (the enclosing
///   statement's indent, in columns); `nest` adds to it.
/// * `start_col` — the column the FIRST character lands on (the statement prefix already consumed
///   `start_col - <line-1 indent the caller adds>` columns). Only affects the fits math; the first
///   line is emitted WITHOUT leading indentation (the caller's line emitter prepends line-1 indent).
/// * `force_flat` — render every group flat (used inside interpolation holes, where a newline would
///   change the string's value). Safe because expression docs contain no hard break.
pub fn render(
    doc: &Doc,
    width: usize,
    base_indent: usize,
    start_col: usize,
    force_flat: bool,
) -> String {
    let mut out = String::new();
    let mut col = start_col;
    let init_mode = if force_flat { Mode::Flat } else { Mode::Break };
    let mut stack: Vec<Cmd> = vec![(base_indent, init_mode, doc)];
    while let Some((indent, mode, d)) = stack.pop() {
        match d {
            Doc::Text(s) => {
                out.push_str(s);
                col += s.chars().count();
            }
            Doc::Concat(ds) => {
                for x in ds.iter().rev() {
                    stack.push((indent, mode, x));
                }
            }
            Doc::Nest(n, x) => stack.push((indent + n, mode, x)),
            Doc::Line => match mode {
                Mode::Flat => {
                    out.push(' ');
                    col += 1;
                }
                Mode::Break => {
                    out.push('\n');
                    pad(&mut out, indent);
                    col = indent;
                }
            },
            Doc::SoftLine => match mode {
                Mode::Flat => {}
                Mode::Break => {
                    out.push('\n');
                    pad(&mut out, indent);
                    col = indent;
                }
            },
            Doc::Group(inner) => {
                let flat = force_flat
                    || fits(
                        width as isize - col as isize,
                        (indent, Mode::Flat, inner),
                        &stack,
                    );
                let m = if flat { Mode::Flat } else { Mode::Break };
                stack.push((indent, m, inner));
            }
        }
    }
    out
}

/// Render `doc` on a single line, collapsing every soft break — for embedding an expression inside a
/// string-interpolation hole, where a real newline would corrupt the string value.
pub fn render_flat(doc: &Doc) -> String {
    render(doc, MAX_WIDTH, 0, 0, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A group that fits stays flat: `Line` → a single space.
    #[test]
    fn group_fits_stays_flat() {
        let d = group(concat(vec![text("a"), line(), text("b")]));
        assert_eq!(render(&d, 80, 0, 0, false), "a b");
    }

    /// A group that overflows breaks: `Line` → newline + indent, `SoftLine` → newline + indent.
    #[test]
    fn group_overflows_breaks() {
        let d = group(concat(vec![text("aaa"), line(), text("bbb")]));
        // width 4 cannot hold "aaa bbb" (7) → break.
        assert_eq!(render(&d, 4, 0, 0, false), "aaa\nbbb");
    }

    /// `nest` indents the continuation lines of a broken group; the classic call-args shape.
    #[test]
    fn nested_break_indents_continuations() {
        // f( <softline> x, <line> y <softline-dedent> )
        let args = nest(4, concat(vec![softline(), text("x,"), line(), text("y")]));
        let d = group(concat(vec![text("f("), args, softline(), text(")")]));
        // "f(x, y)" is 7 wide, so it stays flat at width 8 …
        assert_eq!(render(&d, 8, 0, 0, false), "f(x, y)");
        // … and breaks at width 6: args indent to column 4, closing ) dedents to 0.
        assert_eq!(render(&d, 6, 0, 0, false), "f(\n    x,\n    y\n)");
    }

    /// `softline` is nothing in the flat layout.
    #[test]
    fn softline_is_empty_when_flat() {
        let d = group(concat(vec![
            text("["),
            softline(),
            text("a"),
            softline(),
            text("]"),
        ]));
        assert_eq!(render(&d, 80, 0, 0, false), "[a]");
    }

    /// `force_flat` collapses even a group that would otherwise overflow — the interpolation-hole path.
    #[test]
    fn force_flat_never_breaks() {
        let d = group(concat(vec![text("aaa"), line(), text("bbb")]));
        assert_eq!(render(&d, 2, 0, 0, true), "aaa bbb");
        assert_eq!(render_flat(&d), "aaa bbb");
    }

    /// `start_col` counts against the budget so a group that fits on a fresh line still breaks when it
    /// starts far to the right (the statement-prefix case).
    #[test]
    fn start_col_consumes_budget() {
        let d = group(concat(vec![text("aa"), line(), text("bb")]));
        // "aa bb" is 5 wide and fits width 8 from col 0 …
        assert_eq!(render(&d, 8, 0, 0, false), "aa bb");
        // … but starting at col 5 it would reach col 10 > 8 → break.
        assert_eq!(render(&d, 8, 0, 5, false), "aa\nbb");
    }

    /// The `fits` check accounts for trailing content after the group (the `rest` stack) — a group
    /// that fits alone still breaks if a following suffix would overflow the line.
    #[test]
    fn fits_accounts_for_trailing_suffix() {
        // group("aa" line "bb") followed by a long suffix text on the same (flat) line.
        let d = concat(vec![
            group(concat(vec![text("aa"), line(), text("bb")])),
            text(" ;;;;;;"),
        ]);
        // width 10: "aa bb ;;;;;;" = 12 > 10 → the group must break so the suffix lands after "bb".
        assert_eq!(render(&d, 10, 0, 0, false), "aa\nbb ;;;;;;");
    }

    /// `join` inserts the separator between parts only.
    #[test]
    fn join_separates_without_trailing() {
        let d = join(vec![text("a"), text("b"), text("c")], text(", "));
        assert_eq!(render(&d, 80, 0, 0, false), "a, b, c");
    }
}
