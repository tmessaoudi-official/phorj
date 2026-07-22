# PHP-gap round 2 — UNMAPPED-anywhere findings (2026-07-22)

**Method.** Read the coverage SSOTs (MASTER-PLAN waves 0–6 + Ω-waves + feature packs + Appendix A rejects;
M-gap-matrix 824 rows incl. every FN-group "named-missing" note; FEATURES.md; KNOWN_ISSUES.md incl. the
DEC-PENDING/deferral sections; SLICE-STATE queue; the 2026-07-16 full-reopen audit D0 PHP-8.4/8.5/8.6
re-sweep and its dispositions; D-php-surface.md — the 869-row PHP inventory the matrix scores). Then swept
PHP 8.x from model knowledge across language / stdlib / runtime-ops / web, and **grep-verified every
candidate against the whole docs corpus** before listing. An item counted MAPPED if it appears as a matrix
row (incl. GU rows — the Ω-6 charter batches those), a DEC ruling, a queue/pack entry, a KNOWN_ISSUES
deferral, or an Appendix-A reject. Everything below had **zero mapping hits** (or a demonstrably
wrong/over-credited verdict, flagged as such). Evidence grade per line: [Verified: grep/doc-read] unless
noted.

**Counts: 8 TOP · 8 MID · 9 LOW/REJECT-candidates = 25 findings.**

A structural sub-finding first: **D-php-surface.md (the 869-row inventory) never inventoried the PHP
extension domains soap/ldap/imap/snmp/bz2/dba/sysv-IPC/shmop/pspell/enchant/calendar/getrusage** — so the
824-row parity model has a small silent denominator hole, and the "no silent scope drops" invariant
(Appendix A) currently has nothing recorded for them. Most are REJECTs, but they need rows.

---

## TOP — adoption-critical

