# J — Claude global bundle: item-by-item earn-in / earn-out audit

**Developer instruction (2026-07-25):** *"include the claude global bundle (what we should take from it
that will help us! no silent omits, everything must be earned either in or out, either way must be loud
and explicit for me to decide!)"*

**Bundle:** `claude-setup-global-20260722-103235` (re-uploaded 2026-07-25; MANIFEST `generated_at`
2026-07-22T08:32:51Z, `scope: global`, `scrub: 1`). 199 files.

---

## J.0 — Baseline: what is ALREADY installed (so nothing is double-counted)

The bundle's Part-3 import **already happened on 2026-07-22** and is live in the repo.
[Verified: read `scripts/claude-bootstrap/{install.sh,README.md}` + `.claude/settings.json` +
`ls ~/.claude/`]

| Already IN | Where | Mechanism |
|---|---|---|
| Global reasoning framework | `scripts/claude-bootstrap/CLAUDE-global.md` → `~/.claude/CLAUDE.md` | SessionStart hook `scripts/claude-bootstrap/install.sh` (`cp -u`) |
| `THINKING.md` | same dir → `~/.claude/THINKING.md` | same |
| `BLAST-RADIUS.md` | same dir → `~/.claude/BLAST-RADIUS.md` | same |
| 5 skills: `ask-human`, `gaps`, `handoff`, `pre-commit`, `retrospective` | repo-native `.claude/skills/*/SKILL.md` | read in place, no install |

**Prior bulk ruling (recorded in `scripts/claude-bootstrap/README.md`):** *"Deliberately NOT imported
(dev-ruled 2026-07-22): the session-remember memory pipeline …, the permission lists, and the other ~43
machine-specific skills."* — That is exactly the **silent bulk omit** the developer is now re-opening.
This audit re-examines all 43 + every hook, ref, bin script, and the settings template, one by one.

**Verified container facts that drive every verdict below:**
- `~/.claude/hooks/` does **not exist** → every bundle hook is currently absent. [Verified: `ls`]
- `~/.claude/refs/`, `~/.claude/agents/`, `~/.claude/bin/` do **not exist**. [Verified: `ls`]
- Installed framework body is **byte-verbatim** vs the bundle (`diff` = 0 lines) + a 14-line phorj
  adaptation header. So machine-specific sections were **softened by disclaimer, not removed**. [Verified]
- Tooling present: `jq` ✅, `php` ✅, `python3` ✅. **Missing: `yamllint`, `shellcheck`.** [Verified: `command -v`]
- No `advisor()` tool in this environment → the project's DEC-268 reviewer-panel ladder is the
  substitute (already documented in project CLAUDE.md — consistent, not a gap).
- `/loop` is **already** available as a host skill here → importing the bundle's `/loop` is redundant.
- The container ships its OWN harness Stop/SessionStart hooks (`stop-hook-reply-gate.py`,
  `stop-hook-git-check.sh`, `session-start-git-identity.sh`, `user-prompt-submit-reply-reminder.py`)
  → **any imported Stop hook risks double-gating**. [Verified: `ls ~/.claude/`]

---

## J.1 — FINDING J-DANGLE: the installed framework references 7 things that do not exist here

