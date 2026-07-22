---
name: pre-commit
description: Use before every git commit — analyses staged changes for blast-radius, produces the four-dimension evidence table (Coverage, Docs, Config, Blast radius) from CLAUDE.md Rule 6, then presents the exact git commit command for manual execution.
user-invocable: true
args: "[--message=<draft-message>]"
---

## --help

> If ARGUMENTS contains `--help`: output the text below verbatim, then immediately STOP — do not execute any other steps. (`--help` takes precedence over all other flags.)
>
> ```
> /pre-commit — Staged-diff gate: blast-radius analysis + four-dimension evidence table + exact commit command.
>
> Usage: /pre-commit [--message=<draft-message>]
>
> Flags:
>   --message=<text>   Seed the commit message with a draft (Claude will refine it)
> ```

---

## Differentiation from related skills

| Skill | Scope | Use when |
|-------|-------|----------|
| `/sweep` | Full codebase, read-only smell review | You want architectural smell detection across all files, no git integration |
| `/pre-commit` | Staged diff only, commit ritual gate | You are about to commit — need blast-radius check + evidence table + commit command |

---

## Side effects

**None** — this skill is read-only. It runs `git diff --staged` and `grep` to analyse staged changes, then displays a report and commit command. It never calls `git commit` and never modifies any file. Output is displayed in conversation only — not persisted to disk; the git commit is the durable artifact.

---

## Step 0 — Precondition checks

**Mandatory task gate**: invoke `ask-human` before Step 1 begins to confirm size and intent. Do not proceed until the user confirms.

**Git checks** (in order; stop at first failure):
1. Verify `git` is installed: `command -v git` — if not found, report `ERROR: git not found in PATH — cannot run /pre-commit` and stop.
2. Verify inside a git repo: `git rev-parse --is-inside-work-tree 2>/dev/null` — if fails, report `ERROR: Not inside a git repository` and stop.
3. Check staged changes exist: `git diff --staged --name-only` — if empty, report `ERROR: No staged changes. Stage files with git add before running /pre-commit.` and stop.
4. Detect active merge/rebase: test for `.git/MERGE_HEAD` and `.git/rebase-merge/` — if either exists, add `WARN: Merge or rebase in progress — evidence table will be produced but commit command will not be shown until the merge/rebase completes.`

---

## Step 1 — Inventory staged changes

Run `git diff --staged --stat` and `git diff --staged --name-only`. For each staged file, record:
- Status: Added / Modified / Deleted / Renamed
- File path and extension
- Lines changed summary

Classify each file:
- **Public interface** — CLI flags, env vars, public functions, documented commands, API contracts, hook behaviour, SKILL.md
- **Internal implementation** — logic, business rules, private helpers
- **Tests** — test files, fixtures, test helpers
- **Config/infra** — Dockerfiles, compose files, Makefile, shell scripts, YAML config
- **Docs** — CLAUDE.md, README, agent definitions, refs/SKILLS.md

---

## Step 2 — Blast-radius analysis

For each file in the **Public interface** or **Config/infra** categories:
1. Extract the changed symbol, flag, function name, or path from the diff
2. Search all references: `grep -r "<symbol>" ~/.claude/ --include="*.md" --include="*.sh" --include="*.json" -l 2>/dev/null`
3. For each hit, read the relevant line and determine if it is a caller, doc reference, or config entry that may need updating
4. Flag any reference NOT already present in the staged diff as a **potential blast-radius item**

If a staged file is a deletion: note that all remaining callers are blast-radius items.

---

## Step 3 — Four-dimension evidence table

Produce the completion gate table from CLAUDE.md Rule 6:

```
| Dimension    | Status | Evidence |
|--------------|--------|---------|
| Coverage     | OK / INCOMPLETE | <test files staged, OR bash -n / exit-code checks if infra, OR "no test suite — N/A with reason"> |
| Docs         | OK / INCOMPLETE | <SKILL.md / README / help text staged, OR "no public interface changed"> |
| Config       | OK / INCOMPLETE | <CLAUDE.md / agent def / SKILLS.md staged, OR "no config impact — <reason>"> |
| Blast radius | OK / INCOMPLETE | <grep hits accounted for, OR list of unresolved references> |
```

**INCOMPLETE** rows block the commit command in Step 5. List exactly what must be staged to resolve each INCOMPLETE row.

Output is displayed in conversation only — not persisted to disk; the git commit is the durable artifact.

---

## Step 4 — Commit message

Parse `--message=<text>` from args if provided. Otherwise derive a draft from the staged diff:
- One imperative-mood subject line (≤72 chars): what changed and the functional reason
- Optionally 1-3 short bullet lines for non-obvious context

**Never append `Co-Authored-By: Claude`** or any Claude attribution (CLAUDE.md Rule 10).

---

## Step 5 — Present commit command

If all four evidence rows are **OK**:

```
All four dimensions satisfied. Run this command:

git commit -m "$(cat <<'EOF'
<commit message here>
EOF
)"
```

If any row is **INCOMPLETE**: list what is missing and do NOT present the commit command. The commit is blocked until all dimensions are resolved — add the missing staged changes and re-run `/pre-commit`.

---

## Error handling summary

| Condition | Behaviour |
|-----------|----------|
| `git` binary not in PATH | ERROR + stop immediately |
| Not inside a git repo | ERROR + stop immediately |
| No staged changes | ERROR + stop immediately |
| Active merge or rebase | WARN — continue to evidence table, suppress commit command |
| `grep` unavailable or returns non-zero | Skip that symbol; note in blast-radius row as "grep unavailable for `<symbol>`" |
| Staged deletion | Note that all remaining callers of the deleted item are blast-radius items |
| Binary file staged | Note in coverage row as "binary file — no diff available; verify manually" |
