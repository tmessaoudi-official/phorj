---
name: ask-human
description: >
  AskUserQuestion protocol — options, recommended first, max 4, yes/no.
user-invocable: true
model: sonnet
allowed-tools: AskUserQuestion
---

## --help

> If ARGUMENTS contains `--help`: output the text below verbatim, then STOP — do not execute any other steps.
>
> ```
> /ask-human — AskUserQuestion protocol — options, recommended first, max 4, yes/no.
>
> No flags — invoked automatically by Claude during the reasoning workflow.
> ```

---

# Human-Friendly Question Protocol

Every question asked to the user MUST follow this protocol. No free-text questions.
No numbered lists. No "Please reply with A, B, or C." Always use `AskUserQuestion`.

---

## Core Rules

| Rule | Requirement |
|------|------------|
| **Always use options** | Never ask a free-text question if options can cover ≥80% of cases |
| **First = Recommended** | The recommended option MUST be first. Label it `"Recommended"` at the end of the label text. |
| **Other is automatic** | `AskUserQuestion` always appends a free-text "Other" option — never add it manually |
| **Batch limit** | Max 4 questions per `AskUserQuestion` call — split larger sets into progressive rounds |
| **Header = chip** | `header` max 12 characters — it shows as a chip above the question |
| **No numbered lists** | Never output "1. Question\n2. Question" — call `AskUserQuestion` instead |

---

## Pattern 1 — Single-Select with Recommendation

Use when the user must pick exactly one option and you have a clear preference.

```
AskUserQuestion({
  questions: [{
    question: "Which implementation approach do you prefer?",
    header: "Approach",
    multiSelect: false,
    options: [
      {
        label: "Pragmatic (Recommended)",
        description: "Balanced approach — good architecture, reasonable scope. Best for most features."
      },
      {
        label: "Minimal",
        description: "Maximum reuse, fewest files changed. Best when deadline is tight."
      },
      {
        label: "Clean",
        description: "Best long-term architecture, most maintainable. Best for core features."
      }
    ]
  }]
})
```

**Rules**:
- Recommended option → first in the array, label ends with `(Recommended)`
- 2–4 options only (Other is auto-added)
- `multiSelect: false` (default — can be omitted)

---

## Pattern 2 — Yes / No Confirmation

Use before any destructive or irreversible action, and before starting implementation after plan approval.

```
AskUserQuestion({
  questions: [{
    question: "Proceed with the implementation plan above?",
    header: "Confirm",
    multiSelect: false,
    options: [
      {
        label: "Yes, proceed (Recommended)",
        description: "Start implementation with the approved plan."
      },
      {
        label: "Ask advisor for review",
        description: "Call advisor() for an independent second opinion before proceeding."
      },
      {
        label: "No, revise",
        description: "Go back and adjust the plan or approach."
      }
    ]
  }]
})
```

**Rules**:
- "Yes" is always first (positive action = recommended default)
- Include an "Ask advisor for review" option for any significant implementation gate, architectural decision, or risky action — position it between Yes and No
- When the user selects "Ask advisor for review", call `advisor()` immediately and present its findings before re-asking
- "No" option label describes what happens next (not just "No")
- Never use this pattern for destructive actions where "No" should be the safe default — invert the order in that case

---

## Pattern 3 — Multi-Select (non-exclusive choices)

Use when multiple answers can coexist (e.g. "which constraints apply?", "which areas to audit?").

```
AskUserQuestion({
  questions: [{
    question: "Which constraints apply to this change?",
    header: "Constraints",
    multiSelect: true,
    options: [
      {
        label: "No breaking changes (Recommended)",
        description: "Existing API contracts must stay intact."
      },
      {
        label: "No new dependencies",
        description: "Do not add any new npm/pip packages."
      },
      {
        label: "Minimal diff",
        description: "Change as few lines as possible."
      },
      {
        label: "Zero behavior change",
        description: "Pure refactor — no observable difference."
      }
    ]
  }]
})
```

**Rules**:
- `multiSelect: true` when choices are not mutually exclusive
- Still list the most common/recommended option first
- Still limited to 4 options (Other is auto-added for free text)

---

## Pattern 4 — Side-by-Side Preview (code or layout comparison)

Use when the user needs to visually compare code snippets, config examples, or UI layouts.

