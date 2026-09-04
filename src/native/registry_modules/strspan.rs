//! Four FN-STR parity rows: `substringFromAny`, `countLeadingIn`, `increment`, `decrement`.
//!
//! Names ruled by the developer 2026-09-04 (descriptive house style over transliterated C
//! abbreviations): PHP's `strpbrk`, `strspn`, `str_increment`, `str_decrement` respectively. All four
//! are byte-oriented in PHP; the first two are set membership over ASCII and the last two are
//! restricted to `[a-zA-Z0-9]`, so none has the UTF-8 hazard that forced `wordWrap` onto code points.
//!
//! Every behaviour below is pinned to `php -n` output captured BEFORE the port was written — the
//! alphanumeric carry rules especially, which are not obvious: `str_increment("9")` is `"10"` and
//! `str_decrement("a0")` is `"9"`, because a borrow out of the leading position DROPS that position.

use crate::native::*;
use crate::types::Ty;
use crate::value::Value;

/// PHP `strpbrk`: the substring starting at the first byte that appears in `chars`. `None` where PHP
/// returns `false` — the module's optional convention (developer-ruled), not a fault.
pub(crate) fn substring_from_any(s: &str, chars: &str) -> Option<String> {
    let set = chars.as_bytes();
    s.as_bytes()
        .iter()
        .position(|b| set.contains(b))
        .map(|i| String::from_utf8_lossy(&s.as_bytes()[i..]).into_owned())
}

/// PHP `strspn`: how many leading bytes are drawn only from `chars`.
pub(crate) fn count_leading_in(s: &str, chars: &str) -> i64 {
    let set = chars.as_bytes();
    s.as_bytes().iter().take_while(|b| set.contains(b)).count() as i64
}

fn alnum_or_err(s: &str, who: &str) -> Result<Vec<u8>, String> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return Err(format!(
            "String.{who} expects a non-empty alphanumeric string, got {s:?}"
        ));
    }
    Ok(s.as_bytes().to_vec())
}

/// PHP `str_increment`. Carries right-to-left; a carry out of the leading position PREPENDS a
/// character chosen by that position's class — `z` prepends `a`, `Z` prepends `A`, `9` prepends `1`
/// (so `"9"` becomes `"10"`, not `"00"`).
pub(crate) fn increment(s: &str) -> Result<String, String> {
    let mut b = alnum_or_err(s, "increment")?;
    let first_was = b[0];
    let mut i = b.len();
    let mut carry = true;
    while carry && i > 0 {
        i -= 1;
        match b[i] {
            b'z' => b[i] = b'a',
            b'Z' => b[i] = b'A',
            b'9' => b[i] = b'0',
            c => {
                b[i] = c + 1;
                carry = false;
            }
        }
    }
    let mut out = String::from_utf8(b).map_err(|e| e.to_string())?;
    if carry {
        out.insert(
            0,
            match first_was {
                b'z' => 'a',
                b'Z' => 'A',
                _ => '1',
            },
        );
    }
    Ok(out)
}

/// PHP `str_decrement`. The inverse, with the rule that is easy to miss: a borrow out of the leading
/// position DROPS that position (`"aa"` → `"z"`, `"a0"` → `"9"`), and so does a leading `'0'` left
/// behind by an ordinary decrement (`"100"` → `"099"` → `"99"`). Underflowing to nothing is an
/// error, as PHP's ValueError is — `"a"`, `"A"` and `"0"` have no predecessor.
pub(crate) fn decrement(s: &str) -> Result<String, String> {
    let mut b = alnum_or_err(s, "decrement")?;
    let mut i = b.len();
    let mut borrow = true;
    while borrow && i > 0 {
        i -= 1;
        match b[i] {
            b'a' => b[i] = b'z',
            b'A' => b[i] = b'Z',
            b'0' => b[i] = b'9',
            c => {
                b[i] = c - 1;
                borrow = false;
            }
        }
    }
    if borrow || (b[0] == b'0' && b.len() > 1) {
        b.remove(0);
    }
    if b.is_empty() {
        return Err(format!(
            "String.decrement: {s:?} has no predecessor (it is the smallest value of its length)"
        ));
    }
    String::from_utf8(b).map_err(|e| e.to_string())
}

fn n_substring_from_any(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(s), Value::Str(c)] => Ok(match substring_from_any(s, c) {
            Some(v) => Value::Str(v.into()),
            None => Value::Null,
        }),
        _ => Err("String.substringFromAny expects (string, string)".into()),
    }
}
fn n_count_leading_in(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(s), Value::Str(c)] => Ok(Value::Int(count_leading_in(s, c))),
        _ => Err("String.countLeadingIn expects (string, string)".into()),
    }
}
fn n_increment(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(s)] => Ok(Value::Str(increment(s)?.into())),
        _ => Err("String.increment expects (string)".into()),
    }
}
fn n_decrement(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(s)] => Ok(Value::Str(decrement(s)?.into())),
        _ => Err("String.decrement expects (string)".into()),
    }
}

