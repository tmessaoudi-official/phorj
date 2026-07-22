# Blast-Radius Reference

> Consulted at Phase 5 entry for any destructive or risky operation.
> Core insight: blast radius is **state-dependent** — the same command can be trivial
> in one context and unrecoverable in another. Run pre-flight checks first; they cost
> 3 seconds and the consequences of skipping them can be permanent.

---

## Pre-Flight State Checks

Run the applicable checks **before** executing, not after.

### Git state gate
```bash
git status --porcelain                  # uncommitted / untracked changes?
ls .git/MERGE_HEAD 2>/dev/null          # merge in progress?
ls .git/REBASE_HEAD 2>/dev/null         # rebase in progress?
ls .git/CHERRY_PICK_HEAD 2>/dev/null    # cherry-pick in progress?
```
If **any** of those files exist: `git reset`, `git checkout`, `git stash`, `git clean`
all have doubled blast radius — they can abort the in-progress operation silently.

### Docker state gate
```bash
# Before docker compose down: does it carry -v (volumes) or --rmi (images)?
# Before docker volume rm / docker system prune: list what would be removed.
docker volume ls
docker ps -a
```

### Scope / glob gate
Before any glob (`*.sh`, `-r dir/`, `pkill -f pattern`):
```bash
echo rm *.sh          # verify expansion before rm *.sh
pgrep -a pattern      # verify matches before pkill -f pattern
```

### Protected paths — never auto-delete / never bundle
`~/.claude/state/` and `~/.claude/projects/<slug>/state/` hold **persistent safety sentinels**: the
bypass toggles (`ask-human-gate-bypass`, `ask-bash-firewall-bypass`, `ask-human-question-guard-bypass`)
and the persistent `autonomous-3c-bypass`. They are removed ONLY by deliberate user action — each
create/delete is `ask`-gated in `settings.json` AND matched by the firewall `danger_patterns`
substring. No cleanup/sweep may delete them (`claude-cleanup.sh` enforces a `*/state/*` guard in
`_cc_act`/`_cc_act_dir`) and no bundle may ship them (the export pipeline never copies `state/` and
asserts against leakage at build time).

---

## Category Reference

### Git — Context-Dependent

| Command | Hidden side effect | Dangerous context |
|---------|--------------------|-------------------|
| `git reset HEAD` / `git reset` | Removes `MERGE_HEAD`/`REBASE_HEAD`/`CHERRY_PICK_HEAD` → silently aborts in-progress op | Any in-progress merge / rebase / cherry-pick |
| `git checkout <branch>` | Silently discards uncommitted changes if conflicting | Dirty working tree |
| `git clean -fd` | Removes untracked source files, not just build artifacts | Any untracked WIP |
| `git commit --amend` | Rewrites commit hash — breaks anyone who already pulled | After `git push` |
| `git add -A` | Stages accidental deletions alongside intentional changes | Dirty tree with inadvertent deletes |
| `git fetch --prune` | Removes remote-tracking refs without warning | Shared branches deleted upstream |
| `git stash drop` | Permanent — no recycle bin | Stash contains unsaved context |

### Docker — Data Destruction

| Command | Hidden side effect | Dangerous context |
|---------|--------------------|-------------------|
| `docker compose down -v` | Destroys **all** named volumes (DBs, uploads, persistent data) — `-v` is the killer flag | Any persistent data present |
| `docker system prune` | Removes **all** unused resources across **all** projects on the host | Other projects share the daemon |
| `docker volume rm` | Permanent, instant, no confirmation | Volume holds database data |
| `docker network rm` | Disconnects **all** containers on that network simultaneously | Running containers depend on it |
| `docker rmi` | Can break containers that aren't running but reference the image | Derived or stopped containers exist |
| `docker compose down` (no `-v`) | Removes containers only — volumes preserved | Safe — kept here for contrast |

### Shell / Filesystem

