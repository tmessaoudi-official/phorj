#!/usr/bin/env python3
"""Generate web/examples.js from the single-file, WASM-runnable examples under examples/.

The playground picker/sidebar loads every example here. Selection rules (no silent truncation — the
build logs everything dropped and why):

  * SINGLE-FILE only. `examples/project/**` and multi-file `conformance/**` are cross-package
    projects; the playground editor holds one source and has no multi-file loader, so they cannot
    be entries.
  * WASM-RUNNABLE only. The browser has no filesystem / sockets / process / OS-RNG, so an example
    importing `Core.File`, `Core.Http`, `Core.Process`, `Core.Random`, or `Core.Cryptography` would
    fault at runtime and is skipped. (Every kept example is already proven to run on the interp + VM
    by the differential gate; those are the only backends the WASM build compiles, so the syscall
    import-scan is a sound WASM-safety proxy.)
  * `interop/` and `lift/` are skipped: they depend on external PHP / are lift inputs, not
    standalone runnable showcases.

Output shape: `window.PHORJ_EXAMPLES` is an ORDERED list of {category, name, src} so the UI can group
by category (sidebar section headers). Run from anywhere; paths resolve relative to this script.
"""

import json
import pathlib
import re
import sys

HERE = pathlib.Path(__file__).resolve().parent
EXAMPLES = HERE.parent.parent / "examples"
OUT = HERE / "examples.js"

# Categories skipped wholesale (external-PHP / non-standalone / multi-file).
SKIP_DIRS = {"project", "interop", "lift"}
# Modules whose capabilities the browser WASM sandbox cannot provide → the example would fault.
SYSCALL_IMPORTS = ("Core.File", "Core.Http", "Core.Process", "Core.Random", "Core.Cryptography")

DEFAULT = """package Main;
import Core.Output;

function main(): void {
    List<string> who = ["world", "Phorj"];
    for (string w in who) {
        Output.printLine("Hello, {w}!");
    }
}
"""


def wasm_unsafe(src: str) -> str | None:
    """Return the offending syscall module if `src` imports one, else None."""
    for mod in SYSCALL_IMPORTS:
        if re.search(rf"^\s*import\s+{re.escape(mod)}\s*;", src, re.MULTILINE):
            return mod
    return None


def main() -> int:
    if not EXAMPLES.is_dir():
        print(f"error: examples dir not found: {EXAMPLES}", file=sys.stderr)
        return 1

    # Ordered entries; the default sits first under a "start here" category.
    entries = [{"category": "start here", "name": "hello (default)", "src": DEFAULT}]
    skipped: list[str] = []

    for phg in sorted(EXAMPLES.rglob("*.phg")):
        rel = phg.relative_to(EXAMPLES)
        category = rel.parts[0] if len(rel.parts) > 1 else "top"
        if category in SKIP_DIRS:
            skipped.append(f"{rel} (category `{category}`)")
            continue
        # Multi-file guard: an example nested more than one dir below its category is part of a
        # multi-file layout (e.g. a package tree) — not a single-file entry.
        if len(rel.parts) > 2:
            skipped.append(f"{rel} (multi-file layout)")
            continue
        src = phg.read_text(encoding="utf-8")
        bad = wasm_unsafe(src)
        if bad:
            skipped.append(f"{rel} (imports {bad})")
            continue
        entries.append({"category": category, "name": phg.stem, "src": src})

    # Group by category (contiguous headers in the sidebar). "start here" is pinned first; the rest
    # sort by (category, name). Path-sort alone interleaves top-level files around subdirs, which
    # would split a category into two headers — this keeps each category in one block.
    entries.sort(key=lambda e: (e["category"] != "start here", e["category"], e["name"]))

    body = "window.PHORJ_EXAMPLES = " + json.dumps(entries, indent=2, ensure_ascii=False) + ";\n"
    OUT.write_text(body, encoding="utf-8")

    cats: dict[str, int] = {}
    for e in entries:
        cats[e["category"]] = cats.get(e["category"], 0) + 1
    print(f"wrote {OUT} with {len(entries)} examples across {len(cats)} categories:")
    for c, n in sorted(cats.items()):
        print(f"  {c}: {n}")
    if skipped:
        print(f"skipped {len(skipped)} example(s):")
        for s in skipped:
            print(f"  - {s}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
