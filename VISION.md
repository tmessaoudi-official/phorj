# Vision

## The one-sentence version

**Phorge is what PHP might feel like if it were designed today: the same approachable, get-things-done
ergonomics, rebuilt on a statically-typed, immutable-by-default, compiler-checked foundation.**

## Why Phorge exists

PHP made web programming accessible to a generation of developers. Its syntax is familiar, its
feedback loop is short, and it gets out of your way. But it carries decades of dynamic-typing
baggage: errors that only surface at runtime, a type system bolted on after the fact, and surprising
coercions.

Phorge keeps what makes that style of language *pleasant* — readable syntax, string interpolation,
classes, a tiny path from "idea" to "running program" — and puts it on a foundation that catches
mistakes **before** the program runs:

- **Static types, checked up front.** If it type-checks, whole classes of runtime errors are gone.
- **Immutable by default.** State you can reason about. Mutation is opt-in, not the default.
- **No surprises.** Integer overflow and division-by-zero are clean errors, never silent wraparound
  or a crash. Malformed input never panics the toolchain.
- **One language, several targets.** The same program runs on a tree-walking interpreter, a bytecode
  VM, transpiles to real PHP, or compiles to a single standalone native binary — all producing
  identical results.

## Design principles

1. **Correctness is the spine.** Every backend must produce byte-identical output to the reference
   interpreter, enforced by differential testing. A feature that can't be proven equivalent across
   backends doesn't ship.
2. **No silent failure.** The toolchain rejects bad input with a diagnostic, never a panic, hang, or
   out-of-bounds read — including when parsing untrusted binaries.
3. **Earn complexity.** Abstractions (a pluggable backend trait, a tracing GC, an arena allocator)
   are added when a third use case demands them, not speculatively. The Rule of Three is a design
   tool, not a slogan.
4. **Std-only, zero-dependency core.** The runtime links no external crates. The whole language fits
   in one head, builds in seconds, and has no supply-chain surface.
5. **Approachable on the outside, rigorous on the inside.** A beginner should be able to read a Phorge
   program; a compiler engineer should respect how it's built.

## Where this is going

The near-term arc is concrete and already underway (see [ROADMAP.md](ROADMAP.md)): a complete language
core (✅), a fast VM (✅), single-binary distribution across operating systems (🔨), then language
enrichment — indexing, collections, optionals, pipes, exceptions, and mutation with a real garbage
collector.

The longer-term ambition is an **ecosystem**, not just a language: a standard library, real modules
and packages, first-class concurrency (uncolored `spawn` + channels) and a native HTTP server, proper
tooling (LSP, formatter), and — closing the loop with its PHP heritage — a **PHP → Phorge migration
tool** that lets existing codebases move onto a typed foundation incrementally.

The furthest horizon (v2) pushes toward systems programming: native ahead-of-time compilation, an
ownership model that removes the GC, and sized-integer performance.

## What Phorge is not

- It is **not** a PHP runtime or a drop-in PHP replacement. It *transpiles to* PHP and aims to
  eventually *import from* it, but Phorge is its own statically-typed language.
- It is **not** trying to be everything at once. The roadmap is deliberate and ordered; "planned"
  features are planned on purpose, not missing by accident.

If that resonates, contributions and ideas are welcome — see [CONTRIBUTING.md](CONTRIBUTING.md).
