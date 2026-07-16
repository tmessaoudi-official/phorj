//! `Core.Uri` kernel tests — every expectation below is PINNED to live php-8.5.8
//! `Uri\Rfc3986\Uri` output (the transpile twin), captured in
//! `docs/research/2026-07-16-uri-twin-probes.md`. A failure here means the kernel drifted from
//! the twin, never that the twin is "wrong".

use super::kernel::*;

fn p(s: &str) -> Parts {
    parse(s).unwrap_or_else(|e| panic!("must parse: {s} ({e:?})"))
}

fn norm_path(s: &str) -> String {
    normalize(&p(s)).path
}

fn tos(s: &str) -> String {
    to_string_normalized(&p(s))
}

#[test]
fn parses_full_uri_into_raw_components() {
    let u = p("https://user:pw@Example.COM:8080/a/../b/./c?x=1&y=2#frag");
    assert_eq!(u.scheme.as_deref(), Some("https"));
    assert_eq!(u.userinfo.as_deref(), Some("user:pw"));
    assert_eq!(u.host.as_deref(), Some("Example.COM"));
    assert_eq!(u.port.as_deref(), Some("8080"));
    assert_eq!(u.path, "/a/../b/./c");
    assert_eq!(u.query.as_deref(), Some("x=1&y=2"));
    assert_eq!(u.fragment.as_deref(), Some("frag"));
    // Raw recomposition round-trips byte-for-byte (toRawString).
    assert_eq!(
        recompose(&u),
        "https://user:pw@Example.COM:8080/a/../b/./c?x=1&y=2#frag"
    );
}

#[test]
fn normalized_getters_match_the_twin() {
    let n = normalize(&p(
        "https://user:pw@Example.COM:8080/a/../b/./c?x=1&y=2#frag",
    ));
    assert_eq!(n.scheme.as_deref(), Some("https"));
    assert_eq!(n.host.as_deref(), Some("example.com"));
    assert_eq!(n.path, "/b/c");
    assert_eq!(
        tos("https://user:pw@Example.COM:8080/a/../b/./c?x=1&y=2#frag"),
        "https://user:pw@example.com:8080/b/c?x=1&y=2#frag"
    );
    // Default ports are NOT elided.
    assert_eq!(
        tos("HTTPS://EXAMPLE.com:443/a/../b?q#f"),
        "https://example.com:443/b?q#f"
    );
}

#[test]
fn relative_references_and_missing_components() {
    let u = p("/path/only?q");
    assert!(u.scheme.is_none() && u.host.is_none());
    assert_eq!(tos("/path/only?q"), "/path/only?q");
    assert_eq!(tos(""), "");
    // `//h` is an authority, not a path.
    let u = p("//h");
    assert_eq!(u.host.as_deref(), Some("h"));
    assert_eq!(u.path, "");
    assert_eq!(tos("//h"), "//h");
    // `http://h` has an EMPTY path (not `/`); `http:///p` an empty host (present ≠ absent).
    assert_eq!(p("http://h").path, "");
    assert_eq!(p("http:///p").host.as_deref(), Some(""));
    assert_eq!(tos("http:///p"), "http:///p");
}

#[test]
fn rejects_what_the_twin_rejects() {
    for bad in [
        "http://exa mple.com/", // raw space
        ":no-scheme",           // empty scheme
        "1x:rest",              // scheme starting with a digit
        "http://h/%zz",         // invalid pct escape
        "http://h/\u{e9}",      // raw non-ASCII
        "http://[2001:zz8::1]/",
        "http://[::1/",  // unclosed IP-literal
        "http://h:80x/", // non-digit port
        "a:b/c",         // hmm — see valid-scheme test below; this one is FINE
    ] {
        if bad == "a:b/c" {
            assert!(parse(bad).is_ok(), "{bad}");
        } else {
            assert_eq!(parse(bad), Err(UriErr::Malformed), "{bad}");
        }
    }
    // Port range: exactly i64 (probed: i64::MAX parses, +1 is out of range, 20 digits too).
    assert!(parse("http://h:9223372036854775807/").is_ok());
    assert_eq!(
        parse("http://h:9223372036854775808/"),
        Err(UriErr::PortRange)
    );
    assert_eq!(
        parse("http://h:99999999999999999999/"),
        Err(UriErr::PortRange)
    );
}

