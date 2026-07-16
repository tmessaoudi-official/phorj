# DEC-240 `Core.Uri` — PHP 8.5 `Uri\Rfc3986\Uri` twin contract (probed live on php-8.5.8)

> Raw probe record for the DEC-240 build. Every behavior below was executed against the oracle
> php-8.5.8 binary (`scripts/toolchain.env`), 2026-07-16. The phorj native must match these
> byte-for-byte on the differential legs; anything not probed here gets a golden test before use.

## API surface (get_class_methods)

`parse` (static → `?Uri`, null on failure) · `__construct(string $uri, ?string $baseUrl = null)`
(throws) · per-component `getX` / `getRawX` / `withX` for scheme, userInfo (+ getUsername /
getRawUsername / getPassword / getRawPassword — read-only split, no withers), host, port
(no raw variant), path, query, fragment · `equals(Uri, UriComparisonMode $mode =
ExcludeFragment)` · `toString` (normalized) · `toRawString` (as written) · `resolve(string)`.

## Normalization (getX + toString; getRawX/toRawString keep the written form)

- scheme + host lowercased (`Example.COM` → `example.com`).
- Dot-segments removed from the PATH (`/a/../b/./c` → `/b/c`) — in getters AND toString.
- Percent-encoding: unreserved octets DECODE (`%7E`→`~`, `%41`→`A`, `%48`→`H` even in host);
  reserved octets stay encoded and hex is UPPERCASED (`%2f`→`%2F`). Applies to path, query,
  fragment, host.
- Default ports are NOT elided (`https://example.com:443/` keeps `:443`); `withPort(0)` and
  `withPort(70000)` are ACCEPTED (no 65535 cap); `withPort(-1)` throws; `withPort(null)` clears.
- IPv6 QUIRK: `getHost` compresses+lowercases (`[2001:db8::1]`), but `toString` EXPANDS to full
  uncompressed form (`[2001:0db8:0000:0000:0000:0000:0000:0001]`). Faithful-twin duty: match it.

## Missing components / relative references

- Relative references are fully legal (`/path/only?q`, ``, `//host/p`); missing scheme/host/port
  → getters return NULL; `http://h` has path `""` (empty, not `/`); `http:///p` has host `""`
  (present-but-empty ≠ null).
- `getPassword` on `user@h` (no `:`) → `''` (empty string, not null). `withUserInfo(null)` clears.
- `mailto:a@b.c` path = `a@b.c`; `urn:isbn:x` path = `isbn:x`; `file:///tmp/x` round-trips.

## Errors (the taxonomy anchor)

All are `Uri\InvalidUriException`; the MESSAGE is component-specific — phorj's typed `UriError`
taxonomy maps 1:1 to these messages (byte-identity via message text):

| Trigger | Message |
|---|---|
| ctor / whole-string parse failure | `The specified URI is malformed` |
| `withScheme("9bad")` | `The specified scheme is malformed` |
| `withHost("ex ample")` | `The specified host is malformed` |
| `withPort(-1)` | `The specified port is malformed` |
| `withPath("a b")` | `The specified path is malformed` |
| `withQuery("a=1&b=%")` | `The specified query is malformed` |
| (expect same shape) `withUserInfo` / `withFragment` | probe before use |
| `resolve()` on a RELATIVE base | `The specified base URI must be absolute` |
| `resolve("http://bad host/")` (malformed ref) | `The specified URI is malformed` |

`Uri::parse(bad)` returns NULL (never throws) — the phorj `Uri.parse` twin should surface the
option/typed-error split accordingly.

## equals

Normalized comparison; `UriComparisonMode::ExcludeFragment` is the DEFAULT (`http://h/b#f` equals
`http://h/b` → true); `IncludeFragment` → false. Dot-segment/case/pct normalization applies
(`http://h/a/../b` equals `http://h/b`).

## resolve (RFC 3986 §5.2.4 — all verified against base `http://a/b/c/d;p?q`)

`g`→`http://a/b/c/g` · `./g`→same · `g/`→`http://a/b/c/g/` · `/g`→`http://a/g` ·
`?y`→`http://a/b/c/d;p?y` · `#s`→`http://a/b/c/d;p?q#s` · ``→`http://a/b/c/d;p?q` ·
`.`→`http://a/b/c/` · `..`→`http://a/b/` · `../..`→`http://a/` · `../../g`→`http://a/g` ·
`//h/x`→`http://h/x` · `../g`→`http://a/b/g`.

## Round 3 (normalization corners)

- **Dot-segment removal never drops an UNMATCHED leading `..`**: `../g/./h` → `../g/h` (relative
  ref keeps its leading `..` — plain RFC remove_dot_segments would eat it); `mailto:a/../b` → `b`
  (matched `..` pops normally). Implement "remove dot segments, but a `..` with no segment to pop
  is emitted verbatim".
- **Port**: empty port is legal and KEPT (`http://h:/p` round-trips `:`; `getPort` → null);
  leading zeros normalize in toString AND getPort (`:0080` → `:80` / 80); an over-i64/huge port
  is a DISTINCT error: `Uri\InvalidUriException: The port is out of range`.
- **Withers are STRICT validators, not encoders**: `withFragment("a b")` / `withUserInfo("a b")`
  throw (`The specified {fragment|userinfo} is malformed`) — they do NOT percent-encode for you.
- **Ctor 2-arg**: `__construct(string $uri, ?Uri $baseUrl)` — base is a `Uri`, not a string.
- **Percent-decoding in normalization is ASCII-unreserved ONLY**: `%C3%A9` stays encoded (hex
  uppercased); `%41` decodes; invalid `%zz` anywhere → whole-URI malformed; a raw non-ASCII byte
  in the input → malformed (must be pre-encoded).
- **withHost(null)** removes the authority (`http:/p`); `withHost("")` keeps an empty one
  (`http:///p`); host normalization (lowercase + pct) applies to wither input in toString, raw
  form kept for toRawString.
- Query may contain `?` and `+` verbatim (`a?b=c`, `a=b+c` — no form-decoding).

## Open items for the build

- Probe `withUserInfo`/`withFragment` malformed-input messages before implementing.
- Probe `__construct($uri, $baseUrl)` two-arg form (base resolution in the ctor).
- Decide the `Uri.parse` phorj surface (throwing vs optional) — the ruling says typed throwing
  `Uri.parse`; PHP's static `parse` is the null-returning form; the ctor is the throwing form.
- The IPv6 toString expansion quirk needs a dedicated golden (uriparser behavior, may be
  version-sensitive — pin it and re-probe on php bumps).