Because the body is verbatim, these pointers dangle. The blanket header ("machine integrations may be
ABSENT and are then optional") covers them *legally* but not *usefully* — a fresh session still reads
instructions it cannot follow. **Severity: P2 (workflow-integrity, not correctness).**

| # | Dangling reference in `~/.claude/CLAUDE.md` | Reality | Fix options |
|---|---|---|---|
| 1 | `~/.claude/refs/SKILLS.md` ("Full list of global slash skills") | `refs/` absent | ship an ADAPTED `refs/SKILLS.md` listing only what is actually present |
| 2 | `expanding-context` skill — Phase 3C says *"invoke `expanding-context` (if available)"* | not installed | import it (see J.2) or delete the reference |
| 3 | `superpowers:test-driven-development` (Rule 7) | plugin absent | header already softens ("apply TDD directly"); optionally prune the reference |
| 4 | `ask-human-question-guard.sh` Stop hook — CLAUDE.md claims *"mechanically caught by"* it | not installed → rule is honor-system, and the claim of mechanical enforcement is **false in this container** | import (J.3) OR reword the claim |
| 5 | `~/.claude/hooks/log-helpers.sh` `log_obs()` (Rule 13 observability) | absent | Rule 13 only applies to hooks we'd author; header covers it |
| 6 | "Memory System Toggles" whole section + `~/.claude/projects/<slug>/memory/` | pipeline absent (dev-ruled OUT) | prune section or keep under header |
| 7 | `~/.claude/run/` sentinels + "statusline line 2" autonomous-mode display | no statusline hook, no `run/` dir | prune, or import statusline (J.3) |

**Recommendation:** ship a **phorj-adapted `refs/SKILLS.md`** (fixes #1 cheaply and truthfully) and
**reword #4** so the framework does not claim enforcement that isn't there. Leave #3/#5/#6/#7 under the
header — pruning the verbatim body creates a maintenance fork against future bundle re-imports, which is
the worse trade. [Grade: Inferred — reasoning from the fork-vs-dangle trade-off, no measurement]

---

## J.2 — The 48 skills, one by one

Judgment criterion: **does it help deliver correct phorj code in an ephemeral remote container?**
(project CLAUDE.md boundary test). "Already IN" = repo-native today.

### Tier A — RECOMMEND IN (high value, zero external deps)

| Skill | Why IN (phorj-specific value) | Notes / adaptation needed |
|---|---|---|
| `/converge` | **Highest-value item in the bundle.** It *is* the convergence loop with a configurable consecutive-clean threshold — precisely the project's **DEC-268 MAXIMAL ladder** (3-lens panel, TWO consecutive clean rounds, cap 5 → ask-human), which is currently hand-rolled from memory at every 3C/6C gate. Importing it makes the mandated gate mechanical and consistent. | Adapt: default to the DEC-268 tier (3 lenses, 2 clean rounds, cap 5); substitute reviewer subagents for `advisor()` |
| `/sweep` | Implements **Phase 6 Second Sweep** (P0–P3 across correctness/regression/security/side-effects/quality) on uncommitted changes — a mandated phase with no tooling today. | Add the phorj "Anti-bandaid gate" + byte-identity dimension |
| `/expanding-context` | Fixes dangling reference J-DANGLE#2; Phase 3C explicitly calls it. 23-dimension silent scan, surfaces only surprises. | Verbatim-ish |
| `/sleuth` | Behavioral bug hunter (logic traps, silent failures, contract violations, edge cases). A language implementation with a VM≡tree-walker≡PHP triple spine is exactly the domain where this pays. | Add a lens: "backend divergence (VM vs TW vs PHP)" |
| `/forge` | Adversarial design critic — inter-module structure, intra-module smells, **cognitive load**. Directly serves the developer's standing ask ("rust quality, naming, structure, documentation must be crystal clear… easy to extend") and Invariant 13 decomposition. | Add Inv-13 (300/500 cap) + Inv-4 (single-sourced kernels) as explicit lenses |
| `/inspect` | 10-agent project health inspector (security, dead code, deprecations, error handling, docs, tests, config, quality, perf, debt). The engine for exactly the review being run tonight. | — |
| `/aggregate-findings` | Cross-stage dedup/synthesis. Mandatory companion once ≥2 of inspect/sleuth/gaps/forge are in — otherwise findings triple-count. | — |
| `/qa-sweep` | Has a **CLI mode** (`--target=cli <binary>`). `phg` *is* a CLI binary with ~20 subcommands and no systematic CLI-surface QA today. Convergence-gated. | Point at `target/release/phg` |
| `/validate-infra` | `bash -n` on shell scripts + YAML lint. The repo has **6 shell scripts + 2 git hooks + 4 GitHub workflows** [Verified: `ls`], and global Rule 7 names `bash -n` as the *required Coverage evidence* for infra changes. | ⚠ `yamllint`/`shellcheck` are MISSING here → skill must degrade loudly to `bash -n` + `python3 -c yaml.safe_load`, not silently skip |
| `/cross-check` (mode B only) | Standalone deep doc validation = **spec-vs-implementation drift detection**, the project's most recurring defect class (Invariant 17 "always-current surfaces"; tonight's findings #5/#7/#14 are all drift). | Drop mode A (Jira) entirely — no Jira here |
| `/recent` | 33 lines, near-zero cost: recent commits + uncommitted + stats. Valuable **after every compaction** (this session compacted twice). | Verbatim |

### Tier B — RECOMMEND IN, lower priority (useful but narrower)

| Skill | Why | Caveat |
|---|---|---|
| `/mega-analysis` | The umbrella pipeline (repair→audit→skill-audit→inspect×2→sleuth×2→gaps×2→vision×2→aggregate). Maps naturally onto the host `Workflow` tool. | Very expensive; several stages (repair/audit/skill-audit) are config-scoped and would need dropping → arguably rebuild a phorj-native `/full-review` instead of importing this |
| `/skill-audit` | 15-dimension per-skill quality audit. Only earns its place **if** we end up with 10+ imported skills. | Circular value; defer until after this import lands |
| `/new-hook` | Hook scaffold with correct exit codes. The repo *does* author hooks (`claude-bootstrap/install.sh`, 2 git hooks). | Marginal — 3 hooks total authored to date |

### Tier C — RECOMMEND OUT (with the explicit reason — no silent omits)

| Skill | Verdict | Reason |
|---|---|---|
| `/ask-human`, `/gaps`, `/handoff`, `/pre-commit`, `/retrospective` | **already IN** | repo-native since 2026-07-22 |
| `/loop` | OUT (redundant) | already a host-provided skill in this container [Verified: available-skills list] |
| `/expand` | OUT (duplicate) | same 23-dimension engine as `/expanding-context`, which is the Phase-3C-referenced one |
| `/audit` | OUT | its subject is the *Claude config* (permissions/hooks/skills). Here that config is 3 files + 5 skills — 11 parallel agents is absurd overkill for that surface |
| `/bundle`, `/install`, `/bootstrap`, `/repair`, `/consolidate`, `/templatize`, `/adapt-project` | OUT | config-portability machinery. These operate *on* Claude configs across machines; the phorj container is a single ephemeral target already served by `claude-bootstrap/install.sh`. `/bundle` is how this tarball was made — it belongs on the dev's machine, not here |
| `/lean`, `/lean-block`, `/lean-unblock` | OUT | "lean mode" toggles a machine-local context-reduction scheme with a blocklist under `~/.claude/`; ephemeral container has nothing to lean |
| `/memory-block`, `/memory-unblock`, `/memory-off`, `/memory-on`, `/memory-readonly`, `/memory-status`, `/memory-promote`, `/sr-health` | OUT | all 8 are controls for the **session-remember pipeline**, itself OUT (J.4). Importing controls for an absent subsystem is pure dead weight |
| `/learning-on`, `/learning-off` | OUT | toggle an output-style *plugin* that isn't installed; both require a restart |
| `/model-audit` | OUT **and actively risky** | it normalizes Claude model IDs across `~/.claude/`. The host system prompt already states the authoritative current model list; a 2026-07-22 snapshot rewriting IDs could *introduce* a contradiction |
| `/pre-session-health` | OUT | 6 signals, of which ~4 concern the memory pipeline + `~/.claude/` git state (absent). `/recent` covers the useful residue |
| `/cleanup` | OUT as-written → **spawns a phorj-native recommendation** | it prunes `/tmp/sr-*`, shell snapshots, and `~/.claude/backups`. The container's *real* disk crisis is `target/debug` at 26 GB (documented in SLICE-STATE: "No space left on device" surfaced as spurious build reds). **Recommend instead: write `scripts/disk-reclaim.sh`** (rm -rf `target/debug/incremental`, `target/release`, stray php-src trees; report `df -h`) — a genuine, evidence-backed gap this skill *pointed at* but does not fill |
| `/skill-extractor` | OUT | mines session history for patterns; session transcripts do not survive the ephemeral container |
| `/command-audit` | OUT | 31 lines, dated stub ("2026-05-26"), audits slash-command definitions |
| `/agent-def` | OUT (for now) | authoring aid for `.claude/agents/*.md`; the repo defines **zero** custom agents. Becomes relevant only if we add phorj-specific agent defs — see J.6 |
| `/consolidate` | OUT | merges N `.claude/` dirs; single-dir situation here |

**Skill tally: 11 Tier-A IN + 3 Tier-B optional IN + 5 already IN + 29 OUT-with-reason = 48.** ✅ all accounted for.

---

## J.3 — Hooks (23 files), one by one

| Hook | Verdict | Reason |
|---|---|---|
| `precompact-handoff.sh` (148) | **STRONG IN** | Writes a handoff *before* context compaction. **This session was compacted twice and lost working state both times** — the single most concrete pain this bundle can fix. Adapt: write to a scratch handoff file + append to SLICE-STATE only on explicit ask (never auto-commit) |
| `ask-human-question-guard.sh` (72) | IN — **but flag the conflict** | Makes the framework's "no bare `?` endings" claim TRUE (fixes J-DANGLE#4). ⚠ The container already runs `stop-hook-reply-gate.py` as a Stop hook; two Stop gates may double-block. **Needs a dev ruling, not a silent install** |
| `ask-human-gate-arm.sh` / `-block.sh` / `-track.sh` (35/83/24) | Optional IN (as a set) | Mechanically enforce the task-categorization gate. Same double-gating caveat; all three or none |
| `log-helpers.sh` (12) | IN **only if** any hook above is imported | hard dependency (`log_obs()`); trivial size |
| `statusline.sh` (523) | OUT | renders `⚠ AUTO-3C` + `▸plan:<topic>` from `~/.claude/run/` sentinels that don't exist here; 523 lines of machine-shaped display logic |
| `session-start-banner.sh` (597) | OUT | banner + plan auto-inherit keyed on machine paths/sentinels; the repo's SLICE-STATE "read FIRST after compaction" header already does the continuity job |
| `advisor-completion-guard.sh` (72) | OUT | guards `advisor()`, which does not exist in this environment |
| `backup-before-write.sh` (54) | OUT | implements Rule 8's *outside-git* backup branch. Everything we touch here is inside git, where `git status` is the check |
| `edit-log.sh` (19) | OUT | append-only edit log; git history is strictly better |
| `session-status.sh` (170), `stop-context-bar.sh` (263), `stop-git-status.sh` (157), `subagent-start-context.sh` (49), `subagent-stop-status.sh` (136) | OUT (5) | informational display hooks; the host harness already provides its own git-status and reply gates |
| `ensure-mcp-health.sh` (31), `patch-playwright-mcp.sh` (27) | OUT (2) | MCP-server lifecycle for the dev's machine clients; irrelevant + those MCP servers aren't here |
| `session-remember/**` (20 files: `save.sh` 305, `start.sh`, `stop.sh`, `trigger.sh`, `consolidate.sh`, `common.sh` 178, `extract.py` 204, 3 prompt templates, 8 test files, `blacklist`) | OUT — **re-confirming the 2026-07-22 ruling, now with the reason stated** | (a) it is an LLM-call-per-session memory pipeline whose store lives in `~/.claude/projects/<slug>/memory/` — **ephemeral here, so it would re-learn and re-forget every session**; (b) the project deliberately uses `docs/plans/SLICE-STATE.md` + the decision register as its continuity mechanism (**Invariant 19**), and a second parallel memory would be a *divergent artifact* — the exact thing Invariant 19 forbids; (c) 8 of the 20 files are its own test suite. **Verdict: OUT, and this is the correct call for a durable reason, not just inertia** |

**Hook tally: 1 strong IN + 4 optional IN (gate set + helper) + 18 OUT = 23.** ✅

---

## J.4 — refs / bin / templates / MCP / misc

| Item | Verdict | Reason |
|---|---|---|
| `refs/SKILLS.md` (103) | **IN, ADAPTED (do not copy verbatim)** | fixes J-DANGLE#1. Must list only skills actually present, or it becomes a second lie |
| `refs/MODELS.md` (46) | OUT — **actively risky** | a 2026-07-22 model-ID snapshot would contradict the host system prompt's authoritative live model list |
| `refs/PREREQUISITES.md` (15) | OUT (fold the useful bit in) | machine prereqs; the one relevant fact (`jq` needed) is verified present, and `yamllint`/`shellcheck` absence is already captured in J.2's `/validate-infra` caveat |
| `refs/SESSION-REMEMBER.md` (22) | OUT | tunable knobs for the OUT pipeline |
| `bin/claude-cleanup.sh` (1091), `lean-swap.sh` (1247), `claude-export.sh` (112), `claude-import.sh` (86), `claude-adapt.sh` (91) + `bin/lib/**` (18 files) + `bin/run-tests.sh` + `bin/tests/**` (6 files, 2 253 lines) | OUT — **all 31 files** | ~4 000 lines of Claude-config lifecycle tooling (export/import/adapt/lean-swap/cleanup) plus its own test suite. Its entire subject is managing configs across machines; the phorj container's needs are met by one 21-line `install.sh`. Importing this would put a second, larger, untested-here toolchain into a language repo |
| `settings.json.template` (504) | **PARTIAL IN — reopens the 2026-07-22 "permissions OUT" ruling; needs a dev decision** | The bulk ruling dropped it as "permission lists", but it also encodes the **deny tier** (force-push, destructive commands) and the **ask tier** + a bash `danger_patterns` firewall — i.e. the mechanical half of the *Destructive & Risky Command Protocol* the framework references. Today repo `.claude/settings.json` has `defaultMode: auto`, a 6-entry allow list, and **no deny list at all** [Verified: read it]. Importing just the deny+ask tiers would harden safety without importing the machine-specific allow entries. **Recommend: import deny+ask only, hand-filtered** |
| `projects/.claude-template/memory/*` (3) | OUT | templates for the OUT pipeline |
| `.claude.json` (379) | OUT | machine MCP/client registry |
| `RTK.md` (29), `RTK-local.md` (25) | OUT | machine-local toolkit the framework itself lists as droppable |
| `plugins-reinstall.sh` (51) | OUT | reinstalls marketplace plugins (incl. `superpowers`); needs network + marketplace auth. Leaves J-DANGLE#3 standing, which the header already covers |
| `.gitignore` (50), `README.md` (210), bundle-root `README.md`/`MANIFEST.json`/`claude-import.sh`/`lib/` | OUT | bundle's own packaging/provenance; provenance is already recorded in `scripts/claude-bootstrap/README.md` |
| `mcp/**` (57 files: desktop-automation MCP server w/ X11/Wayland/Windows drivers, four corporate service client configs + their `.env` files + 2 design/plan docs + uv.locks) | **HARD OUT — security rationale** | (a) zero relevance to a language compiler; (b) contains corporate-tooling artifacts — four service `.env` files and scrubbed `<mcp-client-N>` placeholders (filenames deliberately not restated here: this repo is public, which is the very argument for exclusion). **`tmessaoudi-official/phorj` is a public GitHub repo** — committing corporate MCP configs, even scrubbed, is an information-exposure risk with no upside. Do not import under any option |

**File tally check:** 48 skills + 23 hooks + 4 refs + 31 bin + 3 project-template + 57 mcp + settings.template
+ `.claude.json` + 2 RTK + plugins-reinstall + `.gitignore` + 3 bundle-root/README/MANIFEST + `lib/` + the 3
already-installed md files ≈ **199 files** ✅ every file has an explicit verdict.

---

## J.5 — Recommended import package (the thing to actually build, if approved)

**Shape:** extend `scripts/claude-bootstrap/` — it already exists, is committed, and self-installs via the
SessionStart hook, so this is additive, not new machinery. [Grade: Verified — read `install.sh`]

1. `.claude/skills/` += 11 Tier-A skills (adapted per the table): `converge`, `sweep`,
   `expanding-context`, `sleuth`, `forge`, `inspect`, `aggregate-findings`, `qa-sweep`,
   `validate-infra`, `cross-check`, `recent`. **Repo-native → no install step needed** (Claude Code
   reads `.claude/skills/*` in place; same pattern as the existing 5).
2. `scripts/claude-bootstrap/` += adapted `refs/SKILLS.md`; `install.sh` copies it to `~/.claude/refs/`.
3. `scripts/claude-bootstrap/hooks/` += `precompact-handoff.sh` + `log-helpers.sh`; wire PreCompact in
   `.claude/settings.json`. **(The 4 ask-human/gate Stop hooks are held back pending the double-gating ruling.)**
4. `.claude/settings.json` += hand-filtered **deny + ask** tiers from `settings.json.template`.
5. New `scripts/disk-reclaim.sh` (the `/cleanup` insight, retargeted at the real 26 GB `target/debug` problem).
6. Reword J-DANGLE#4's false "mechanically caught" claim in `CLAUDE-global.md`'s adaptation header.

**Net:** 11 skills + 2 hooks + 2 refs/scripts + a settings hardening — out of 199 bundle files.
**14 in, 185 out, every one named above.**

---

## J.6 — Open questions for the developer (Invariant 15 — I do not rule these)

- **Q-J1** Import the 11 Tier-A skills? (all / subset / none) — recommended: all 11.
- **Q-J2** `/converge` adaptation: hard-code the DEC-268 MAXIMAL tier as its default, or keep it configurable?
  Recommended: hard-code the project tier as default, allow override.
- **Q-J3** The 4 `ask-human`/gate **Stop hooks**: import (mechanical gate enforcement) vs skip (avoid
  double-gating with the container's own `stop-hook-reply-gate.py`)? **Recommended: skip for now**, and
  instead reword the framework's false enforcement claim — cheaper and no conflict risk.
- **Q-J4** `settings.json` **deny+ask tiers** — import hand-filtered? This *reverses* part of the
  2026-07-22 "permissions OUT" ruling. Recommended: yes, deny tier at minimum (there is no deny list today).
- **Q-J5** Build `scripts/disk-reclaim.sh`? Recommended: yes — evidence-backed recurring failure.
- **Q-J6** Tier-B `/mega-analysis`: import, or author a leaner phorj-native `/full-review` that chains
  only the phorj-relevant stages? Recommended: **author phorj-native** — the imported pipeline's
  repair/audit/skill-audit stages are config-scoped dead weight.
- **Q-J7** Add phorj-specific **agent defs** (`.claude/agents/`) — e.g. a `backend-parity-reviewer`
  (VM≡TW≡PHP lens) and an `inv13-decomposer`? That would also earn `/agent-def` in. Recommended: yes,
  after the skill import settles.
- **Q-J8** Prune the verbatim framework body's inapplicable sections (memory toggles, statusline/run
  sentinels), or keep them under the blanket header? Recommended: **keep** — pruning forks the file
  against future bundle re-imports; the header already disclaims them.