pub(super) fn strspan_natives() -> Vec<NativeFn> {
    vec![
        NativeFn {
            module: "Core.String",
            name: "substringFromAny",
            params: vec![Ty::String, Ty::String],
            ret: Ty::Optional(Box::new(Ty::String)),
            pure: true,
            eval: NativeEval::Pure(n_substring_from_any),
            // `strpbrk` returns `false`, not null, so the emission maps it — the same false->null
            // bridge `base64Decode` and `hexDecode` next door already use.
            lift_from: &[],
            php: |a| {
                format!(
                    "(($__pb = strpbrk({}, {})) === false ? null : $__pb)",
                    parg(a, 0),
                    parg(a, 1)
                )
            },
        },
        NativeFn {
            module: "Core.String",
            name: "countLeadingIn",
            params: vec![Ty::String, Ty::String],
            ret: Ty::Int,
            pure: true,
            eval: NativeEval::Pure(n_count_leading_in),
            lift_from: &["strspn"],
            php: |a| format!("strspn({}, {})", parg(a, 0), parg(a, 1)),
        },
        NativeFn {
            module: "Core.String",
            name: "increment",
            params: vec![Ty::String],
            ret: Ty::String,
            pure: true,
            eval: NativeEval::Pure(n_increment),
            lift_from: &["str_increment"],
            php: |a| format!("str_increment({})", parg(a, 0)),
        },
        NativeFn {
            module: "Core.String",
            name: "decrement",
            params: vec![Ty::String],
            ret: Ty::String,
            pure: true,
            eval: NativeEval::Pure(n_decrement),
            lift_from: &["str_decrement"],
            php: |a| format!("str_decrement({})", parg(a, 0)),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pinned to `php -n` output captured BEFORE the port existed, so this compares against PHP
    /// rather than against my reading of the carry rules.
    #[test]
    fn increment_matches_php() {
        for (i, w) in [
            ("a", "b"),
            ("z", "aa"),
            ("Z", "AA"),
            ("9", "10"),
            ("az", "ba"),
            ("zz", "aaa"),
            ("Az", "Ba"),
            ("a9", "b0"),
            ("Zz", "AAa"),
            ("A9", "B0"),
            ("zz9", "aaa0"),
        ] {
            assert_eq!(increment(i).unwrap(), w, "increment({i:?})");
        }
    }

    /// The rule that is easy to get wrong and impossible to guess: a borrow out of the leading
    /// position DROPS it (`"aa"` → `"z"`), and so does a leading `'0'` an ordinary decrement leaves
    /// behind (`"100"` → `"099"` → `"99"`). Without both, every one of these is wrong.
    #[test]
    fn decrement_matches_php_including_the_leading_drop() {
        for (i, w) in [
            ("b", "a"),
            ("aa", "z"),
            ("1", "0"),
            ("10", "9"),
            ("Ba", "Az"),
            ("b0", "a9"),
            ("ba", "az"),
            ("a0", "9"),
            ("100", "99"),
            ("aaa", "zz"),
            ("Aa", "z"),
            ("1a", "z"),
            ("a1", "a0"),
            ("z0", "y9"),
            ("10a", "9z"),
            ("zz", "zy"),
        ] {
            assert_eq!(decrement(i).unwrap(), w, "decrement({i:?})");
        }
    }

    /// PHP raises ValueError for these; phorj faults. Empty and non-alphanumeric are rejected by
    /// both functions, and the three smallest single characters have no predecessor.
    #[test]
    fn invalid_input_is_an_error_on_both_functions() {
        for bad in ["", "a b", "é", "a-b"] {
            assert!(increment(bad).is_err(), "increment({bad:?}) must fail");
            assert!(decrement(bad).is_err(), "decrement({bad:?}) must fail");
        }
        for smallest in ["a", "A", "0"] {
            assert!(
                decrement(smallest).is_err(),
                "decrement({smallest:?}) must fail — nothing precedes it"
            );
            // …but incrementing them is fine, which is what makes this asymmetric.
            assert!(increment(smallest).is_ok());
        }
    }

    /// Round-trip: incrementing then decrementing returns the input, wherever a predecessor exists.
    /// This is the property the carry and borrow rules must satisfy jointly — a fixture pair can
    /// agree with PHP while the two directions disagree with each other.
    #[test]
    fn decrement_undoes_increment() {
        for w in ["a", "z", "az", "zz", "a9", "Zz", "zz9", "A9", "9", "Z"] {
            let up = increment(w).unwrap();
            assert_eq!(decrement(&up).unwrap(), w, "decrement(increment({w:?}))");
        }
    }

    #[test]
    fn set_membership_matches_php() {
        assert_eq!(
            substring_from_any("This is a test", "st").as_deref(),
            Some("s is a test")
        );
        assert_eq!(substring_from_any("abc", "xyz"), None);
        assert_eq!(substring_from_any("", "a"), None);
        assert_eq!(count_leading_in("42 apples", "1234567890"), 2);
        assert_eq!(count_leading_in("abc", "xyz"), 0);
        assert_eq!(count_leading_in("aaabbb", "a"), 3);
        assert_eq!(count_leading_in("", "x"), 0);
        // An empty set matches nothing, and must not loop or panic.
        assert_eq!(count_leading_in("abc", ""), 0);
        assert_eq!(substring_from_any("abc", ""), None);
    }
}
