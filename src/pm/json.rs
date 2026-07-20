//! A tiny std-only JSON reader/writer for the package manager's `phorj.json` + `phorj.lock` (DEC-316).
//!
//! The language's own JSON (`ext::json`) operates on runtime `Value`s, not compiler-side Rust data, so
//! the PM carries its own minimal parser — hand-rolled like `bundle::sha256` (the external-dependency
//! policy forbids `serde_json`). Objects preserve insertion order (a `Vec` of pairs) so serialized
//! manifests/locks are deterministic (Invariant 10). Scope = exactly what a manifest needs: objects,
//! arrays, strings (with the standard escapes), integers/decimals, booleans, null.

/// A parsed JSON value. Numbers are kept as their source text (the PM only ever reads version STRINGS,
/// never arithmetic) so no float round-tripping is introduced.
#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Num(String),
    Str(String),
    Arr(Vec<Json>),
    /// Insertion-ordered key→value pairs (duplicate keys keep the last, like most JSON readers).
    Obj(Vec<(String, Json)>),
}

impl Json {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }
    pub fn as_obj(&self) -> Option<&[(String, Json)]> {
        match self {
            Json::Obj(v) => Some(v),
            _ => None,
        }
    }
    pub fn as_arr(&self) -> Option<&[Json]> {
        match self {
            Json::Arr(v) => Some(v),
            _ => None,
        }
    }
    /// Object member lookup (last-wins on duplicate keys).
    pub fn get(&self, key: &str) -> Option<&Json> {
        self.as_obj()?
            .iter()
            .rev()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v)
    }

    /// Parse a whole document; errors if trailing non-whitespace remains.
    pub fn parse(src: &str) -> Result<Json, String> {
        let mut p = Parser {
            b: src.as_bytes(),
            i: 0,
        };
        p.ws();
        let v = p.value()?;
        p.ws();
        if p.i != p.b.len() {
            return Err(format!(
                "trailing characters after JSON value at byte {}",
                p.i
            ));
        }
        Ok(v)
    }

    /// Serialize as pretty JSON (2-space indent, `\n` newlines) — the on-disk manifest/lock form.
    pub fn to_pretty(&self) -> String {
        let mut out = String::new();
        self.write(&mut out, 0);
        out.push('\n');
        out
    }

    fn write(&self, out: &mut String, depth: usize) {
        match self {
            Json::Null => out.push_str("null"),
            Json::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Json::Num(n) => out.push_str(n),
            Json::Str(s) => write_str(out, s),
            Json::Arr(items) => {
                if items.is_empty() {
                    out.push_str("[]");
                    return;
                }
                out.push('[');
                for (n, item) in items.iter().enumerate() {
                    if n > 0 {
                        out.push(',');
                    }
                    newline_indent(out, depth + 1);
                    item.write(out, depth + 1);
                }
                newline_indent(out, depth);
                out.push(']');
            }
            Json::Obj(pairs) => {
                if pairs.is_empty() {
                    out.push_str("{}");
                    return;
                }
                out.push('{');
                for (n, (k, v)) in pairs.iter().enumerate() {
                    if n > 0 {
                        out.push(',');
                    }
                    newline_indent(out, depth + 1);
                    write_str(out, k);
                    out.push_str(": ");
                    v.write(out, depth + 1);
                }
                newline_indent(out, depth);
                out.push('}');
            }
        }
    }
}

fn newline_indent(out: &mut String, depth: usize) {
    out.push('\n');
    for _ in 0..depth {
        out.push_str("  ");
    }
}

fn write_str(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

struct Parser<'a> {
    b: &'a [u8],
    i: usize,
}