| DOMAIN | item | why it matters vs PHP | suggested phorj-better design | wave |
|---|---|---|---|---|
| WEB | **`phg serve` TLS/HTTPS story** — zero mention in any doc [Verified: grep tls/https×serve = 0] | every real PHP deployment terminates TLS (Apache/nginx/FPM); phorj owns its server yet has neither native TLS nor a documented reverse-proxy posture — no production deployment story exists on paper | native rustls termination (`serve --tls-cert/--key`, dep already admitted) OR an explicit ruled "reverse-proxy-only" posture doc — adjudicate, don't leave silent | W6 (GA blocker) |
| WEB | **trusted-proxy headers** (`X-Forwarded-For/Proto/Host`, RFC 7239 `Forwarded`) [Verified: 0 hits] | behind any proxy, client IP / scheme / host are wrong → Secure-cookie decisions, rate-limit keys, logs and URL generation all lie; Symfony/Laravel ship trusted_proxies as core security config | typed `TrustedProxies` config on serve, deny-by-default (untrusted = ignore headers), CIDR allow-list; pairs with DEC-270's SSRF IP machinery | W3/Web pack |
| WEB | **streaming response bodies** (PHP `readfile`/`fpassthru`/incremental `echo`+`flush`) [Verified: 0 hits; `Response` is a fully-materialized value] | large downloads/exports currently need the whole body in memory; SSE/WebSocket (mapped in the Web pack) don't cover plain chunked/file streaming | `Response.stream(Iterator<bytes>)` riding the shipped DEC-257 Iterator — lazy, typed, back-pressured; LADDER leg = PHP chunked echo | W3/Web pack |
| WEB | **static-file Range/`Accept-Ranges` + response compression (`Content-Encoding: gzip`)** [Verified: 0 hits] | PHP inherits both from the front server; phorj's own static server (ETag/304 shipped) silently breaks video seek/resumable downloads and ships uncompressed text | Range single-range support + negotiated gzip (zlib is already in the compression queue) in the static handler; correctness-testable, no config | W3/Web pack |
| STDLIB | **HttpClient outbound proxy support** (`HTTP(S)_PROXY`, CONNECT tunneling) + **custom CA roots + mTLS client certs** [Verified: 0 hits; only bundled webpki roots documented] | corporate networks are proxy+private-CA by default (curl covers all three); without them phorj HTTP is unusable in exactly the enterprises the lifter targets; KNOWN_ISSUES maps HTTP/2/pooling/cookie-jar as future slices but not these | explicit `ProxyConfig`/`TlsConfig` on the Transport seam (Secret-typed creds, no ambient env magic — or env honored behind an explicit opt-in) | W3/Web pack |
| LANGUAGE | **class-constant expressiveness** — const *expressions* (arrays, arithmetic, enum refs), PHP 8.1 `new` in initializers, 8.3 typed class constants, 8.5 closures/casts in const-exprs + attributes on constants | phorj class consts are literal-init-only and top-level `const` is rejected (SYN-024/110); the D0 audit *inventoried* the 8.4/8.5 const-expr items but never dispositioned them [Verified: absent from D0.3/D0.4 delta tables] — real ported code uses const arrays/enum consts everywhere, a direct lifter blocker | compile-time-evaluated const expressions (types mandatory — beats 8.3), closed evaluable subset, erased to PHP const exprs 1:1 | W4 |
| WEB | **server-side HTTP/2 (h2c/h2)** — client-side HTTP/2 is queued [Verified: KNOWN_ISSUES:374], server side has zero mention | PHP gets HTTP/2 from the front server; if serve grows native TLS (above), staying HTTP/1.1-only forfeits the "beats PHP runtime" claim on every modern benchmark | h2 behind the TLS slice (ALPN), same handler contract; explicitly rule its dep (h2 crate vs refuse) via the dependency policy | post-1.0 |
| LANGUAGE | **enum interface conformance + enum constants** (PHP 8.1: `enum X: string implements HasLabel`, `const` in enums) | the sugar-pack "enum impl blocks" maps *methods* only [Verified: no doc mentions enums implementing interfaces or enum consts]; backed enums shipped (DEC-302) make this the next thing real PHP 8.1 code hits in the lifter | enums may `implements` interfaces (they already lower to a class model, so conformance checking is nearly free) + consts riding the const-expr slice | W4 |

## MID

