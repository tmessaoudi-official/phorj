---
name: Bug report
about: Report a reproducible bug — especially any crash, panic, hang, or wrong output
title: "[bug] "
labels: bug
assignees: ""
---

## What happened

<!-- A clear description of the bug. Wrong output? A crash/panic? A hang? -->

## Minimal reproduction

<!-- The smallest .phg program (or stdin/-e snippet) that triggers it. -->

```phorj
function main() {
    // ...
}
```

## Command

<!-- e.g. `phg run repro.phg`, `phg run -e '...'`, `phg build repro.phg` -->

```sh
phorj ...
```

## Expected vs. actual

- **Expected:**
- **Actual:** <!-- paste stdout/stderr; for a crash, paste the full message -->

## Does it differ between backends?

<!-- Important for parity bugs: do `phg run` and `phg run` produce DIFFERENT output? -->

- [ ] `run` and the VM disagree
- [ ] only `run` is wrong
- [ ] only the VM is wrong
- [ ] both agree (output is just wrong, or it's a crash)

## Environment

- phorj version (`phg --version`):
- OS / arch:
- built from source / prebuilt binary:
