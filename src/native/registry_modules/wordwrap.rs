//! `String.wordWrap` — PHP `wordwrap`'s algorithm, counted in CODE POINTS (developer-ruled
//! 2026-09-04).
//!
//! **Why not bytes.** PHP's `wordwrap` is byte-oriented, and with `cut = true` it splits a
//! multi-byte character outright: `wordwrap("ééééééé", 3, "|", true)` yields the bytes
//! `c3 a9 c3 7c a9 …`, which is not valid UTF-8 [Verified 2026-09-04 under php-8.5.9 —
//! `preg_match('//u', …)` rejects it]. A phorj `string` is UTF-8 by construction and cannot hold
//! that, so a byte-faithful port is not merely undesirable here, it is unrepresentable.
//!
//! **Why this needs no byte-identity disclosure.** The PHP leg emits `__phorj_wordwrap`, which runs
//! the SAME codepoint algorithm, rather than calling PHP's native `wordwrap`. So all three legs agree
//! exactly; what differs is `String.wordWrap` versus PHP's `wordwrap` on multi-byte input, which is a
//! deliberate semantic choice, not a break in the spine. For ASCII — overwhelmingly what wrapping is
//! used on — the two are identical anyway.
//!
//! The control flow is PHP's own (ext/standard/string.c): greedy, breaking at the last space before
//! the width is exceeded, and cutting mid-word only when `cut` is set.

use crate::native::*;
use crate::types::Ty;
use crate::value::Value;

/// PHP's `wordwrap` control flow over a code-point slice.
pub(crate) fn word_wrap(text: &str, width: i64, brk: &str, cut: bool) -> String {
    let cs: Vec<char> = text.chars().collect();
    let bs: Vec<char> = brk.chars().collect();
    // A zero-or-negative width would make "current - laststart >= width" true at every position and
    // emit a break between every character; PHP raises a ValueError for width 0 with cut. Clamp to 1,
    // which is the smallest width that still makes progress, and matches PHP's own behaviour at 1.
    let width = width.max(1) as usize;
    if cs.is_empty() || bs.is_empty() {
        return text.to_string();
    }
    let (mut out, mut laststart, mut lastspace) =
        (String::with_capacity(text.len()), 0usize, 0usize);
    let mut current = 0usize;
    while current < cs.len() {
        // An existing break in the input resets the line, exactly as PHP does.
        if cs[current..].starts_with(&bs[..]) {
            out.extend(&cs[laststart..current + bs.len()]);
            current += bs.len();
            laststart = current;
            lastspace = current;
            continue;
        }
        if cs[current] == ' ' {
            if current - laststart >= width {
                out.extend(&cs[laststart..current]);
                out.push_str(brk);
                laststart = current + 1;
            }
            lastspace = current;
        } else if current - laststart >= width && laststart >= lastspace {
            // A single word longer than the width: only `cut` may break it.
            if cut {
                out.extend(&cs[laststart..current]);
                out.push_str(brk);
                laststart = current;
                lastspace = current;
            }
        } else if current - laststart >= width && laststart < lastspace {
            out.extend(&cs[laststart..lastspace]);
            out.push_str(brk);
            laststart = lastspace + 1;
            lastspace = laststart;
        }
        current += 1;
    }
    if laststart != current {
        out.extend(&cs[laststart..]);
    }
    out
}

fn text_word_wrap(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(s), Value::Int(w), Value::Str(b), Value::Bool(c)] => {
            Ok(Value::Str(word_wrap(s, *w, b, *c).into()))
        }
        _ => Err("String.wordWrap expects (string, int, string, bool)".into()),
    }
}

pub(super) fn wordwrap_natives() -> Vec<NativeFn> {
    vec![NativeFn {
        module: "Core.String",
        name: "wordWrap",
        params: vec![Ty::String, Ty::Int, Ty::String, Ty::Bool],
        ret: Ty::String,
        pure: true,
        eval: NativeEval::Pure(text_word_wrap),
        // NOT lifted from PHP's `wordwrap`: the semantics deliberately differ on multi-byte input,
        // so lifting one onto the other would silently change a lifted program's behaviour.
        lift_from: &[],
        php: |a| {
            format!(
                "__phorj_wordwrap({}, {}, {}, {})",
                parg(a, 0),
                parg(a, 1),
                parg(a, 2),
                parg(a, 3)
            )
        },
    }]
}

