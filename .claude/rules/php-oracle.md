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

# PHP oracle resolution — the toolchain image, the host-runner fallback, the no-pin check

Moved out of `CLAUDE.md` § "Toolchain & quality gate" on 2026-11-14 (`/rules-split`, plan
`docs/plans/2026-11-14-ground-prep.plan.md`). The text below is unchanged except one figure: the test-side fallback
was "present in 9 test files" and is in 12 (`grep -l PHORJ_PHP tests/*.rs`, 2026-11-14); the resolution paragraphs
were rewritten on 2026-11-19 when the toolchain moved into Docker. `CLAUDE.md` keeps the gate
command, the transpile floor, the "never gate against the bare `php`" rule and a one-line index entry. A new lesson
about the oracle belongs HERE, not in `CLAUDE.md`.

**Since 2026-11-19 there is ONE resolution path, the toolchain image** (`docker/Dockerfile`, plan
`docs/plans/2026-11-19-dockerize.plan.md`): the local hooks and gates (through `scripts/dk`) and the CI jobs all run
in the image built `FROM php:8.5-cli-trixie` (8.6 for `gate-php86`, the same Dockerfile with `PHP_MINOR=8.6`), which
sets `PHORJ_PHP=/usr/local/bin/php`. The tag floats the patch, like the old glob. bcmath (and the PDO drivers) are
shared extensions there, so `/usr/local/bin/php` is a wrapper that names them on the command line when `-n` is given
(`php -n` loads no ini); everything else passes through to `php.real`. One PHP build means one PCRE2 (10.44 in both
images, measured 2026-11-19): the old split between PHP 8.5's bundled 10.44 locally and ubuntu-24.04's system 10.42 on
CI (DEC-522: `{,n}`) is gone, and a `preg_*` pin now holds wherever the gate holds. The non-gating `oracle-nightly`
(8.7-dev, no official image) is the one job still on `setup-php`.

`scripts/toolchain.env` stays as the HOST-runner resolver (claude.ai cloud sessions, `git config phorj.runner host`):
it capability-checks an inherited `PHORJ_PHP` (bcmath, probed with a DRAINING pipe: `grep -q` closes the pipe early,
php dies with SIGPIPE, and under `pipefail` a valid oracle is rejected ~13% of the time — measured, DEC-456), then
globs `php-8.5.*` and `php8.5`. Its phpbrew glob (`/stack/tools/phpbrew/php/`) is obsolete on this machine now that
the hooks run in the image; it is kept until the developer agrees to remove it. No script may pin a patch version —
`scripts/validate-infra.sh` enforces that over every tracked shell script and workflow (a variable-built path is a
resolver, not a pin, and is deliberately not flagged), and `scripts/test-validate-infra.sh` (run by pre-push) keeps
the check itself from going dark.

**A PHP-side expectation must hold on every gating PHP, not only the floor.** The transpiled PHP is version-stable
when it passes its arguments explicitly (`trimAscii` emits `trim($s, " \t\n\r\0\x0B\x0C")`), but `tests/lift_roundtrip.rs`
runs the ORIGINAL PHP source on whichever PHP the job installs, so a PHP default that changes between 8.5 and 8.6 reds
`gate-php86` alone. First case: 8.6 added `\f` to `trim()`'s default set (DEC-588, ruled 2026-11-23: `trimAscii`
follows 8.6, and the round-trip fixture that leans on `\f` asserts the 8.5 split instead of skipping it). Compare
`scripts/dk php …` against `scripts/dk --php 8.6 php …` before adding a fixture that leans on a PHP default.
