# Response-side header injection guard (DEC-363, RULED 2026-07-26) — **P1 SECURITY**

> **Status:** RULED by the developer 2026-07-26, **not yet built**. Canonical home for the rule
> (Invariant 19). Decision *identity + status* = the DEC-363 row in
> `docs/research/full-audit/raw/C-decisions.md`. Original analysis:
> `docs/research/2026-07-25-completeness-register.md` §7.3.

## The vulnerability (reachable from ordinary handler code on a shipped `phg serve`)

`Response.withHeader(name, value)` interpolates **both** arguments straight into a header line with
zero validation (`src/cli/http_prelude.rs:71-73`), `Cookie.render()` does the same for its string
fields (`:45`), `serialize()` CRLF-joins the lines into the response head (`:91-99`), and
`src/serve/handlers.rs:189` `respond_once` returns the handler's bytes **verbatim** — so no Rust-side
serializer exists that could re-validate.

Reproduced with `phg run` (`target/release/phg` @ `27f08cb`):

```phg
string evil = Bytes.toString(b"x\x0d\x0aX-Injected: yes\x0d\x0a\x0d\x0a<html>pwned</html>") ?? "";
Response.text(200, "ok").withHeader("X-User", evil).serialize()
```
```
HTTP/1.1 200 OK
Content-Length: 2          <-- still describes "ok"
Content-Type: text/plain
X-User: x
X-Injected: yes            <-- injected header
                           <-- head terminated early
<html>pwned</html>         <-- injected SECOND body

ok
```

`Content-Length: 2` while ~30 further bytes follow in the same response ⇒ this is a
**request-smuggling / desync** primitive, not only response splitting. The header **name** is equally
unvalidated (an evil name injects its own line), and `Cookie` carries the same payload through
`Set-Cookie`.

### The five injectable surfaces

| Surface | Site |
|---|---|
| `withHeader` **name** | `http_prelude.rs:71-73` — raw into `"{name}: {value}"` |
| `withHeader` **value** | same line |
| `Cookie.name` | `http_prelude.rs:45` — raw into `"{this.name}={this.value}; Path={this.cookiePath}"` |
| `Cookie.value` | same line |
| `Cookie.cookiePath` | same line |

`Cookie`'s other three fields (`isSecure`, `isHttpOnly`, `isPartitioned`) are `bool` and cannot carry
CR/LF/NUL, so **three** of the six `Cookie` fields need guarding, not all six.

### The aggravating fact: the request side already rejects this

`src/ext/http_client/natives.rs:112-118` rejects **CR, LF, or `:` in a header name** and **CR or LF in
a value**, pinned by `src/ext/http_client/tests.rs:450 header_injection_is_rejected_at_the_gate`
(which feeds `"a\r\nHost: evil"`). The outbound response path has no equivalent — the asymmetry is what
raised this from "small, ranked 25th" to a top-10 item.

## THE RULE

> **Guard in the phorj prelude, panic-class fault, at `Response.withHeader` and the `Cookie`
> constructor.** Rejected character class: **CR, LF, NUL** in any value, plus **`:`** in a header name.
> Wording mirrors the request-side gate: ``header `{name}` contains a forbidden character``.

**Why the prelude:** one implementation in phorj source ⇒ all three legs (`run`, `run --tree-walker`,
transpiled PHP) identical **by construction**. A Rust-side guard in `respond_once` was **rejected**:
`phg build --php` never executes `respond_once`, so the PHP leg would stay fully exploitable — an
Invariant-1 breach plus a security asymmetry between two supported deployment paths.

**Why the `Cookie` constructor rather than `render()`:** every builder (`path()`, `secure()`,
`httpOnly()`, `partitioned()`) re-constructs via `new Cookie(...)` (`http_prelude.rs:29-40`), so the
constructor is a single chokepoint covering all three string fields **and** all four builders.

**Why panic-class rather than a checked throw** — settled by evidence, not preference:
`respond_once` documents and implements *"a runtime fault degrades to a 500 — never a panic (EV-7)"*,
and the server is *"Resilient by design (GA blockers B3/B4): a fault on one request…"*
(`src/serve/handlers.rs:143`, `:186-188`). So a fault here is **a 500 on that one request, never a
server kill** — it is not a remote DoS vector, which was the only real argument for making the
builders throw. Keeping them non-throwing also avoids rippling `throws` into every handler and every
`examples/web/` file.

Rejected alternative, recorded so it is not re-litigated: **guard only in `serialize()`**. One
chokepoint, covers future fields, still byte-identical — but it fires at respond time, so the
diagnostic cannot name the `withHeader` call that built the bad value. Worse debugging, same safety.

## Ruled extras

1. **NUL joins the rejected set on BOTH sides.** The request-side gate rejects CR/LF but **not** NUL;
   PHP's own `header()` rejects NUL. The request side is widened in the same change, so the two gates
   stay identical and a known header-truncation trick is closed.
2. **Pre-check helpers ship:** `Http.isValidHeaderName(string): bool` and
   `Http.isValidHeaderValue(string): bool`. Because a violation is a 500 rather than a 400, a handler
   holding **user-derived** input otherwise has no way to validate first and return a clean 400. These
   make the 400 path expressible without making the builders throw.

## Definition of done

1. Guard at `Response.withHeader` (name + value) and the `Cookie` constructor (`name`, `value`,
   `cookiePath`), rejecting CR/LF/NUL and `:` in names.
2. The request-side gate widened to NUL; its existing
   `header_injection_is_rejected_at_the_gate` test extended.
3. `isValidHeaderName` / `isValidHeaderValue` on `Core.Http`, exercised on both Rust backends.
4. A test per injectable surface feeding a CRLF payload and asserting the fault — plus one asserting
   the **serialized head** no longer splits.
5. Byte-identity re-verified: the guard lives in the prelude, so the transpiled PHP leg must fault
   identically (`PHORJ_REQUIRE_PHP=1`, php-8.5 floor).
6. Faults cannot be runnable examples — capture the injection case in `examples/web/README.md`
   (Invariant 9's carve-out).
