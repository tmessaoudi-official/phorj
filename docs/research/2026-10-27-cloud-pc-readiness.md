# Cloud and PC readiness — verification round, 2026-10-27

Branch `ccr-56056e44-aec471` at `b4b4ad6e` (`docs: row 4c close round 2 on frozen 216af26c — DECOMPOSED-PARENT-TRAITS recorded`).
Every claim carries its evidence grade. "Verified" means it was run or read in this session, with the output quoted.

## Verdict

| Area | State | Grade |
|---|---|---|
| Branch content is safe to merge | CI green on the head `b4b4ad6e` (#683) and on `216af26c` (#682); #681 on `a8a874f7` failed and `216af26c` fixed it; tree clean | Verified |
| Commit hygiene | 34 commits ahead of `master`, 0 behind; all 34 authored AND committed by `Takieddine MESSAOUDI <takieddine.messaoudi.official@gmail.com>`; all unsigned (`%G?` = N); 0 `Co-Authored-By` / `Claude-Session` / "Generated with" lines | Verified |
| Merge shape | `master` (`d1e26e68`) is an ancestor of the branch: a fast-forward after re-signing, no conflicts possible | Verified |
| Repo on the PC with the global setup | Repo `.claude/` has no `deny` key and no reference to `tools/cloud-kit`; the kit's scripts refuse outside a cloud container (23/23 kit tests pass, including the off-cloud refusals) | Verified (repo side); the PC itself was not inspected |
| Cloud: the framework auto-installs in a NEW session | **NO, not today.** See below | Verified |
| Cloud: PHP 8.5 leg in a cloud session | **NO.** Fresh sessions get PHP 8.3.6 with no bcmath; CI is the only PHP 8.5 leg | Verified |

## Cloud — what a fresh session actually gets

A read-only probe session was started in the same environment (`env_01SA8ABL53m5T1oZZvPUix5z`) on this branch, then
archived. Its raw output:

```
REMOTE=true KEY_SET=no
head: cannot open '/root/.claude/CLAUDE.md' for reading: No such file or directory
jq: error: Could not open file /root/.claude/settings.json: No such file or directory
php: PHP 8.3.6 (cli)        age: 1.1.1      gawk: MISSING      jq: jq-1.7      cargo-nextest: MISSING
(no bcmath)   git core.hooksPath: unset
```

So, in a new session:
1. `PHORJ_CLOUD_KIT_KEY` is not set, so the kit cannot be decrypted.
2. `~/.claude/settings.json` does not exist, so `arm.sh` never ran and no SessionStart hook installs the kit.
3. `gawk`, `cargo-nextest`, PHP 8.5 and `core.hooksPath` are all absent, so the README's setup script is not what
   the environment runs, or it failed early (its first line is one `&&` chain).

This long-running session has the framework only because it was installed by hand earlier. Its container also has no
kit hook and no key [Verified: `jq '.hooks.SessionStart'` lists only the banner; `KEY_SET=no`].

The platform docs say a setup-script edit and a new environment variable take effect for sessions started AFTER the
change, and that the setup script cannot read the environment's variables or secrets. `arm.sh` was designed for this:
it only registers a hook, and decryption happens later, inside the session, where the variable is visible.

### To make the cloud work (environment settings — the cloud environment menu in the session's title bar, then Edit)

1. **Merge to `master` first.** New sessions start from `master`, which does not have `tools/cloud-kit/` yet, so
   `arm.sh` cannot run in them until the merge.
2. **Environment variable:** `PHORJ_CLOUD_KIT_KEY` = the `AGE-SECRET-KEY-1…` line from
   `~/.config/phorj-cloud-kit.key` on the PC. Never paste it in a chat.
3. **Setup script** (the README's, with an absolute path, because the script's working directory is not documented
   [Unverified]):
   ```sh
   add-apt-repository -y ppa:ondrej/php && apt-get update -qq \
     && apt-get install -y php8.5-cli php8.5-bcmath php8.5-mbstring php8.5-sqlite3 age gawk
   cargo install cargo-nextest --locked
   cd /home/user/phorj && git config core.hooksPath scripts/git-hooks && bash tools/cloud-kit/arm.sh
   ```
   Whether the PPA (launchpad.net) is reachable under the environment's network policy was not tested [Unverified].
   If it is blocked, line 1 fails and everything after it on that line is skipped. That would match what the probe
   saw, but this is a hypothesis.
4. **Check it** with a fresh session. Its banner should print `▸ MODE: autonomous …` and the hook should print
   `cloud-kit: … installed`; `php -v` should say 8.5 and `php -m | grep bcmath` should match. If the hook line is
   missing (for example, `arm.sh` refused because `/root/.ccr` did not exist yet when the setup script ran
   [Unverified]), run `bash tools/cloud-kit/install.sh` once by hand in that session. Errors go to
   `~/.claude/logs/hooks-errors.log`.

## PC — sign and merge to `master`

Wait until CI on `b4b4ad6e` is green (#683: green [Verified]). Then:

```sh
cd /stack/projects/phorj
git status                                    # must be clean
git fetch origin
git checkout master && git pull --ff-only     # master should be d1e26e68
git checkout -B resign origin/ccr-56056e44-aec471
git rebase --force-rebase --gpg-sign master   # re-signs every commit (36 with this report and its count fix); author dates kept, committer date = now
git log --format='%h %G? %an  %s' master..resign | head -40   # expect G (good signature) on every line
git checkout master && git merge --ff-only resign
git push origin master                        # your pre-push hook runs the FULL gate with PHP 8.5
git branch -d resign
git push origin --delete ccr-56056e44-aec471  # optional, once master is green
```

What to expect:
- **No conflicts.** `master` has nothing the branch lacks [Verified: `git rev-list --count HEAD..origin/master` = 0].
- **New hashes for every commit (36 with this report and its count fix).** Docs cite the old short SHAs together with each commit's subject; doc-guards G3 is built to
  recover a citation by its subject after a re-sign [Inferred: the guard's documented rule and commit `7ed814e7`,
  `give the cloud-kit row 4 evidence its commit subject, so doc-guards G3 can recover it`]. `/progress` may show
  some rows as "claimed" until their evidence column cites the new SHAs.
- **A long pre-push run.** It runs the full suite with `--all-features`, both clippy legs, the PHP 8.5 spine,
  size-gate, validate-infra, doc-tests, release build and microbench-gate. The cloud ran a subset; this run is the
  first time the PHP leg and `--all-features` run together on these commits [Inferred: CI runs default features only
  on `ccr-*` branches, per `ci.yml` commit `34035ea2`].
- **Known open defects ride along.** They are pre-existing, recorded with owners, and none of them is a regression on
  `master`: DECOMPOSED-PARENT-TRAITS and STATIC-COMPOSITION-PARITY item 6 (row 4c, close round 3), INIT-CHAIN
  (row 4h), STATIC-REDECL-TYPE (row 4i), DIAMOND-EXPONENTIAL (row 9) — all in `KNOWN_ISSUES.md`.

## PC — bring the cloud memory home

Four encrypted deltas are waiting in `tools/cloud-kit/memory-deltas/` (`20261026T152807Z`, `20261026T165516Z`,
`20261026T212941Z`, `20261027T000933Z`). Apply them in order, as the README says:

```sh
age -d -i ~/.config/phorj-cloud-kit.key tools/cloud-kit/memory-deltas/<stamp>.age >/tmp/delta.tar
tar -tf /tmp/delta.tar                                            # look first
tar -x -C ~/.claude/projects -f /tmp/delta.tar --exclude='*/MEMORY.md'
tar -xOf /tmp/delta.tar ./-stack-projects-phorj/memory/MEMORY.md  # merge this one by hand if present
```

`MEMORY.md` is excluded because extracting it would REPLACE the PC's index.

## Not verified, and why

- **The PC itself** (its `~/.claude`, signing key, PHP 8.5, hooks): this session cannot reach it.
- **Decryption of `kit.tar.age` with the real key:** no session has had the key yet.
- **The setup script's working directory, and whether the PPA is reachable:** not documented, and a new session
  would have to run the script to find out.
- **Plan row 4c is still `doing`:** close round 3 has to fix DECOMPOSED-PARENT-TRAITS and rule on the
  trait-static visibility question.
