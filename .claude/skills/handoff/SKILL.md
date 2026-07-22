---
name: handoff
spotlight: true
description: Use at the end of a session to save current state so the next session can continue cleanly without losing context about what was done, what is pending, and any non-obvious gotchas.
user-invocable: true
---

## --help

> If ARGUMENTS contains `--help`: output the text below verbatim, then STOP — do not execute any other steps.
>
> ```
> /handoff — Use at the end of a session to save current state so the next session can continue cleanly without losing context about what was done, what is pending, and any non-obvious gotchas.
>
> No flags — invoked without arguments.
> ```

---

Save session state for clean continuation next session.

Write a handoff note so the next session can continue cleanly. Use your knowledge of the current session — you were here. Write in first person ("I").

**Path:** Derive it from the current project directory:
`~/.claude/projects/{slug}/memory/sessions/handoff.md`
where `{slug}` = the project directory with `/` replaced by `-`.
Example: if working in `/path/to/your/project`, the slug is `-stack` and the path is `~/.claude/projects/-stack/memory/sessions/handoff.md`.
Example: if working in `/home/user/myproject`, the slug is `-home-developer-myproject`.

Create the `memory/sessions/` directory if it doesn't exist.

Format:

```
# Handoff

## State
{What's done, what's not. Files modified, decisions made, branch state. 2-4 lines max.}

## Next
{What to pick up. Priority order. 1-3 items.}

## Context
{Non-obvious gotchas, blockers, env state from this session. Skip section entirely if nothing.}

## Memory Updates
{Any user/feedback/project memories worth creating or updating based on this session.
 Format: "- [type] description" (types: user, feedback, project, reference).
 Skip section entirely if nothing new to persist.}
```

Rules:
- Under 25 lines total
- Specific: file paths, branch names, command names, variable names
- Forward-looking — next session doesn't care about the journey, only the current state
- "Memory Updates" is advisory — the next session will see it and decide whether to act
- If nothing meaningful to hand off, write: "No active work."

After writing the file, append `<!-- manual -->` on its own line at the very end. This marker tells the stop hook that a human explicitly saved state — it will skip overwriting with an auto-generated handoff.

Say "Saved." when done — nothing else.
