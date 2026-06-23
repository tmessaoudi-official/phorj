# Front-End + Registry Whale Map — Decomposition Research

Scope: `src/{parser,native,ast,lexer,loader,cli}.rs`. Behavior-preserving cohesion split only.
NO OOP/SOLID/visitor. Exhaustiveness sacred. Native registry index stability is a HARD invariant.

Line counts (code vs `#[cfg(test)]`):

| File | Total | Code | Tests (`mod tests`) | Notes |
|------|------|------|--------|-------|
| parser.rs | 3328 | ~1–1932 | 1933–3328 (~1396, **42%**) | tests dominate the file |
| native.rs | 2053 | ~1–1378 | 1379–2053 (~675) | registry whale |
| ast.rs | 1654 | ~1–1463 | 1464–1654 (~190) | data + helper fns |
| lexer.rs | 999 | ~1–620 | 621–999 (~379) | single-pass scanner |
| loader.rs | 1544 | ~1–1218 | 1219–1544 (~325) | multi-pass resolver |
| cli.rs | 1783 | ~1–1356 | 1357–1783 (~426) | command surface + explain text |

**Cross-cutting observation:** a large fraction of every "whale" is test code in an inline
`mod tests`. The single cheapest, lowest-risk win across ALL six files is to extract
`#[cfg(test)] mod tests` into a sibling `<file>_tests.rs` (via `#[path]` or `include!`) or a
`tests/` submodule. This alone takes parser 3328→~1932, native 2053→~1378, cli 1783→~1356, etc.,
with ZERO behavioral risk (tests are not part of the byte-identity spine). Recommended as Wave 0
of any front-end decomposition.

---

## 1. parser.rs (3328) — recursive-descent + Pratt

### Structural map
- L15  `fn stamp_visibility(Item, Visibility) -> Item` — exhaustive `match item` over `Item` (4 vis-carrying arms + `other`). Free fn.
- L37  `struct Parser { tokens, pos, depth }` — the shared mutable state.
- L46  `impl Parser { … }` — ONE 1874-line impl block (L46–1920). All parsing.
- L1920 `fn compound_op(&TokenKind) -> Option<BinaryOp>` — free fn, compound-assign desugar table.
- L1933 `mod tests` — 1396 lines.

### Parser methods grouped by cohesion (line ranges)
**Cursor/util core (must stay shared):** `new`47, `peek`57, `peek_span`62, `advance`67,
`check`77, `eat`82, `expect`92, `error`101, `expect_ident`778.

**Expressions (Pratt + postfix + primary):** `parse_expr`112, `parse_range`119,
`infix_op`139 (precedence table), `parse_binary`164 (Pratt core), `parse_unary`226
(nest-depth guard), `parse_postfix`258, `parse_arg_list`349, `parse_primary`367,
`parse_match`723, `parse_if_expr`754. (~620 lines)

**Types:** `parse_type`495, `parse_type_intersection`512, `parse_type_atom`528,
`parse_type_params`1457. (~150 lines)

**Strings:** `split_interpolation`589 — shared by Str/Html primary arms (~63 lines).

**Patterns:** `parse_pattern`652 (~71 lines).

**Statements:** `parse_stmt`789 (dispatch), `parse_throw`822, `parse_try`833,
`parse_var_inferred`871, `parse_mutable_var_decl`889, `parse_block`910, `parse_return`921,
`parse_if`934, `parse_for`973, `for_header_is_classic`998, `parse_cfor_rest`1023,
`parse_for_clause_stmt`1055, `parse_while`1109, `parse_do_while`1145,
`parse_var_decl_or_expr_stmt`1165, `finish_assign_or_expr`1190, `try_var_decl_header`1238.
(~450 lines)

**Items / declarations:** `parse_item`1255 (dispatch), `parse_decl_visibility`1317,
`parse_program`1337, `parse_package`1356, `parse_import`1371, `parse_type_alias`1394,
`parse_function`1405, `parse_params`1472, `parse_enum`1493, `parse_class`1532,
`parse_use_traits`1610, `parse_trait`1629, `parse_resolution`1647, `parse_interface`1689,
`parse_name_list`1747, `parse_class_member`1757, `parse_property_hook`1811,
`parse_modifiers`1864, `parse_ctor_params`1890. (~660 lines)

