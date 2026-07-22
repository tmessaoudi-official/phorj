# Web / Parity Slices — Interactive Design Rulings

> Live capture of the developer's interactive rulings (Invariant 15) for the queued slices:
> Entry-roles + config, Rich Request, Invokable/toString, inbound TLS. Persisted per Invariant 19
> so rulings survive context loss. Formal DEC rows are minted into
> `docs/research/full-audit/raw/C-decisions.md` when a slice starts building.

## Decisions Log

- [2026-07-22] AGREED **D1 — Entry roles & config wiring** (LOCKED, ruled by developer):
  - **Role declaration:** `#[Entry(kind: Type)]` — role is a named arg. Active kinds: `Cli`, `Web`.
    Reserved (recognized, not yet built): `Desktop`, `Mobile`, `Worker`, `Embedded`.
  - **Config wiring:** config is injected as a **typed parameter** of the entry (DEC-318
    entry-param injection), NOT named in the attribute. The parameter type (e.g. `WebConfig`) is
    the single declaration of which config the entry wants. Rejected `#[Entry(kind: Web,
    config: WebConfig)]` — duplicates the type between attribute and param (drift risk) and would
    need a new type-as-attribute-arg capability for no gain.
  - **Per-type config for EVERY entry kind** (not just Web): each kind gets its own config type
    (`WebConfig`, `CliConfig`, `DesktopConfig`, …), each built by a `#[Config]` provider.
  - **Config location:** conventionally `src/config.phg` (a `#[Config]` provider per config type);
    convention, not mandatory — a provider may live in any `.phg`.
  - **Precedence chain (highest wins):** CLI flag > env var > `#[Config]` provider (phorj code —
    reads env, computes, typed) > `phorj.json` static block > attribute inline default.
  - **Rationale:** only code (`#[Config]`) can read env + compute; JSON is static, CLI/env are
    strings, attribute args are compile-time constants. The chain gives computed/env-aware config
    plus instant flag/env overrides with no recompile.
  - **OPEN sub-point to confirm at implementation:** whether today's signature inference (DEC-191)
    stays as a fallback when `kind:` is omitted (backward-compat) or is retired in favor of explicit
    `kind:`. Flag when the slice starts.

## Formal Plan
<!-- written at Phase 4 approval, once all D-rulings are locked -->
