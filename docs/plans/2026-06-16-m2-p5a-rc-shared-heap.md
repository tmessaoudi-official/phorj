# M2 P5 Phase A — `Rc`-shared heap objects

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:executing-plans (inline; subagents
> deadlock on the ask-human gate in this repo). Steps use checkbox (`- [ ]`) syntax. Phorge git
> autonomy applies (commit green, self-contained). Read `docs/INVARIANTS.md` and the design spec
> `docs/specs/2026-06-16-m2-p5-object-model-design.md` before touching the backends.

**Goal.** Make compound heap objects *shared* instead of *deep-cloned*: wrap `Instance`, `EnumVal`,
and list payloads in `Rc`, so `Op::GetLocal`'s clone (and every interpreter var-read) becomes an
O(1) refcount bump instead of a deep `HashMap`/`Vec` copy. Reclamation stays automatic via `Drop`
and is provably correct (the M1 heap is immutable + acyclic — no `Rc` cycle can leak; design §3).
**Behavior is unchanged** — this is a pure perf refactor gated by the differential harness, with a
before/after `phorge bench` as the "did it help" evidence.

**Scope.** `src/value.rs` (the `Value` variants + the `Box`→`Rc` swap) and the construct/extract
sites in **both** backends. **No** `Op` set / bytecode-format / AST / checker change. The slab arena
and the slot-indexed field layout are out (the latter is the bench-gated Phase B; design §4).

## Representation

- `Value::Instance(Rc<Instance>)` (was `Box<Instance>`)
- `Value::Enum(Rc<EnumVal>)` (was `Box<EnumVal>`)
- `Value::List(Rc<Vec<Value>>)` (was `Vec<Value>`) — `Rc<Vec>` chosen over `Rc<[Value]>` for
  construction simplicity (`Rc::new(vec)`); revisit only if it matters. `Map`/`Set`/`Str` are left
  as-is (not stressed by the bench; trivial follow-on if ever needed).

`#[derive(Clone)]` on `Value` still holds (`Rc: Clone`); `eq_val`/`as_display`/`type_name` match by
reference and auto-deref through `Rc`, so their bodies need **no** change.

## Construct sites (→ `Rc::new`)

- `src/vm.rs`: `MakeEnum` (~295), `MakeInstance` (~330).
- `src/interpreter.rs`: list literal (~251), enum construct (~371), instance construct (~430/454/456
  — fold the `inst.clone()` double-build into one `Rc`: build `let rc = Rc::new(inst);` once, share
  `rc.clone()` for `this`, return `Value::Instance(rc)`).

## Extract sites needing more than a type swap (can't move out of an `Rc`)

- `src/vm.rs` `GetEnumField` (~308): `ev.payload.into_iter().nth(i)` → `ev.payload.get(i).cloned()`.
- `src/interpreter.rs` `For` (~214): `Value::List(items) => items; for item in items` →
  iterate `items.iter()` and `declare(name, item.clone())`.
- `src/vm.rs` `Index` (~237) / `GetField` (~338): already clone the element via deref — confirm they
  still compile unchanged under `Rc` (auto-deref), adjust only if the borrow checker complains.

All other sites (`MatchTag`, `match_pattern`, field reads, `eq_val`) are read-only/auto-deref → no
change.

## Phasing — one TDD-safe, parity-gated, bench-measured commit

- [ ] **A0 (baseline bench):** record the *current* object-heavy number — `phorge bench
      /tmp/w4/bench_obj.phg` (and `bench_scalar.phg`) — into the commit message / CHANGELOG. (Today:
      object VM 1537 ms, 4.73×; scalar VM 156 ms, 11.57×.)
- [ ] **A1:** `src/value.rs` — swap the three `Value` variants to `Rc<…>`; add `use std::rc::Rc`.
      Confirm `eq_val`/`as_display`/`type_name`/the `value.rs` unit tests compile + pass unchanged.
- [ ] **A2:** fix the construct sites (`Rc::new`) and the three move-out extract sites in `vm.rs` +
      `interpreter.rs` (list-for, enum-field, ctor double-build). `cargo build` clean.
- [ ] **A3:** `cargo test` green (244), incl. the full differential suite + examples sweep
      (byte-identical parity is the safety net for this behavior-preserving refactor).
      `cargo clippy --all-targets` + `cargo fmt --check` clean.
- [ ] **A4 (after bench):** record the *new* object-heavy number; compute the speedup. Update
      `CHANGELOG.md` (P5 Phase A entry with before/after) and `CLAUDE.md`/`docs/INVARIANTS.md` (heap
      objects are now `Rc`-shared; GC still deferred to M3). Commit `perf(vm): Rc-share heap objects
      — refcount instead of deep-clone-on-load (M2 P5a)`.
- [ ] **A5 (decide Phase B):** from the A4 bench, decide whether field access still dominates →
      whether to open the bench-gated Phase B (slot-indexed `Vec` fields). Record the call.

## Acceptance criteria

- Full suite green (244), differential + examples byte-identical, clippy + fmt clean,
  `#![forbid(unsafe_code)]` intact.
- A measured object-heavy bench improvement (before/after recorded); no regression on the scalar
  bench beyond noise.
- No `Op`/bytecode/AST/checker change; `Box`→`Rc` confined to `value.rs` + enumerated sites.

## Risks & rollback

- **Risk — borrow-checker friction at move-out sites.** Mitigation: the three are enumerated above;
  each has a known `.cloned()`/`.iter()` fix. **Risk — a missed construct site.** Mitigation: the
  type swap makes any missed `Box::new` a *compile error*, not a silent bug.
- **Rollback:** single isolated commit; `git revert` restores the value-native state.

## Decisions Log

- [2026-06-16] AGREED: P5 Phase A = `Rc`-wrap `Instance`/`Enum`/`List`; behavior-preserving, gated by
  the differential harness, measured by `phorge bench`. (Design: `docs/specs/2026-06-16-m2-p5-object-model-design.md`.)
- [2026-06-16] AGREED: `Value::List` becomes `Rc<Vec<Value>>` (not `Rc<[Value]>`) for construction
  simplicity; `Map`/`Set`/`Str` left unchanged (not bench-stressed).
- [2026-06-16] AGREED: Phase B (slot-indexed field layout) is **bench-gated** on A4 — not started
  without evidence that field access still dominates.
