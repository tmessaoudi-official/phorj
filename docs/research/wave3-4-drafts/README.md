# Wave 3–4 design drafts (2026-07-03)

Design proposals for three XL `DESIGN-NEEDED` items, produced by a parallel research fan-out while
the import-redesign S1/S2 proceeded on the main line. **Status: DRAFTS — proposals, not frozen specs;
each carries PENDING developer adjudications (invariant 15) that an autonomous session records but does
NOT rule.** They promote to `docs/specs/` (frozen) once the developer adjudicates the forks below.

| Draft | Item | The blocking fork |
|---|---|---|
| `w3-1-db-access.md` | W3-1 Database access | **Dependency policy amendment** — SQLite (`rusqlite`) is not one of the policy's 4 domains; admitting it is a 5th-domain amendment. Ships `Core.Sql` (pure, Tier A) with NO adjudication first. |
| `w3-2-http-client.md` | W3-2 HTTP client | **Dependency policy amendment** — HTTPS needs a TLS crate (`rustls`), also outside the 4 domains. Ships `Core.Url` (pure, zero-dep) first; plaintext HTTP is std-doable but low-value. |
| `w4-4-unicode-strings.md` | W4-4 Unicode strings | **No dep**, but a byte-identity landmine: Unicode-correct **case folding** diverges from the `php -n` tier-1 oracle → LADDER-quarantine candidate. Codepoints (not graphemes) as the default unit. |

**Cross-cutting finding:** W3-1 and W3-2 — the bulk of the Wave-3 +6-parity jump — are BOTH gated on
one decision: *whether to amend the dependency policy to admit `rusqlite` + `rustls` as new vetted,
feature-gated, spine-quarantined domains* (the `corosensei`/`ctrlc` shape). Both also have a pure,
zero-adjudication P0 (`Core.Sql`, `Core.Url`) that ships regardless. W4-4 is independent (no dep) but
needs its own case-folding-vs-byte-identity ruling.