| DOMAIN | item | why it matters vs PHP | suggested phorj-better design | wave |
|---|---|---|---|---|
| STDLIB | **`pack()`/`unpack()` binary struct codec** — absent from the 869-row D-surface itself [Verified: no pack/unpack rows]; `Bytes.format` (Ω-3 design) is sprintf-for-bytes, not fixed-width binary encode, and no read direction exists anywhere | binary protocols/file formats (network packets, GD-free image sniffing, git objects) need endian-aware int/float encode+decode; PHP has had it since forever | typed `Bytes.readInt32BE/writeUInt16LE…` family + a declarative `BinaryLayout` codec (compile-checked field spec — beats PHP's stringly `"Nver/nlen"`) | W4/Ω-3 |
| LANGUAGE | **trait constants (PHP 8.2)** — D-surface SYN-115 lists them; matrix credits SYN-115 **CE** but the phorj trait doc/example surface is "methods, state, ctor, abstract, hooks" with constants never mentioned [Verified: examples/guide/traits.phg header; FEATURES row] | likely an over-credited matrix verdict — lifted PHP 8.2 traits with consts would fail; also feeds the const-expr slice | verify in code; if absent: trait `const` participating in conflict resolution like methods | W4 (verify first) |
| STDLIB | **HttpClient streaming/large bodies** — hard 64 MB cap is documented, but no streaming download/upload (curl writes to a stream/file) [Verified: FEATURES row; no streaming mention] | file mirroring, big exports, webhook relays all break at the cap with no recourse | `client.stream(req): Iterator<bytes>` + `.toFile(path)` on the same Iterator protocol; cap stays the *default* | W3/Web pack |
| WEB | **pluggable session store contract** (PHP `SessionHandlerInterface` — Redis/DB-backed sessions) | shipped Core.Session store is worker-shared *in-process* → sessions can't survive restarts or scale past one host; the DEC layered-openness ruling enumerates seams (DbDriver, LogSink, HttpTransport, CacheBackend…) and omits SessionStore [Verified: ruling text] | add `SessionStore` to the public-contract list; ship Memory (now) + Db-backed (rides Core.DatabaseModule) | W3/Web pack |
| STDLIB | **`sys_getloadavg`-adjacent runtime introspection: CPU time / `getrusage`** [Verified: 0 hits anywhere; memory_get_* is covered via Core.Runtime] | self-instrumenting long-lived serve processes (the observability pack) need CPU-time, not just RSS | `Runtime.cpuTime(): Duration` (user+sys), std-only via /proc + getrusage syscall wrapper; feeds Metrics | W5/Runtime pack |
| OPS | **`phg env`/`phg doctor` (phpinfo analog)** — matrix N/A's phpinfo, but the *diagnostic* capability (effective config, feature flags, extension list, oracle php path, versions in one dump) has no home [Verified: no doctor/env cmd anywhere; `phg extensions` covers one slice] | "what is this binary and what can it do" is the #1 support question; phpinfo is PHP's most-used ops tool | `phg env` — deterministic, secret-free, machine-readable (`--json`); extends the shipped `phg extensions` | W6 |
| OPS | **task-runner scripts in `phorj.json`** (composer `scripts` DX half — install-time hooks stay REJECTED for supply-chain, already recorded as a Pass-2 brag) | `composer test`/`composer fix` muscle memory; DEC-319 DX north-star says smooth tooling | explicit `phg run-script <name>` — never implicit, never on install (keeps the supply-chain win); arg-vector only per W4-11 | post-1.0 |
| WEB | **graceful reload / zero-downtime deploy** (PHP-FPM `reload` semantics) [Verified: 0 hits; DEC-204 onShutdown covers stop, not handover] | long-lived serve replaces FPM's per-request model — but FPM gives ops free graceful reloads; phorj currently drops connections on redeploy | `serve` SIGHUP = finish in-flight requests, re-exec new binary on the inherited listener fd | post-1.0 |

## LOW / REJECT-candidates (record in Appendix A — currently *silent* scope drops, not even inventoried)

| DOMAIN | item | why it matters vs PHP | suggested disposition | wave |
|---|---|---|---|---|
| STDLIB | **SOAP** (`ext/soap` — still in PHP core) | enterprise WS-*/legacy B2B integrations exist; the one domain where "PHP does it, phorj can't at all" is checkable in 5 min | REJECT-with-reason (userland over HttpClient+XML once W4-10 lands); record row | REJECT |
| STDLIB | **LDAP** (`ext/ldap`) | Active Directory auth is table-stakes in the enterprises the lifter targets (Symfony ships an Ldap component) | post-1.0 extension candidate over Core.Net TLS-or-refuse; record either way | post-1.0 |
| STDLIB | **IMAP** | unbundled from PHP core to PECL in 8.4 — PHP itself is dropping it | REJECT (Core.Mail is send-only by design); record row | REJECT |
| STDLIB | **SNMP** | niche ops protocol, PECL-grade usage | REJECT; record row | REJECT |
| STDLIB | **dba + SysV IPC family (shmop/sysvsem/sysvmsg/ftok)** | shared-memory concurrency contradicts the ruled isolates+channels model (W5-16) | REJECT-with-reason (points at the M-Parallel ruling); record rows | REJECT |
| STDLIB | **pspell/enchant** (spellcheck) | dictionary deps, tiny usage | REJECT; record row | REJECT |
| STDLIB | **ext/calendar** (easter_date, julian/jewish conversions) | tiny usage; icu4x (DEC-271) brings real calendar systems anyway | REJECT, pointer to Core.Intl calendars | REJECT |
| STDLIB | **tidy** (HTML repair) | the W4-10 fork's HTML5 parser (Dom\HTMLDocument row) subsumes repair-parsing | REJECT, pointer to the XML/DOM fork | REJECT |
| STDLIB | **compression format breadth: bz2** (+ note: Core.Compress names only zlib/gzip/zip anywhere) | `ext/bz2` is core PHP; cheap to note in the Core.Compress spec rather than rediscover | fold as a format row into the queued compression slice (adopt-or-reject there) | W4 note |

---

## Checked and already MAPPED (verified — do NOT re-add; listed to save the next pass)

- **Multipart forms / file uploads / request bodies**: mapped by the DEC-218 developer note ("HTTP verbs +
  request bodies/file uploads in a clean, well-organized OOP way") — but only as one sentence; the Web
  pack build slice should cite it. `request_parse_body`/`$_FILES` are D-surface rows.
- **Generators/yield** (W4-2, queue #6/#11) · **Fibers** (DEC-225 spike) · **WeakReference/WeakMap**
  (DEC-205 collector-then-`Weak<T>`, phased) · **attributes+reflection v2** (Ω-4, SYN-118 P) ·
  **__toString/__invoke** (W4-3) · **magic __get/__set/__call, references, goto, LSB, destructors,
  eval/include, error handlers** (all GD/rejected with reasons) · **anonymous classes** (SYN-117 GU,
  charter tail) · **lambdas capturing `this`** (SYN-172 documented deferral).
- **named args/variadics/spread** (DEC-297/298/299 ruled) · **enum methods** ("enum impl blocks" sugar
  pack) · **tuples** (DEC-288 shipped) · **backed enums** (DEC-302 shipped).
- **DateTime/tz** (DEC-247, tz-crate approved) · **intl/ICU** (DEC-271 icu4x) · **XML/DOM/XPath/HTML5
  parser/XSLT** (W4-10 open fork — all 12 FN-XML rows incl. Dom\HTMLDocument & XSLTProcessor are D-surface
  rows) · **mb-tail, iconv** (M-text programme) · **streams/fopen zoo, sockets** (FN-STREAM/SOCK GU +
  Core.Net pack) · **subprocess** (W4-11 + queue #8) · **compression/zip** (queue #1 tail + Core.Compress
  pack) · **GD/exif** (DEC-273 Image extension, decode-limits) · **finfo** (DEC-273 advisory) ·
  **readline** (DEC-273 Cli) · **BigInt/BCMath/GMP** (W4-13) · **serialize/var_export/print_r** (queue #7)
  · **SPL heaps/deque/PQ** (queue #9) · **sscanf, hash breadth, needs_rehash, sodium/openssl, preg tail,
  wordwrap/strtr/ucwords/soundex, realpath/chmod/flock/tempnam, DNS/ftp/inet, getopt, proc-title,
  php_uname, lazy objects, Closure::getCurrent, curl_multi** — all matrix rows (GU/P/GP) under the Ω-6
  charter or D0 dispositions.
- **Session/CSRF/cookies/middleware/rate-limit/SSE/WebSocket/templates** (Web pack + shipped Core.Session,
  DEC-242 CHIPS, DEC-283 .phgml) · **HTTP/2 client + pooling + cookie jar** (KNOWN_ISSUES future slices;
  keep-alive = DEC-266) · **DB pooling / COPY / LISTEN-NOTIFY** (recorded out-of-scope in the Db spec) ·
  **scheduler/signals** (Signals+Scheduler pack, DEC-204) · **REPL/doc/profiler** (W5-15, F-profiler M13) ·
  **AOT/bytecode cache** (I-aot v2; RT-006) · **FFI** (Openness pack slice-1, E-TRANSPILE-FFI ladder) ·
  **install-time scripts** (deliberately rejected — Pass-2 brag #26) · **`.phorj` container compression
  deflate/zstd** (build spec, unrelated to stdlib compression).