### Candidate sub-modules
Pratt/precedence vs item parsing is the natural seam (matches the prompt). The clean
constraint is that ALL methods live on the single `Parser` struct and share `&mut self`
state (`pos`, `depth`). Rust supports splitting one inherent impl across files via multiple
`impl Parser` blocks — so each sub-module is `impl Parser { … }` over the SAME struct, no
new type, no dispatcher, no trait. This is behavior-preserving by construction.

Proposed `parser/` module (struct + core util stay in `parser/mod.rs`):

| File | Methods | ~Lines |
|------|---------|--------|
| `parser/mod.rs` | struct, cursor util, `parse_program`/`parse_expr`/`parse_stmt`/`parse_item` entry points, `stamp_visibility`, `compound_op` | ~250 |
| `parser/exprs.rs` | Pratt: `parse_range`/`infix_op`/`parse_binary`/`parse_unary`/`parse_postfix`/`parse_arg_list`/`parse_primary`/`parse_match`/`parse_if_expr`/`split_interpolation` | ~680 |
| `parser/types.rs` | `parse_type`/`_intersection`/`_atom`/`parse_type_params` | ~150 |
| `parser/patterns.rs` | `parse_pattern` | ~75 |
| `parser/stmts.rs` | all `parse_*` statement methods + `try_var_decl_header`/`finish_assign_or_expr` | ~450 |
| `parser/items.rs` | all top-level declaration parsers (function/class/enum/interface/trait/import/package/members/hooks/ctor params/modifiers) | ~660 |

Tests → `parser/tests.rs` (or keep per-submodule). **Cleanest win:** items.rs + stmts.rs +
exprs.rs are large, cohesive, and have minimal interdependence beyond the shared cursor.

### Exhaustive matches in parser.rs
- `stamp_visibility` L15 — `match item` over **all `Item` variants** (4 + catch-all). Adding an
  `Item` variant should be reviewed here (catch-all `other` makes it non-failing but possibly wrong).
- `parse_primary` L367 — `match self.peek().clone()` over `TokenKind` (literals/keywords/
  delimiters) with `_ => Err`. Token-dispatch; NOT exhaustive (has `_`), so adding a TokenKind
  won't break compilation — review needed when adding expression-leading tokens.
- `parse_stmt` L789, `parse_item` L1255, `parse_class_member` L1757, `parse_decl_visibility`
  L1317 — all `match self.peek()` dispatch tables with `_` arms.
- `infix_op` L139 / `compound_op` L1920 — `Option`-returning operator tables (not exhaustive).
- **Risk:** none of the parser dispatch matches are compiler-exhaustive (all have `_`), so the
  compiler will NOT flag a forgotten token. These are "soft" exhaustive sites — splitting them
  into files makes the set HARDER to audit when adding a token. Mitigation: keep all
  TokenKind-dispatch entry points (`parse_primary`, `parse_stmt`, `parse_item`) co-located or
  cross-referenced in `parser/mod.rs` docs.

### Coupling / risk
- Single `&mut self` cursor state shared by every method → any split is multi-`impl` over one
  struct (no API change). Low risk.
- `depth` nest-guard invariant: `parse_unary` is "the one function every nesting vector passes
  through" (see comment L40). If exprs move to `parser/exprs.rs`, that invariant stays intact
  (same method, same struct).
- `split_interpolation` is shared by two primary arms (Str + Html) — keep with exprs.

---

## 2. native.rs (2053) — the (module,name) registry

### Structural map
- L22 `struct NativeFn { module, name, params, ret, eval: NativeEval, php }` — registry record.
- L54 `enum NativeEval { Pure(fn), HigherOrder(fn) }` — `#[derive(Clone, Copy)]`.
- L67 **`pub const CONSOLE_PRINTLN: usize = 0`** — pinned slot, baked into `Op::CallNative`.
- Per-module native bodies + a `*_natives() -> Vec<NativeFn>` builder each:
  - console: `console_println`73; (helper `parg`91)
  - math: bodies 102–149, `math_natives`150
  - text: bodies 217–304, `text_natives`305
  - file: bodies 402–428, `file_natives`429
  - bytes: bodies 472–539, `bytes_natives`540
  - html: helpers + `tag_el!`755 / `tag_void!`796 macros, bodies 642–827, `html_natives`828
  - list: bodies 983–1078 (incl. HigherOrder map/filter/reduce 1030–1078), `list_natives`1079
  - map: bodies 1146–1180, `map_natives`1181
  - set: bodies 1231–1257, `set_natives`1258
