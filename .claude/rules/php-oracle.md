---
paths:
  - "scripts/toolchain.env"
  - "scripts/validate-infra.sh"
  - "scripts/test-validate-infra.sh"
  - "tests/**"
  - "src/transpile/**"
  - "src/lift/**"
  - "src/native/**"
  - ".github/workflows/**"
---

# PHP oracle resolution — local resolver, CI's second path, the PCRE2 split, the no-pin check

Moved out of `CLAUDE.md` § "Toolchain & quality gate" on 2026-11-14 (`/rules-split`, plan
`docs/plans/2026-11-14-ground-prep.plan.md`). The text below is unchanged except one figure: the test-side fallback
was "present in 9 test files" and is in 12 (`grep -l PHORJ_PHP tests/*.rs`, 2026-11-14). `CLAUDE.md` keeps the gate
command, the transpile floor, the "never gate against the bare `php`" rule and a one-line index entry. A new lesson
about the oracle belongs HERE, not in `CLAUDE.md`.

**Locally** the oracle is resolved by `scripts/toolchain.env` and nothing else — it globs
`php-8.5.*` newest-first, requires `bcmath` (probing with a DRAINING pipe: `grep -q` closes the pipe
early, php dies with SIGPIPE, and under `pipefail` a valid oracle is rejected ~13% of the time —
measured, DEC-456), and capability-checks even an inherited `PHORJ_PHP`, so a stale export from a
long-lived shell is announced and ignored rather than trusted. **CI does NOT source it** — the
workflows set only `PHORJ_REQUIRE_PHP=1` and rely on the test-side `PHORJ_PHP`-or-`php` fallback
present in 12 test files, with `setup-php` supplying an 8.5+bcmath build; that is correct today but
is a SECOND resolution path, so a change here is not automatically a change there. The two paths also
resolve DIFFERENT PCRE2 builds — PHP 8.5's bundled 10.44 here, ubuntu-24.04's system 10.42 on CI (inferred from CI's behaviour; `php:8.3-cli` in Docker reproduces it) — so a
`preg_*` behaviour PCRE2 changed between them passes locally and reds CI (DEC-522: `{,n}`); a pin on
such a behaviour must hold on both, or the construct is refused on every leg. No script may pin
a patch version — `scripts/validate-infra.sh` enforces that over every tracked shell script and
workflow (a variable-built path is a resolver, not a pin, and is deliberately not flagged), and
`scripts/test-validate-infra.sh` (run by pre-push) keeps the check itself from going dark.