#[test]
fn percent_normalization_is_ascii_unreserved_only() {
    // %7E→~ and %41→A decode; %2f stays and uppercases; non-ASCII octets stay encoded.
    assert_eq!(
        tos("http://h/%7Euser/%41%2f?x=%7e#%7E"),
        "http://h/~user/A%2F?x=~#~"
    );
    assert_eq!(normalize(&p("http://h/%7Euser/%41%2f")).path, "/~user/A%2F");
    assert_eq!(normalize(&p("http://h/%c3%a9")).path, "/%C3%A9");
    // Host: decoded + lowercased (%41→A→a); reserved escapes uppercase.
    assert_eq!(
        normalize(&p("http://EX%41MPLE.com/")).host.as_deref(),
        Some("example.com")
    );
    assert_eq!(
        normalize(&p("http://ex%2fmple/")).host.as_deref(),
        Some("ex%2Fmple")
    );
}

#[test]
fn dot_segment_removal_matches_the_twin_corpus() {
    // Scheme-less relative paths (keep unmatched leading `..`):
    for (input, expected) in [
        ("a/..", ""),
        ("a/../", ""),
        ("..", ".."),
        ("../", "../"),
        (".", ""),
        ("./", "./"),
        ("a/../..", ".."),
        ("a/../../", "../"),
        ("../g/./h", "../g/h"),
        ("/a/../../b", "/b"),
        ("/..", "/"),
        ("/.", "/"),
        ("a/./b", "a/b"),
        ("./a", "a"),
        ("../..", "../.."),
        ("..//g", "..//g"),
        ("a//b/../c", "a//c"),
    ] {
        assert_eq!(norm_path(input), expected, "path {input:?}");
    }
    // Scheme-ful rootless paths (unmatched `..` drops):
    for (input, expected) in [
        ("s:a/..", ""),
        ("s:..", ""),
        ("s:../b", "b"),
        ("s:a/../..", ""),
        ("s:./", "./"),
    ] {
        assert_eq!(norm_path(input), expected, "path of {input:?}");
    }
    // Rooted paths behind an authority:
    for (input, expected) in [
        ("http://h/a/..", "/"),
        ("http://h/a/../", "/"),
        ("http://h/..", "/"),
        ("http://h/../a", "/a"),
        ("http://h/a/./b/", "/a/b/"),
        ("http://h//x//y", "//x//y"),
    ] {
        assert_eq!(norm_path(input), expected, "path of {input:?}");
    }
}

#[test]
fn ports_normalize_and_round_trip() {
    // Empty port round-trips (getPort null is the native layer's mapping).
    assert_eq!(tos("http://h:/p"), "http://h:/p");
    assert_eq!(norm_port(""), "");
    // Leading zeros strip; a lone 0 is kept.
    assert_eq!(tos("http://h:0080/"), "http://h:80/");
    assert_eq!(norm_port("0"), "0");
}

#[test]
fn ipv6_hosts_lowercase_as_written_but_expand_in_tostring() {
    // getHost: lowercased AS WRITTEN — no re-compression.
    assert_eq!(norm_host_getter("[2001:DB8::1]"), "[2001:db8::1]");
    assert_eq!(
        norm_host_getter("[2001:db8:1:2:3:4:5:6]"),
        "[2001:db8:1:2:3:4:5:6]"
    );
    assert_eq!(norm_host_getter("[::FFFF:1.2.3.4]"), "[::ffff:1.2.3.4]");
    assert_eq!(norm_host_getter("[V1.ABC]"), "[v1.abc]");
    // toString: expanded to eight 4-digit hextets; IPv4 tails become pure hex.
    assert_eq!(
        tos("http://[2001:DB8::1]:80/p"),
        "http://[2001:0db8:0000:0000:0000:0000:0000:0001]:80/p"
    );
    assert_eq!(
        tos("http://[::ffff:192.168.1.1]/"),
        "http://[0000:0000:0000:0000:0000:ffff:c0a8:0101]/"
    );
    assert_eq!(
        tos("http://[::1]/"),
        "http://[0000:0000:0000:0000:0000:0000:0000:0001]/"
    );
    // IPvFuture: lowercased, never expanded.
    assert_eq!(tos("http://[V1.ABC]/"), "http://[v1.abc]/");
}