- L1293 `fn build() -> Vec<NativeFn>` — **the ordering authority**: pushes Console.println first,
  then `extend`s each `*_natives()` in fixed order, then asserts `registry[CONSOLE_PRINTLN]`.
- L1327 `pub fn registry() -> &'static [NativeFn]` — `OnceLock` singleton.
- L1334 `index_of(module,name)` / L1346 `index_of_by_leaf(leaf,name)` / L1356 `import_map(items)`.

### Candidate sub-modules
Each stdlib module is already a self-contained block (bodies + a `*_natives()` factory). This
splits VERY cleanly per the prompt's suggestion:

| File | Contents | ~Lines |
|------|----------|--------|
| `native/mod.rs` | `NativeFn`, `NativeEval`, `ClosureInvoker`, `CONSOLE_PRINTLN`, `build`, `registry`, `index_of*`, `import_map`, `parg` | ~220 |
| `native/console.rs` | `console_println` + factory | ~30 |
| `native/math.rs` | bodies + `math_natives` | ~115 |
| `native/text.rs` | bodies + `text_natives` | ~190 |
| `native/file.rs` | bodies + `file_natives` | ~70 |
| `native/bytes.rs` | bodies + `bytes_natives` | ~130 |
| `native/html.rs` | helpers + `tag_el!`/`tag_void!` macros + `html_natives` | ~340 |
| `native/list.rs` | pure + HigherOrder bodies + `list_natives` | ~165 |
| `native/map.rs` | bodies + `map_natives` | ~85 |
| `native/set.rs` | bodies + `set_natives` | ~85 |

Each `native/<mod>.rs` exposes only `pub(crate) fn <mod>_natives() -> Vec<NativeFn>`; the bodies
stay private to their file. `build()` in `mod.rs` keeps the canonical push order. This is the
single cleanest "per stdlib module" split in the whole front-end.

### HARD INVARIANT — registry index stability (do not violate)
- **Slot order is load-bearing.** The compiler bakes a `usize` index into `Op::CallNative(idx,
  argc)`; that index is `registry()`'s position. The index is produced at compile time by
  `index_of` / `index_of_by_leaf` and consumed at run time by the VM. As long as a program is
  compiled and run by the SAME build, the absolute index value doesn't need to be a fixed number
  **except for `CONSOLE_PRINTLN = 0`**, which is a hard-coded constant the compiler emits
  directly (migrated `Op::Print`). `build()` self-asserts this slot.
- **Therefore the split MUST preserve `build()`'s push order**: `Console.println` FIRST (so it
  lands at index 0), then `math, text, file, bytes, html, list, map, set` in exactly the current
  order. Re-ordering the `registry.extend(...)` calls, or moving `console_println` out of the
  first explicit `vec![...]`, breaks `CONSOLE_PRINTLN` (caught by the assert) AND silently shifts
  every other native's index. If any future code pins another constant (none today besides
  `CONSOLE_PRINTLN`), that slot's relative position must be frozen too.
- **Per-file split does NOT threaten index stability** as long as `build()` remains the single
  ordering coordinator in `mod.rs`. The bodies' file location is irrelevant to the index; only the
  `vec![console] + extend(math) + extend(text) + …` sequence is. Keep `build()` and `CONSOLE_PRINTLN`
  together and untouched.