impl Parser<'_> {
    fn ws(&mut self) {
        while self.i < self.b.len() && matches!(self.b[self.i], b' ' | b'\t' | b'\n' | b'\r') {
            self.i += 1;
        }
    }

    fn value(&mut self) -> Result<Json, String> {
        self.ws();
        match self.b.get(self.i) {
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'"') => Ok(Json::Str(self.string()?)),
            Some(b't') => self.lit("true", Json::Bool(true)),
            Some(b'f') => self.lit("false", Json::Bool(false)),
            Some(b'n') => self.lit("null", Json::Null),
            Some(c) if *c == b'-' || c.is_ascii_digit() => self.number(),
            Some(c) => Err(format!("unexpected byte `{}` at {}", *c as char, self.i)),
            None => Err("unexpected end of JSON".to_string()),
        }
    }

    fn lit(&mut self, word: &str, val: Json) -> Result<Json, String> {
        if self.b[self.i..].starts_with(word.as_bytes()) {
            self.i += word.len();
            Ok(val)
        } else {
            Err(format!("invalid literal at byte {}", self.i))
        }
    }

    fn number(&mut self) -> Result<Json, String> {
        let start = self.i;
        if self.b.get(self.i) == Some(&b'-') {
            self.i += 1;
        }
        while self.i < self.b.len()
            && matches!(
                self.b[self.i],
                b'0'..=b'9' | b'.' | b'e' | b'E' | b'+' | b'-'
            )
        {
            self.i += 1;
        }
        let s =
            std::str::from_utf8(&self.b[start..self.i]).map_err(|_| "bad number".to_string())?;
        Ok(Json::Num(s.to_string()))
    }

    fn string(&mut self) -> Result<String, String> {
        self.i += 1; // opening quote
        let mut s = String::new();
        while self.i < self.b.len() {
            let c = self.b[self.i];
            self.i += 1;
            match c {
                b'"' => return Ok(s),
                b'\\' => {
                    let e = *self.b.get(self.i).ok_or("unterminated escape")?;
                    self.i += 1;
                    match e {
                        b'"' => s.push('"'),
                        b'\\' => s.push('\\'),
                        b'/' => s.push('/'),
                        b'n' => s.push('\n'),
                        b't' => s.push('\t'),
                        b'r' => s.push('\r'),
                        b'b' => s.push('\u{08}'),
                        b'f' => s.push('\u{0c}'),
                        b'u' => {
                            let hex = self
                                .b
                                .get(self.i..self.i + 4)
                                .ok_or("truncated \\u escape")?;
                            let cp = u32::from_str_radix(
                                std::str::from_utf8(hex).map_err(|_| "bad \\u hex")?,
                                16,
                            )
                            .map_err(|_| "bad \\u hex")?;
                            self.i += 4;
                            s.push(char::from_u32(cp).unwrap_or('\u{fffd}'));
                        }
                        other => return Err(format!("invalid escape `\\{}`", other as char)),
                    }
                }
                _ => {
                    // Copy the raw UTF-8 byte(s): back up and take the whole char.
                    let rest = &self.b[self.i - 1..];
                    let ch_len = utf8_len(rest[0]);
                    let chunk =
                        std::str::from_utf8(&rest[..ch_len]).map_err(|_| "bad utf-8 in string")?;
                    s.push_str(chunk);
                    self.i += ch_len - 1;
                }
            }
        }
        Err("unterminated string".to_string())
    }

    fn array(&mut self) -> Result<Json, String> {
        self.i += 1; // [
        let mut items = Vec::new();
        self.ws();
        if self.b.get(self.i) == Some(&b']') {
            self.i += 1;
            return Ok(Json::Arr(items));
        }
        loop {
            items.push(self.value()?);
            self.ws();
            match self.b.get(self.i) {
                Some(b',') => {
                    self.i += 1;
                }
                Some(b']') => {
                    self.i += 1;
                    return Ok(Json::Arr(items));
                }
                _ => return Err(format!("expected `,` or `]` in array at byte {}", self.i)),
            }
        }
    }

    fn object(&mut self) -> Result<Json, String> {
        self.i += 1; // {
        let mut pairs = Vec::new();
        self.ws();
        if self.b.get(self.i) == Some(&b'}') {
            self.i += 1;
            return Ok(Json::Obj(pairs));
        }
        loop {
            self.ws();
            if self.b.get(self.i) != Some(&b'"') {
                return Err(format!("expected string key in object at byte {}", self.i));
            }
            let key = self.string()?;
            self.ws();
            if self.b.get(self.i) != Some(&b':') {
                return Err(format!("expected `:` after key `{key}` at byte {}", self.i));
            }
            self.i += 1;
            let val = self.value()?;
            pairs.push((key, val));
            self.ws();
            match self.b.get(self.i) {
                Some(b',') => {
                    self.i += 1;
                }
                Some(b'}') => {
                    self.i += 1;
                    return Ok(Json::Obj(pairs));
                }
                _ => return Err(format!("expected `,` or `}}` in object at byte {}", self.i)),
            }
        }
    }
}

fn utf8_len(first: u8) -> usize {
    match first {
        0x00..=0x7f => 1,
        0xc0..=0xdf => 2,
        0xe0..=0xef => 3,
        _ => 4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_object_and_roundtrips() {
        let src =
            r#"{ "name": "Acme/Util", "version": "1.2.3", "require": { "Acme/Json": "^1.0" } }"#;
        let j = Json::parse(src).unwrap();
        assert_eq!(j.get("name").unwrap().as_str(), Some("Acme/Util"));
        assert_eq!(
            j.get("require").unwrap().get("Acme/Json").unwrap().as_str(),
            Some("^1.0")
        );
        // Re-parse of the pretty form is structurally identical (deterministic order).
        let pretty = j.to_pretty();
        assert_eq!(Json::parse(&pretty).unwrap(), j);
    }

    #[test]
    fn handles_arrays_escapes_and_literals() {
        let j = Json::parse(r#"{"a":[1,2,3],"b":true,"c":null,"s":"a\"b\nA"}"#).unwrap();
        assert_eq!(j.get("a").unwrap().as_arr().unwrap().len(), 3);
        assert_eq!(j.get("b").unwrap(), &Json::Bool(true));
        assert_eq!(j.get("c").unwrap(), &Json::Null);
        assert_eq!(j.get("s").unwrap().as_str(), Some("a\"b\nA"));
    }

    #[test]
    fn empty_containers_and_nesting() {
        let j = Json::parse(r#"{"e":{},"a":[],"n":{"x":[{"y":1}]}}"#).unwrap();
        assert_eq!(j.get("e").unwrap(), &Json::Obj(vec![]));
        assert_eq!(j.get("a").unwrap(), &Json::Arr(vec![]));
        assert!(j.get("n").unwrap().get("x").is_some());
    }

    #[test]
    fn rejects_trailing_and_truncated() {
        assert!(Json::parse(r#"{"a":1} junk"#).is_err());
        assert!(Json::parse(r#"{"a":"#).is_err());
        assert!(Json::parse(r#"{a:1}"#).is_err()); // unquoted key
    }

    #[test]
    fn pretty_is_stable() {
        let j = Json::parse(r#"{"b":1,"a":2}"#).unwrap();
        // insertion order preserved (b before a), not sorted
        assert_eq!(j.to_pretty(), "{\n  \"b\": 1,\n  \"a\": 2\n}\n");
    }
}