```
AskUserQuestion({
  questions: [{
    question: "Which API response format do you prefer?",
    header: "Format",
    multiSelect: false,
    options: [
      {
        label: "Flat (Recommended)",
        description: "Simpler to consume on the frontend.",
        preview: "{\n  \"id\": 1,\n  \"status\": \"pending\",\n  \"amount\": 100\n}"
      },
      {
        label: "Nested",
        description: "Groups related fields together.",
        preview: "{\n  \"claim\": {\n    \"id\": 1,\n    \"status\": \"pending\"\n  },\n  \"payment\": {\n    \"amount\": 100\n  }\n}"
      }
    ]
  }]
})
```

**Rules**:
- Only use `preview` when there is a concrete artifact to compare (code, layout, config)
- Do NOT use `preview` for simple preference questions
- `preview` is only supported for single-select (not multiSelect)
- Preview content renders as markdown in a monospace box

---

## Pattern 5 — Progressive Batching (max 4 per call)

When you have more than 4 questions, split into logical rounds. Do NOT dump all questions at once.

```
# Round 1 — Scope and context (most critical first)
AskUserQuestion({
  questions: [
    { question: "Which module is affected?", header: "Module", ... },
    { question: "What is the expected outcome?", header: "Output", ... },
    { question: "Any constraints?", header: "Constraints", ... }
  ]
})

# Wait for answers, then Round 2 — only if needed
AskUserQuestion({
  questions: [
    { question: "Should we create a feature branch?", header: "Branch", ... }
  ]
})
```

**Rules**:
- Group by logical theme (scope → output → constraints → details)
- Ask the most decision-critical questions first
- Only ask follow-up questions if answers from Round 1 reveal new ambiguities
- Never ask questions you could answer by reading the codebase — explore first

---

## Pattern 6 — Stating Ambiguity (for sub-agents without AskUserQuestion)

Sub-agents (e.g. architect) cannot use `AskUserQuestion`. When ambiguity exists, they state it inline with a default assumption. The supervisor then surfaces it to the user using Pattern 1 or 2.

**Sub-agent output format**:
```markdown
> ⚠️ **Assumption**: The request mentions "status field" but does not specify
> the source. I assumed it comes from the Mario API response.
> **Default**: Use `claim.status` from the existing Mario payload.
> **Alternative**: Fetch status from a dedicated `/status` endpoint.
> Supervisor should confirm with user before implementation begins.
```

**Supervisor then asks**:
```
AskUserQuestion({
  questions: [{
    question: "The architect assumed status comes from the Mario API payload. Is that correct?",
    header: "Assumption",
    multiSelect: false,
    options: [
      { label: "Yes, use Mario payload (Recommended)", description: "claim.status from existing payload." },
      { label: "No, use dedicated endpoint", description: "Fetch from a separate /status route." }
    ]
  }]
})
```

---

## Navigation (automatic — no configuration needed)

The Claude Code UI handles all navigation automatically:
- **Arrow keys** → move between options
- **Enter** → confirm selection
- **Space** → toggle selection (multiSelect mode)
- **First option** → pre-selected by default when the UI opens

You do not need to configure this — just ensure the recommended option is always first.

---

## Anti-patterns — Never Do These

| Forbidden | Why | Use instead |
|-----------|-----|-------------|
| `"Please reply with A, B, or C"` | Not interactive, error-prone | `AskUserQuestion` with options |
| Numbered question lists in prose | Non-interactive, user must type | `AskUserQuestion` with up to 4 questions |
| `"Other"` as a manual option | Auto-added by `AskUserQuestion` | Remove it — it's always there |
| Recommended option last | User defaults to first | Put recommended first |
| 5+ options per question | Overloads the user | Max 4 options + auto Other |
| Asking questions you can answer by reading files | Wastes user time | Read files first, then ask only what files can't answer |
| Asking confirmation after a destructive action | Too late | Always ask BEFORE destructive actions |

---

## Quick Reference

```
Single choice + recommendation   → Pattern 1 (multiSelect: false, first = recommended)
Yes/No gate                       → Pattern 2 (Yes first, advisor option second, describe No action)
Multiple constraints/preferences  → Pattern 3 (multiSelect: true)
Code/layout comparison            → Pattern 4 (preview on options)
More than 4 questions             → Pattern 5 (progressive rounds)
Sub-agent ambiguity               → Pattern 6 (state assumption, supervisor asks)

Advisor option                    → Include "Ask advisor for review" between Yes and No on any
                                    significant gate; call advisor() when user selects it
```

$ARGUMENTS