| Command | Hidden side effect | Dangerous context |
|---------|--------------------|-------------------|
| `rm -rf symlink/` | Trailing `/` → deletes contents of symlink **target**, not the symlink | Target is a shared directory |
| `chmod -R 777 .` | Makes `.git/`, `.env`, SSH keys world-writable | Any secrets or git history in tree |
| `sed -i 's/x/y/g' *.sh` | Glob expands to unexpected files; `-i` has no undo | Pattern matches broadly |
| `mv src dst` | Silently overwrites `dst` — no prompt by default | `dst` already exists |
| `cat > file` | Truncates `file` **immediately** before any data arrives | `file` has content you need |
| `find . -exec rm {} \;` | Executes on every matched file | Condition or path too broad |
| `cp -r src/ dst/` | Behavior differs if `dst/` already exists vs doesn't | Destination state unknown |

### Environment / Shell State

| Command | Hidden side effect | Dangerous context |
|---------|--------------------|-------------------|
| `source script.sh` | Permanently mutates current shell (PATH, functions, traps) — cannot be undone | Script has side effects |
| `ssh-add -D` | Removes **all** keys from agent, not just the intended one | Multiple keys loaded |
| `git config --global` | Affects **all** repos on the machine | Intended change is repo-scoped |
| `export VAR=` | Sets empty string — different from `unset VAR` in many tools | Tool checks `[[ -n "$VAR" ]]` |
| `eval "$(cmd)"` | Code injection if `cmd` output contains shell metacharacters | `cmd` reads external input |

### Process / Signals

| Command | Hidden side effect | Dangerous context |
|---------|--------------------|-------------------|
| `kill -9 PID` | No SIGTERM cleanup — can corrupt files mid-write | Process has open file handles or locks |
| `pkill -f pattern` | Matches full **command line**, not just process name — far broader | Pattern is a common substring |
| `killall name` | Kills **all** processes with that name, all users | Multi-process or multi-user system |
| `nohup cmd &` | Persists after logout with no monitoring or signal handling | One-off commands meant to be temporary |

### Make

| Command | Hidden side effect | Dangerous context |
|---------|--------------------|-------------------|
| `make clean` | `CLEANFILES` may include generated files that are slow to recreate | Generated files aren't in version control |
| `make hard-restart` (/path/to/your/project) | Wipes **all** images + volumes, full rebuild from scratch | Any persistent data or long build cache |

### Databases

| Command | Hidden side effect | Dangerous context |
|---------|--------------------|-------------------|
| `TRUNCATE table` | No `WHERE` possible; resets sequences; faster than `DELETE` but equally unrecoverable | Any data you might want back |
| `UPDATE SET col=val` without `WHERE` | Updates **all** rows — parser never warns | Forgetting the WHERE clause |
| `DELETE FROM table` without `WHERE` | Deletes all rows; transaction-safe only if not yet committed | Same |
| `VACUUM FULL` (Postgres) | Exclusive table lock — blocks **all** reads and writes for duration | Actively queried table |
| `DROP TABLE CASCADE` | Also drops dependent views, foreign keys, sequences | Dependent objects exist |

### Package Managers

| Command | Hidden side effect | Dangerous context |
|---------|--------------------|-------------------|
| `npm install` (no args) | Modifies `package-lock.json` — lock drift in CI | CI expects a pinned lockfile |
| `pip install --upgrade pkg` | Can break pinned transitive deps elsewhere in the env | Complex dependency graph |
| `apt-get upgrade` | Can upgrade kernel, restart services, break running programs | Any non-throwaway system |

### Config Artifact Updates — Manual Registries

These registries are **not auto-synced** — adding a file or plugin does not update them. Missing entries cause silent omissions at bundle/install time. Check after every new command or plugin.

| Trigger | Registry to update | File |
|---------|--------------------|------|
| New `~/.claude/skills/*/SKILL.md` created | `_CE_GENERIC_SKILLS` — add the entry | `~/.claude/bin/lib/claude-export/config/defaults.sh` |
| New local plugin or skill created under `plugins/marketplaces/local/` | Review `_CE_EXCLUDES` scope — `"plugins/"` blocks the whole tree | Same file |
| New `~/.claude/bin/` script created that should travel with a bundle | `_CE_GENERIC_SKILLS` or `_CE_ORIGIN_ONLY_FILES` depending on portability | Same file |

**Automated backstop**: `/audit` Agent I surfaces drift between `defaults.sh` and `~/.claude/skills/`. Run after any sprint that creates skills or plugins — it is the canary that catches what manual review misses.
