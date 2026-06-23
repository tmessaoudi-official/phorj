# Namespace Reshape — PascalCase enforcement + examples migration + audit

## Decisions Log
- [2026-06-23] AGREED: do all of (1) implement PascalCase package enforcement, (2) migrate all
  examples+projects to PascalCase, (3) line-by-line audit + feature→example coverage matrix.
- [2026-06-23] AGREED: philosophy — examples must model the *enforced* form so they are
  forward-compatible (write today what will be required tomorrow, even before enforcement lands).
- [2026-06-23] AGREED: `module` (manifest distributable coordinate, concept C) stays lowercase
  (Composer `vendor/package` style); only **package/folder segments** (concept A/B) become PascalCase.
- [2026-06-23] AGREED: reserved entry package `main` → `Main` (D2); the entry *function* `main()`
  stays camelCase (it is a value identifier).

## Context (verified state, 2026-06-23)
- Reshape slices already DONE: 1 (manifest `module`), 2a (identifier casing E-NAME-CASE/E-TYPE-CASE),
  4 (library types / E-PKG-TYPE lifted). Stdlib already PascalCase + native fns camelCase.
- Full gate GREEN at `df40926` (648 lib + 82 differential incl. all 73 examples + 4 projects + PHP-8.4).
- `mangle()`/`pascal()` already PascalCase source segments for PHP output ⇒ the rename is
  **output-preserving**; the differential harness proves byte-identity through every step.

## Formal Plan
- **Phase A — `main` → `Main`** (one atomic green commit): `package Main`→`package Main` across all
  `.phg`, src inline test programs, tests fixtures, user-facing hints/docs; engine reserved literal
  `== ["main"]` → `["Main"]` (loader 268/334/413/1110/1124 + test asserts); checker auto-prepend +
  E-NO-PACKAGE/E-RESERVED-PACKAGE hints. Function `main()` untouched.
- **Phase B — user packages `acme.*` → `Acme.*`**: rename folders, package decls, imports, call-site
  leaves, fixture strings (src/tests), docs. Module coordinates stay lowercase. Green commit.
- **Phase C — `E-PKG-CASE`**: checker rule on package-decl + import-path segments (PascalCase);
  negative tests; `phg explain E-PKG-CASE`. Green commit.
- **Phase D — audit + coverage matrix**: line-by-line read of every example; feature→example matrix;
  fix real issues; update KNOWN_ISSUES/CHANGELOG/examples/README/specs. Green commit(s).

## Acceptance
- `PHORGE_REQUIRE_PHP=1 PHORGE_PHP=php-8.4.22 cargo test --release` green after every phase.
- clippy + fmt clean. Every example PascalCase. `E-PKG-CASE` rejects a lowercase package decl.