- The `NativeEval` `Copy` invariant (comment L63: "a `CallNative` dispatch reads it by value,
  ending the registry borrow before the invoker captures the backend") must survive — it's a
  derive on the enum in `mod.rs`, unaffected by splitting bodies out.

### Exhaustive matches
None in native.rs — `NativeEval` is matched at the call sites (interpreter/VM `exec_op`), not
here. The registry is a flat `Vec`; no exhaustive `match` to preserve. Lowest-risk file.

---

## 3. ast.rs (1654) — data + AST-walking helpers

### Structural map
Pure data (enums/structs), L10–1462, interleaved with several non-trivial free helper fns:
- Types/exprs: `enum Type`10, `enum Pattern`51, `enum StrPart`84, `struct MatchArm`90,
  `enum UnaryOp`97, `enum BinaryOp`103, `enum Expr`124 (**24 variants**), `enum LambdaBody`252.
- **Helper fns (the non-data part):**
  - `free_vars`268 — exhaustive `match body` over `LambdaBody`.
  - `class_implements`293, `class_supertypes`354, `instanceof_table`395, `class_mro`420 —
    program-walking interface/inheritance table builders (BTreeMap, sorted, cycle-safe).
  - `class_method_origins`467, `class_field_conflicts`663 — trait/inheritance resolution helpers.
  - `ctor_plan`789 — constructor resolution (trait/parent precedence).
  - `collect_free_expr`830, `collect_free_block`926, `collect_free_stmt`936,
    `collect_pattern_bindings`1038 — the recursive free-variable walkers (exhaustive matches over
    Expr/Stmt/Pattern).
- Declaration structs (L1061–1462): `Param`, `Modifier`, `Visibility`, `CtorParam`, `Stmt`
  (1119), `CatchClause`, `FunctionDecl`, `EnumVariant`, `EnumDecl`, `ClassMember`(1267),
  `ClassDecl`(1307), `UseTrait`, `TraitDecl`, `Resolution`, `InterfaceDecl`, `Item`(1423),
  `Program`(1454).
- L1464 `mod tests`.

### Candidate sub-modules
ast.rs is **mostly pure data** but carries a meaningful helper layer. The data is highly
interlinked (Expr references Type/Pattern/StrPart; Stmt references Expr; Item references all
decls) but Rust modules re-export freely, so splitting data into `ast/{expr,stmt,item,ty}.rs`
with a `pub use` re-export in `ast/mod.rs` is behavior-preserving (consumers `use crate::ast::*`).

| File | Contents | ~Lines |
|------|----------|--------|
| `ast/mod.rs` | `pub use` re-exports; small shared enums (`UnaryOp`, `BinaryOp`, `StrPart`, `Modifier`, `Visibility`) | ~120 |
| `ast/ty.rs` | `enum Type` | ~45 |
| `ast/expr.rs` | `enum Expr`, `enum LambdaBody`, `struct MatchArm`, `enum Pattern` | ~250 |
| `ast/stmt.rs` | `enum Stmt`, `struct CatchClause`, `struct Param`, `struct CtorParam` | ~200 |
| `ast/item.rs` | `Item`, `Program`, `FunctionDecl`, `EnumDecl/Variant`, `ClassDecl`, `ClassMember`, `Interface/Trait/UseTrait/Resolution` | ~430 |
| `ast/helpers.rs` (or `ast/walk.rs` + `ast/classes.rs`) | `free_vars` + `collect_free_*` + `collect_pattern_bindings`; AND the program-walking class tables (`class_implements`/`_supertypes`/`instanceof_table`/`class_mro`/`class_method_origins`/`class_field_conflicts`/`ctor_plan`) | ~700 |

**Recommendation:** separating the helper fns from the data is the higher-value cut (the data is
near-inert; the helpers are real logic and form two cohesive clusters — free-variable walking vs
class/inheritance table building). Consider `ast/walk.rs` (free-var walkers, exhaustive over
Expr/Stmt/Pattern) and `ast/classes.rs` (the 7 program→table builders). Whether to also split the
data enums is optional and lower value (it's pure declarations).

### Exhaustive matches (SACRED — these break the build if a variant is added)
- `free_vars` L268 / `collect_free_expr`830 / `collect_free_block`926 / `collect_free_stmt`936 /
  `collect_pattern_bindings`1038 — these are the recursive walkers and contain the genuinely
  **compiler-exhaustive** `match` over `Expr` (24 variants), `Stmt`, `Pattern`, `LambdaBody`.
  Adding an Expr/Stmt/Pattern variant FORCES updating these. Keeping them together in one
  `ast/walk.rs` is good (single place to audit the walk completeness).
- `class_implements` / `ctor_plan` etc. match on `Item` / `ClassMember` (partial matches with
  `_`, not exhaustive — they filter for the classes/ctors they care about). Lower coupling.

### Coupling / risk
- Re-export discipline: every consumer does `use crate::ast::{…}`. A submodule split MUST
  re-export all public types from `ast/mod.rs` so no consumer path changes. Verify with a grep
  for `ast::` import sites after the split.
- Data enum split is purely mechanical; the helper split is where review attention goes (the
  exhaustive walkers).

---

## 4. lexer.rs (999) — single-pass scanner

### Structural map
- L8 `struct Lexer<'a> { … }`; L15 `impl Lexer` with cursor + scanners:
  `new`16, `peek/peek2/peek3`25-37, `bump`37, `skip_whitespace`49, `scan_number`59,
  `skip_line_comment`102, `skip_block_comment`111, `scan_string`137, `scan_html`201,
  `scan_bytes`257, `hex_digit`315, `scan_ident`329, `current_char`349.
- L357 `fn keyword(&str) -> Option<TokenKind>` — the keyword table (~45 entries).
- L408 `pub fn lex(src) -> Result<Vec<Token>, Diagnostic>` — main tokenizer loop (212 lines):
  a big byte-dispatch `match` driving the scanners + operator recognition.
- L621 `mod tests` (~379 lines).

### Candidate sub-modules
Smaller and more cohesive than the others — the win is modest. If split:

| File | Contents | ~Lines |
|------|----------|--------|
| `lexer/mod.rs` | `struct Lexer`, cursor (`peek*`/`bump`/`current_char`/`skip_whitespace`), `lex` main loop, `keyword` | ~330 |
| `lexer/scan.rs` | `scan_number`/`scan_string`/`scan_html`/`scan_bytes`/`scan_ident`/`hex_digit`/comment skippers | ~290 |

`lex` (the operator/dispatch loop) + `keyword` stay with the struct in `mod.rs`. **Verdict:**
lexer.rs is borderline — at 620 code lines it's the smallest whale; after extracting tests it's
under typical thresholds. Lowest priority; the scanners-vs-driver cut is the only sensible seam.

### Exhaustive matches
- `lex` L408 — big `match` on the current byte; has a default/error arm (NOT compiler-exhaustive).
- `keyword` L357 — `match s` string table → `Option` (`_ => None`); not exhaustive.
None of the lexer matches are compiler-exhaustive, so no variant-coupling risk.

---

## 5. loader.rs (1544) — multi-pass cross-package resolver

### Structural map
- Visibility helpers: `struct DefInfo`46, `vis_violation`55, `vis_word`74.
- `struct Unit`88 + `impl Unit`104; `struct LoadStats`127 + impl; `plural`149.
- **Entry points:** `load`158 (project-vs-loose dispatch), `load_loose_src`174,
  `load_project`202 (the orchestrator, ~180 lines).
- `struct Source`385 + impl; `mangle`412, `pascal`425.
- Import/type maps: `user_import_map`437, `build_type_imports`468.
- `is_builtin_type_leaf`553.
- **Resolution pass (the AST rewrite):** `struct ResolveCtx`562, `resolve_type_ref`584,
  `resolve_type`611, `resolve_item`645, `resolve_block`735, `resolve_stmt`739, `resolve_expr`847,
  `check_fn_visibility`1014, `resolve_call`1028.
- Parse + validation: `parse_one`1095, `parse_at`1103, `enforce_loose_main`1109,
  `validate_folder_path`1123, `relative_under`1170, `same_file`1178, `collect_phg`1186, `walk`1195,
  `read_file`1214.
- L1219 `mod tests`.

### Candidate sub-modules
loader.rs is a pipeline: discover files → parse → build symbol/type/import tables → resolve+mangle
AST → validate → flat-merge. The phases are the cohesion seam:

| File | Contents | ~Lines |
|------|----------|--------|
| `loader/mod.rs` | `Unit`/`LoadStats`, `load`/`load_loose_src` entry points, `load_project` orchestration | ~300 |
| `loader/fs.rs` | `collect_phg`/`walk`/`read_file`/`relative_under`/`same_file`/`validate_folder_path`/`enforce_loose_main`/`parse_one`/`parse_at` | ~220 |
| `loader/symbols.rs` | `Source`, `mangle`, `pascal`, `user_import_map`, `build_type_imports`, `is_builtin_type_leaf`, `DefInfo`, `vis_violation`/`vis_word` | ~280 |
| `loader/resolve.rs` | `ResolveCtx` + `resolve_*` + `check_fn_visibility`/`resolve_call` (the AST-rewrite pass — exhaustive over Item/Type/Stmt/Expr) | ~470 |

### Exhaustive matches
- `resolve_item`645 / `resolve_type`611 / `resolve_stmt`739 / `resolve_expr`847 — the
  name-mangling rewrite walks the WHOLE AST and contains compiler-exhaustive matches over
  `Item`/`Type`/`Stmt`/`Expr` (mirrors `checker::erase_generics`'s exhaustive walk, per CLAUDE.md).
  These are SACRED — adding an Expr/Stmt/Type/Item variant forces an arm here. Keep them together
  in `loader/resolve.rs` so the walk completeness is auditable in one place.

### Coupling / risk — PASS ORDERING is load-bearing
- `load_project` is the orchestrator: it parses every file, builds the symbol table + per-file
  type-import map FIRST, then runs the resolve/mangle pass, then folder=path validation, then the
  flat merge. **The order of these phases is behavior** (e.g. mangle-before-backend is the
  byte-identity guarantee per CLAUDE.md: "mangle + resolve *before* any backend ⇒ run≡runvm
  structural"). A split must keep `load_project` as the single sequencer in `mod.rs`; moving phase
  bodies to sibling files is fine, re-ordering the calls is NOT.
- `ResolveCtx` threads shared state (symbol/type/import maps) through the whole resolve pass —
  it's a parameter-passed context (`&ResolveCtx`), not `&mut self`, so moving the `resolve_*` free
  fns to one file is mechanical.
- Two-pass type rewrite (Pass-1 symbol table, Pass-2 rewrite) coupling must stay intact.

---

## 6. cli.rs (1783) — command surface

### Structural map
- Help/version: `version_line`18, `help_text`23, `help_for`55.
- **`explain_text`170 — a 538-line `match code` with 168 string arms** (one per `E-`/`W-` code).
  This single function is ~30% of the file's code.
- `cmd_explain`708, `cmd_vendor`721.
- Source resolution: `enum SourceSpec`734, `resolve_source`747, `on_deep_stack`763.
- Pipeline glue (shared): `lex_parse`777, `check_and_expand`790 (THE chokepoint — erase/expand
  before backends), `parse_checked`811, `parse_checked_program`820.
- **Per-command entry points:** `cmd_run`825, `cmd_runvm`834, `cmd_check`843, `run_program`859,
  `runvm_program`870, `check_program`882, `check_json_program`895, `transpile_program`904,
  `serve_program`915, `cmd_build`933, `cmd_parse`949, `cmd_lex`957, `cmd_transpile`967,
  `cmd_disasm`980 (+ helpers `annotate`991, `disasm_program`1016).
- Bench: `BENCH_DEFAULT_ITERS`1062, `cmd_bench`1070, `cmd_bench_vs_php`1075, `php_version_line`1081,
  `php_bench_section`1102, `median_of`1188, `fmt_dur`1205, `peak_growth_of`1223, `fmt_kb`1237,
  `bench_report`1247, `bench_report_opts`1253.
- L1357 `mod tests`; L1374 `enum Shape`.

### Candidate sub-modules
cli.rs is the most heterogeneous whale. Two outsized, self-contained chunks dominate (explain
text + bench), and the rest is per-command glue:

| File | Contents | ~Lines |
|------|----------|--------|
| `cli/mod.rs` | version/help, `SourceSpec`/`resolve_source`/`on_deep_stack`, the shared pipeline glue (`lex_parse`/`check_and_expand`/`parse_checked*`), thin `cmd_run`/`cmd_runvm`/`cmd_check`/`run_program`/`runvm_program`/`check_program`/`transpile_program`/`cmd_parse`/`cmd_lex`/`cmd_transpile` | ~400 |
| `cli/explain.rs` | `explain_text` (168 arms) + `cmd_explain` | ~545 |
| `cli/bench.rs` | all `cmd_bench*`/`php_bench_section`/`median_of`/`fmt_*`/`peak_growth_of`/`bench_report*` + `BENCH_DEFAULT_ITERS` + `Shape` | ~290 |
| `cli/disasm.rs` | `cmd_disasm`/`annotate`/`disasm_program` | ~75 |
| `cli/build.rs` | `cmd_build` | ~20 |
| `cli/vendor.rs` | `cmd_vendor` | ~15 |
| `cli/serve.rs` | `serve_program` | ~20 |

**Cleanest wins:** `cli/explain.rs` (538 lines, a pure data-ish string table, zero logic coupling
— extracting it alone takes the file 1783→~1245) and `cli/bench.rs` (cohesive, self-contained).
The tiny per-command fns (build/vendor/serve) are low value to split — could stay in `mod.rs`.

### Exhaustive matches
- `explain_text` L170 — `match code` over string codes with `_ => return None` (NOT exhaustive;
  it's a lookup table). No variant coupling — but it IS a "soft" completeness obligation (every
  emitted `E-`/`W-` code should have an arm). Splitting it out doesn't change that obligation.
- `help_for` L55 — `match cmd` string dispatch, `_` arm. Not exhaustive.
- No compiler-exhaustive matches in cli.rs (it consumes the pipeline, doesn't match the AST/Op
  sets). Disasm's `annotate` uses a `_`-fall-through annotator deliberately (per CLAUDE.md, "no
  second match surface to drift") — that fall-through is INTENTIONAL; do not "complete" it into an
  exhaustive match.

### Coupling / risk
- `check_and_expand` (L790) is "the single `cli::check_and_expand` chokepoint" (per CLAUDE.md) —
  every backend + the project loader route through it for `erase_generics`/alias expansion/html
  desugar BEFORE any backend. Keep it in `cli/mod.rs`; it's the byte-identity gate. Do not
  duplicate it per command.
- The disasm `_`-fall-through annotator is a deliberate single-match-surface design — preserve it.

---

## Cross-file summary of SACRED exhaustive matches (variant-add coupling)
| File | Function(s) | Matched type | Compiler-exhaustive? |
|------|-------------|--------------|----------------------|
| ast.rs | `collect_free_expr`/`_block`/`_stmt`, `collect_pattern_bindings`, `free_vars` | Expr/Stmt/Pattern/LambdaBody | **YES** — keep co-located (`ast/walk.rs`) |
| loader.rs | `resolve_item`/`resolve_type`/`resolve_stmt`/`resolve_expr` | Item/Type/Stmt/Expr | **YES** — keep co-located (`loader/resolve.rs`) |
| parser.rs | `parse_primary`/`parse_stmt`/`parse_item`/`parse_class_member` | TokenKind dispatch | No (`_` arms) — soft, cross-ref in docs |
| parser.rs | `stamp_visibility` | Item | partial (catch-all) |
| lexer.rs | `lex`, `keyword` | byte / &str | No |
| native.rs | — | — | none (flat Vec) |
| cli.rs | `explain_text`, `help_for` | &str | No (lookup tables) |

Note: the `Op` exhaustive triad (`vm.rs`/`chunk.rs`/`compiler.rs`) is OUT of this scope but is the
one truly fatal coupling — none of the six front-end files participate in it except indirectly via
`native::CONSOLE_PRINTLN` feeding `Op::CallNative`.

## Recommended decomposition order (lowest risk → highest value)
1. **Wave 0 (all files):** extract `#[cfg(test)] mod tests` → sibling test files. Zero behavioral
   risk; reclaims ~3000 lines across the six whales.
2. **native/ per-module split** — cleanest cohesion, no exhaustive match, modest index caveat
   (freeze `build()` order + `CONSOLE_PRINTLN`).
3. **cli/explain.rs + cli/bench.rs** — large self-contained chunks, no AST coupling.
4. **parser/ {exprs,stmts,items,types,patterns}** — multi-`impl` over one struct; review the soft
   TokenKind dispatch sites.
5. **loader/ {fs,symbols,resolve}** — keep `load_project` sequencing + the exhaustive resolve walk
   intact.
6. **ast/ {walk,classes} (+ optional data split)** — keep the exhaustive walkers together; re-export
   from `ast/mod.rs`.
7. **lexer/ {mod,scan}** — lowest priority; smallest whale post-test-extraction.