#[test]
fn resolve_matches_the_rfc_and_twin_corpus() {
    let base = p("http://a/b/c/d;p?q");
    let res = |r: &str| to_string_normalized(&resolve(&base, &p(r)));
    for (r, expected) in [
        ("g:h", "g:h"),
        ("g", "http://a/b/c/g"),
        ("./g", "http://a/b/c/g"),
        ("g/", "http://a/b/c/g/"),
        ("/g", "http://a/g"),
        ("//g", "http://g"),
        ("?y", "http://a/b/c/d;p?y"),
        ("g?y", "http://a/b/c/g?y"),
        ("#s", "http://a/b/c/d;p?q#s"),
        ("g#s", "http://a/b/c/g#s"),
        ("g?y#s", "http://a/b/c/g?y#s"),
        (";x", "http://a/b/c/;x"),
        ("g;x", "http://a/b/c/g;x"),
        ("g;x?y#s", "http://a/b/c/g;x?y#s"),
        ("", "http://a/b/c/d;p?q"),
        (".", "http://a/b/c/"),
        ("./", "http://a/b/c/"),
        ("..", "http://a/b/"),
        ("../", "http://a/b/"),
        ("../g", "http://a/b/g"),
        ("../..", "http://a/"),
        ("../../", "http://a/"),
        ("../../g", "http://a/g"),
        ("../../../g", "http://a/g"),
        ("../../../../g", "http://a/g"),
        ("/./g", "http://a/g"),
        ("/../g", "http://a/g"),
        ("g.", "http://a/b/c/g."),
        (".g", "http://a/b/c/.g"),
        ("g..", "http://a/b/c/g.."),
        ("..g", "http://a/b/c/..g"),
        ("./../g", "http://a/b/g"),
        ("./g/.", "http://a/b/c/g/"),
        ("g/./h", "http://a/b/c/g/h"),
        ("g/../h", "http://a/b/c/h"),
        ("g;x=1/./y", "http://a/b/c/g;x=1/y"),
        ("g;x=1/../y", "http://a/b/c/y"),
        ("g?y/./x", "http://a/b/c/g?y/./x"),
        ("g?y/../x", "http://a/b/c/g?y/../x"),
        ("g#s/./x", "http://a/b/c/g#s/./x"),
        ("g#s/../x", "http://a/b/c/g#s/../x"),
        ("http:g", "http:g"),
    ] {
        assert_eq!(res(r), expected, "resolve({r:?})");
    }
    // A base fragment is never inherited.
    let fb = p("http://a/b#bf");
    assert_eq!(to_string_normalized(&resolve(&fb, &p("g"))), "http://a/g");
}

#[test]
fn empty_query_and_fragment_are_distinct_from_absent() {
    let u = p("http://h/p?#");
    assert_eq!(u.query.as_deref(), Some(""));
    assert_eq!(u.fragment.as_deref(), Some(""));
    assert_eq!(tos("http://h/p?#"), "http://h/p?#");
    assert!(p("http://h/p").query.is_none());
}

#[test]
fn userinfo_and_odd_schemes() {
    assert_eq!(
        p("http://a!$&'()*+,;=:x@h/").userinfo.as_deref(),
        Some("a!$&'()*+,;=:x")
    );
    assert_eq!(p("a+b-c.D://h/").scheme.as_deref(), Some("a+b-c.D"));
    assert_eq!(
        normalize(&p("a+b-c.D://h/")).scheme.as_deref(),
        Some("a+b-c.d")
    );
    assert_eq!(p("mailto:a@b.c").path, "a@b.c");
    assert_eq!(p("urn:isbn:0451450523").path, "isbn:0451450523");
    assert_eq!(tos("file:///tmp/x"), "file:///tmp/x");
    // No IPv4 normalization — dotted-decimal is a plain reg-name.
    assert_eq!(
        normalize(&p("http://127.000.000.1/")).host.as_deref(),
        Some("127.000.000.1")
    );
    // Query may hold `?` and `+` verbatim.
    assert_eq!(
        normalize(&p("http://h/?a?b=c")).query.as_deref(),
        Some("a?b=c")
    );
    assert_eq!(
        normalize(&p("http://h/?a=b+c")).query.as_deref(),
        Some("a=b+c")
    );
}

#[test]
fn component_validators_gate_wither_input() {
    // The twin's withers are strict validators (no auto-encoding).
    assert!(valid_scheme("http") && !valid_scheme("9bad") && !valid_scheme(""));
    assert!(valid_userinfo("user:pw") && !valid_userinfo("a b"));
    assert!(valid_host("h") && valid_host("") && !valid_host("ex ample"));
    assert!(valid_host("[::1]") && !valid_host("[::1"));
    assert!(valid_port("0").is_ok() && valid_port("").is_ok());
    assert_eq!(valid_port("-1x"), Err(UriErr::Malformed));
    assert!(valid_path("/a/b", true, true) && !valid_path("a b", false, false));
    // With an authority the path must be empty or absolute.
    assert!(!valid_path("rel", true, true));
    // A scheme-less relative first segment may not contain `:`.
    assert!(!valid_path("a:b/c", false, false));
    assert!(valid_path("a:b/c", false, true));
    assert!(valid_query_or_fragment("a?b/c=1&x") && !valid_query_or_fragment("a b"));
}