#[cfg(test)]
mod tests {
    use super::word_wrap;

    /// Pinned to real `php -n` output (php-8.5.9), captured before the port was written. These are
    /// the ASCII cases, where phorj's codepoint algorithm and PHP's byte algorithm must agree
    /// EXACTLY — if they ever diverge here, the port is wrong, not merely different.
    #[test]
    fn ascii_matches_php_byte_for_byte() {
        for (text, width, brk, cut, want) in [
            (
                "The quick brown fox sat over the lazy dog",
                15,
                "\n",
                true,
                "The quick brown\nfox sat over\nthe lazy dog",
            ),
            (
                "The quick brown fox sat over the lazy dog",
                15,
                "\n",
                false,
                "The quick brown\nfox sat over\nthe lazy dog",
            ),
            (
                "A very looooooooooooong word",
                8,
                "\n",
                true,
                "A very\nlooooooo\noooooong\nword",
            ),
            (
                "A very looooooooooooong word",
                8,
                "\n",
                false,
                "A very\nlooooooooooooong\nword",
            ),
            ("short", 10, "\n", false, "short"),
            ("", 5, "\n", false, ""),
            ("a b c d e", 1, "\n", false, "a\nb\nc\nd\ne"),
        ] {
            assert_eq!(
                word_wrap(text, width, brk, cut),
                want,
                "wordwrap({text:?}, {width}, {brk:?}, {cut})"
            );
        }
    }

    /// The whole reason this is a port rather than a call: PHP's `wordwrap` splits a multi-byte
    /// character with `cut = true` and emits invalid UTF-8. Ours never can, because it cuts between
    /// code points — which is exactly the ruled behaviour.
    #[test]
    fn a_multibyte_cut_never_produces_invalid_utf8() {
        // PHP gives bytes c3 a9 c3 7c a9 … here; we give whole characters.
        assert_eq!(word_wrap("ééééééé", 3, "|", true), "ééé|ééé|é");
        // Width counts CHARACTERS, so a 3-char width holds three é regardless of their byte length.
        assert_eq!(word_wrap("日本語です", 2, "|", true), "日本|語で|す");
        // The real property, and the one PHP's byte version breaks: wrapping only INSERTS breaks,
        // it never alters content. Removing the breaks must give the input back exactly. (Stated
        // over space-free input, because a break AT a space consumes that space — PHP does the same.)
        for (text, w) in [("ééééééé", 3), ("日本語です", 2), ("ábcdéfghí", 4)] {
            let wrapped = word_wrap(text, w, "|", true);
            assert_eq!(
                wrapped.replace('|', ""),
                text,
                "wrapping must only insert breaks, but {text:?} became {wrapped:?}"
            );
        }
    }

    /// Degenerate inputs must terminate and must not emit a break between every character. A width
    /// of 0 would make `current - laststart >= width` true at position 0 forever.
    #[test]
    fn degenerate_widths_and_breaks_are_total() {
        assert_eq!(
            word_wrap("abc", 0, "\n", true),
            word_wrap("abc", 1, "\n", true)
        );
        assert_eq!(
            word_wrap("abc", -5, "\n", true),
            word_wrap("abc", 1, "\n", true)
        );
        // An empty break string cannot separate anything; return the input rather than looping.
        assert_eq!(word_wrap("a b c", 1, "", false), "a b c");
        assert_eq!(word_wrap("", 1, "\n", true), "");
    }

    /// A break already present in the input resets the line, as PHP does — otherwise pre-wrapped
    /// text would be re-wrapped against a running count that never restarts.
    #[test]
    fn an_existing_break_resets_the_line() {
        assert_eq!(word_wrap("aaa\nbbb ccc", 3, "\n", false), "aaa\nbbb\nccc");
    }
}
