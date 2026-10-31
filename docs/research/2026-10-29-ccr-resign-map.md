# Cloud branch re-sign map — `ccr-56056e44-aec471` → `master` (landed `b8f383ba`, 2026-10-29)

`land.sh` re-signed the cloud branch's 37 commits onto `master`, so every hash the landed docs cite (plans, SLICE-STATE,
MASTER-PLAN, the cloud-kit plan) names the **old** commit, which exists only on `origin/ccr-56056e44-aec471`. This table
resolves each old hash to its `master` copy. Built 2026-10-31 by `git patch-id --stable` over `master..origin/ccr-56056e44-aec471`
(37) against `d1e26e68..master` (40): all 37 matched by patch-id, none by fallback; the 3 `master`-only commits are the PC
commits `a48276f7`, `d75f1cc4`, `f5624849` (2026-10-08/09) that turned the planned fast-forward into a rebase.

With this map in the repo, `origin/ccr-56056e44-aec471` is no longer the only place the cited hashes resolve; deleting it
stays the developer's call.

| old (cloud branch) | new (master) | subject |
|---|---|---|
| `7abca638` | `e0924513` | docs: cloud takeover audit, and DEC-569 — cloud sessions commit only to their own branch, never master |
| `f1ba2f07` | `05a81c94` | docs: cloud audit follow-ups — import caveats for the global export, re-derived counts, SLICE-STATE pointer to DEC-569 |
| `34035ea2` | `114ad488` | ci: run the CI gate on ccr-* cloud-session branches (DEC-569) |
| `8d058a55` | `2a1c65ff` | feat(tools): cloud kit — the developer's Claude framework and memories, encrypted, installed only in claude.ai cloud sessions (DEC-570) |
| `8f218731` | `6da64ac8` | docs: cloud kit 6C follow-ups — CLAUDE.md routing names the DEC-570 exception, plan rows closed, gawk and first-session checks in the README |
| `d15a7808` | `be1dcc9f` | fix(transpile): parenthesize a `with` result used as a receiver, so `p with { x = 5 }.d()` is valid PHP (row 4b step 1, TRANSPILE-WITH-CALL) |
| `52c62302` | `8b76eca4` | fix(vm): `with` and property hooks follow the runtime class when the compile-time class does not decide them (row 4b step 2, VM-WITH-STATIC-CLASS) |
| `04c66747` | `bcab642f` | test(chunk): pin size_of::<Op>() at 32 bytes, so a new variant cannot silently grow every VM instruction (row 4b step 2 follow-up) |
| `2c377b52` | `069924fa` | fix(vm): row 4b step 2 follow-ups — cache the runtime hook lookup, and stop asserting a disputed PHP set-hook shape (DEC-571 PENDING) |
| `5ae9b306` | `cb87aa0c` | feat(tools): the encrypted cloud kit and the first memory delta, to the developer's age public key (DEC-570 plan step 4) |
| `3b61324c` | `098d72d9` | fix(checker): a trait method's bare name is a field read when a using class stores that field (row 4b step 3, TRAIT-BARE-NAME) |
| `d9cfe8e1` | `91f82095` | fix(vm,checker): row 4b close-panel round 1 — exact-only compile-time hooks, a per-subtree gate, the enum `.value` P0 |
| `52d78f86` | `0c976cec` | docs: close plan row 4b (evidence d9cfe8e1) and cloud-kit plan row 4 (5ae9b306) |
| `0cc92794` | `68ee3eec` | docs: row 4b 6C follow-ups — MASTER-PLAN states the merge as pending, two ASSUMED entries, encrypted memory delta |
| `7ed814e7` | `de1f7a4b` | docs(plan): give the cloud-kit row 4 evidence its commit subject, so doc-guards G3 can recover it |
| `f6a749f4` | `68c944aa` | fix(vm,checker): row 4b close-panel round 2 — enum `.value` behind an untyped receiver, the write example, the JIT cliff disclosed |
| `dfb5ccc4` | `5758fa3e` | perf(jit): row 4b step 5 — the unboxed JIT lowers GetFieldOrHook/SetFieldOrHook per site, closing the ~57x/~80x cliffs |
| `6b8c135d` | `60ab2770` | perf(jit): row 4b step 5b — the unboxed JIT lowers CloneInstance, closing the ~38x `with` cliff |
| `3759abd9` | `a2c2f549` | test(jit): row 4b step 5b — pin the clone of an owned temp receiver, a wide class and a list field |
| `2e35ff2c` | `21148505` | fix(compiler,checker): row 4b close-panel round 3 — untyped enum `.value`, the trait hint, take-scan proof, rulings recorded |
| `cc82b710` | `9d745520` | fix(vm): row 4b close-panel round 4 — a backed enum's untyped `.value` beside a `value` field or method |
| `2a3da869` | `49f1dbb0` | docs(slice-state): stamp the round-4 commit sha `cc82b710` into the row 4b cursor |
| `f809c795` | `2b791d68` | chore(cloud-kit): encrypted memory delta — row 4b close-panel rounds 3–4 notes |
| `12fe3107` | `d27814bb` | test: row 4b close-panel round 5 — pin the untyped `?.value` on its two other lowerings; DEC-571..573 mirror rows |
| `ce75df4f` | `2c33fb0f` | docs: close plan row 4b (evidence `12fe3107`) — five close-panel rounds, the round-5 cap ruled; the cursor moves to row 4c |
| `602a581c` | `7b432eac` | fix(transpile): a decomposed parent's static method is static in its interface (row 4c item 3) |
| `3feafab6` | `e8ec622f` | fix: an inherited static field is the declaring class's one slot on every leg (row 4c item 2) |
| `7e0dd068` | `133b9bdf` | fix(checker): a subclass of a trait-using class inherits what its parent composed (row 4c item 5) |
| `5ebfd0ad` | `8e58351a` | fix: a trait's computed static runs once per using class, in one order on every leg (row 4c item 1) |
| `280a626a` | `05903807` | docs: end-of-day resume point — row 4c items 1, 2, 3 and 5 built, item 4 traced; cloud-kit memory replay queued; encrypted memory delta |
| `d2ed39a3` | `90ec0b06` | fix: field initializers from traits and multiple parents run on every leg (row 4c item 4) |
| `a8a874f7` | `361e9325` | fix: row 4c close-panel round 1 — a diamond's static owner in the checker's order, traits-first statics, DEC-574 |
| `216af26c` | `909a5045` | fix: row 4c close round 2 — static method in a decomposed parent, LSP trait hover, CI diamond case |
| `b4b4ad6e` | `1ed23f83` | docs: row 4c close round 2 on frozen 216af26c — DECOMPOSED-PARENT-TRAITS recorded, row stays doing |
| `600dfd80` | `a378d786` | docs: cloud and PC readiness report — a fresh cloud session gets no kit, no key, PHP 8.3; sign-and-merge steps |
| `ca73d957` | `9ba2cd6b` | docs(readiness): the merge carries 36 commits, counting the report and this fix |
| `c314682c` | `b8f383ba` | docs(readiness): G3 is textual (verified), check local master equals origin/master, unset vs failing setup |
