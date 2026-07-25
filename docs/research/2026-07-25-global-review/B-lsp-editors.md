# B — LSP completion for UFCS + editor syntax-highlighting defects

Investigator note: every claim below is graded. Structural claims come from reading the named file
at the named line; behavioural claims come from **running** the release binary
(`/home/user/phorj/target/release/phg`, LSP driven over real JSON-RPC on stdio) and from **running
the real TextMate engine** (`vscode-textmate` 
 + `vscode-oniguruma`, installed under the scratch dir)
over the repo's actual grammar and actual `.phg` files. No repo file was modified. **No design
ruling is made here** (project Invariant 15) — findings + options + one recommendation each.

Probe artefacts (scratch, disposable):
`/tmp/claude-0/-home-user-phorj/4519ba2a-7bcc-54d2-80b5-d8fbd68ed10d/scratchpad/probe-lsp/`
(`drive*.py` = LSP JSON-RPC drivers, `tm.js` = TextMate tokenizer harness, `sweep*.js` = repo-wide
sweeps, `fixed*.tmLanguage.json` = candidate corrected grammars).

---

## LSP completion pipeline (map)

Entry: `phg lsp` → `src/main.rs:284` → `src/lsp/mod.rs:49` `Server::handle`.
`textDocument/completion` → `src/lsp/mod.rs:264` `Server::completion` → resolves URI + cursor →
byte offset (`symbols::offset_at`) → `src/lsp/completion/mod.rs:73` `complete(text, offset, program,
uri, docs)`.

Capabilities advertised (`src/lsp/mod.rs:461-462`): `textDocumentSync:1`, `hoverProvider`,
`definitionProvider`, `completionProvider` **with `triggerCharacters:["."]`**, `documentSymbolProvider`,
`referencesProvider`, `documentHighlightProvider`, `renameProvider`, `documentFormattingProvider`.
**No `signatureHelpProvider`** — [Verified: read the full literal at `src/lsp/mod.rs:462`; the string
contains no `signatureHelp` key]. So the `.` trigger IS wired (the developer's "no suggestions" is not
a trigger-registration problem) — [Verified: the string contains `"completionProvider":{"triggerCharacters":["."]}`].

`complete()` classifies the cursor **purely lexically** (`context()`, `src/lsp/completion/mod.rs:157`)
into three contexts:

| Ctx | Trigger (lexical) | Candidate set | Source |
|---|---|---|---|
| `Import(prefix)` | current line starts `import ` + dotted-path tail | `catalog::core_module_paths()` (← `cli::module_catalog`) + `loader::project_packages(path)` | `mod.rs:87-117` |
| `Member(recv)` | scan back over ident bytes, require a `.`, read the ident before it; **skipped if that ident is itself preceded by `.`** (`in_dotted_chain`, `mod.rs:191`) | (1) `catalog::module_members(recv)`; else (2) `scope::receiver_type_name(prog, offset, recv)` → `catalog::class_members(prog, ty)` | `mod.rs:118-146` |
| `General` | everything else | top-level symbols + enclosing locals/params + imported qualifiers + other open buffers' top-level symbols + `KEYWORDS` | `mod.rs:147`, `general_items` at `mod.rs:206` |

Parse tolerance: when the live buffer doesn't parse, `parse_repaired` (`mod.rs:299`) **blanks the
cursor's whole line with spaces** (length-preserving) and re-parses.

Receiver-type resolution (`src/lsp/scope.rs:218` `receiver_type_name`):
- `this` → enclosing `Item::Class` name.
- else → declared `Type` of a matching **param**, **`Type x` local** (`typed_binder_in_stmts`,
  `scope.rs:262`), **class field**, or **ctor-promoted param** (`class_receiver_type`, `scope.rs:275`).
- then `named_head` (`scope.rs:241`) keeps only `Type::Named{name}` (unwrapping one `T?`); union /
  intersection / function / `Type::Infer` → `None`.

Candidate set for a **value** receiver (`src/lsp/catalog.rs:19` `class_members`): walks
`program.items` for `Item::Class` / `Item::Trait` / `Item::Interface` **whose `name` equals the type
name**, plus transitive `extends` via `ast::class_supertypes`. **Nothing else.**

### The decisive structural fact

`native::registry()` is referenced **exactly once** in the whole of `src/lsp/`:
`src/lsp/catalog.rs:89`, inside `module_members(qualifier: &str)`, filtered by
`n.module.rsplit('.').next() == Some(qualifier)` — i.e. keyed on the **receiver's literal spelling**,
never on the receiver's **type**.
[Verified: `grep -rn "native::registry\|ufcs\|Ty::\|types::" src/lsp/ | grep -v tests` returns only
`catalog.rs:4` (a doc comment) and `catalog.rs:89`. There is no `Ty`, no `ufcs_first_accepts`, no
type→native mapping anywhere in `src/lsp/`.]

So for `line.` where `line` is a `string`:
`module_members("line")` → `[]` (no Core module leaf is `line`) → `receiver_type_name` → `Some("string")`
→ `class_members(prog, "string")` → no user `Item::Class` named `string` → `[]` → `EMPTY`.

The UFCS *runtime/check-time* resolver that the developer is using lives in
`src/checker/calls/ufcs.rs:28` `try_ufcs`: it walks `native::registry()`, requires
`n.params.len() == args.len()+1`, requires the module to be imported
(`self.imports.get(leaf) == Some(n.module)`, `ufcs.rs:91`) **or** the function to be member-imported
(DEC-274 alias, `ufcs.rs:72-92`), and requires `ufcs_first_accepts(&n.params[0], recv_ty)`
(`ufcs.rs:144`, a `unify` against a throwaway substitution). `NativeFn.params` is `Vec<Ty>`
(`src/native/mod.rs:60`), so the eligibility test the LSP would need is *available* — it is simply
never invoked from the LSP.

### Import gating — the runtime rule, measured

[Verified: ran `phg check` on two probe files]

```
// noimport.phg — string line = "hello"; if (line.contains("ell")) …
type error at 5:22: type `string` has no method `contains`
exit=1
// withimport.phg — same + `import Core.String;`
OK (type-checks clean)
exit=0
```

⇒ UFCS on a primitive **requires** the module import (or a DEC-274 function-level import). Therefore
UFCS completion **should be import-gated** to stay aligned with what the language accepts (the same
"aligned by construction" principle `src/lsp/catalog.rs:1-9` states as its charter).
Asymmetrically, the existing **module-qualifier** path is **not** import-gated
— [Verified: `String.` in a buffer with **no** `import Core.String;` returns **45 items**; see B7].

---

## UFCS completion gaps (B1..B10)

All rows below measured by driving the real `phg lsp` over stdio (`drive2.py` / `drive3.py`).
Raw measurement table:

| Buffer (cursor at the trailing `.`) | items | first labels |
|---|---|---|
| `string line = "hello"; line.` **+ `import Core.String;`** | **0** | — |
| `String.` (module qualifier, import present) | 45 | capitalize, capitalizeWords, characters, chunk, codepointLength, … |
| `Foo f = new Foo(); f.` (user class) | 1 | bar |
| `this.` inside `Foo` | **1** | n  *(method `bar` MISSING — see B6)* |
| `List<int> xs = [1,2,3]; xs.` **+ `import Core.List;`** | **0** | — |
| `line.trim().` (chained) | **58** | main, line, String, package, import, function, class, enum … *(the GENERAL list)* |
| `var line = "hi"; line.` (inferred) | 0 | — |
| `this.s.` where `s: string` | **57** | Foo, String, package, import, function, … *(the GENERAL list)* |
| `function f(int n) { n. }` | **0** | — |
| `Map<string,int> m = {}; m.` | **0** | — |
| `string line; line.` **without** the import | 0 | — |
| `String.` **without** the import | **45** | capitalize, … *(over-suggests)* |
| `Zzz.` (unknown receiver) | 0 | — |

### B1 — UFCS on a primitive-typed receiver yields ZERO completions (the developer's exact report)
[Verified: LSP round-trip above → `{"isIncomplete":false,"items":[]}`]
**Root cause** (single, precise): `src/lsp/completion/mod.rs:139` calls only
`catalog::class_members(p, &ty)`; `catalog::class_members` (`src/lsp/catalog.rs:19-36` →
`decl_members`, `catalog.rs:40-52`) only matches **user** `Item::Class`/`Trait`/`Interface` by name.
A primitive type name (`string`, `int`, `float`, `bool`, `bytes`, `decimal`) is a `Type::Named` with
no corresponding AST item, so the candidate set is empty. The UFCS-eligible native set is never
consulted for a value receiver (see "decisive structural fact").
Scope of the gap: **every** primitive. Measured `string` → 0 and `int` → 0.

### B2 — UFCS on a generic container receiver (`List<T>`, `Map<K,V>`, `Set<T>`) yields ZERO
[Verified: `List<int> xs` → 0 items; `Map<string,int> m` → 0 items, both with the module imported]
Same root cause. `named_head` (`src/lsp/scope.rs:241`) correctly strips the args and returns
`"List"` / `"Map"`, but `class_members(p, "List")` finds no user class. Note these ARE also the
names of Core modules — so a *type-name*→`module_members` bridge would already cover them, whereas
primitives need a `params[0]`-type filter (`Core.String.*` for `string`, `Core.Math`/`Core.Int` for
`int`, …).

### B3 — UFCS on a `var`-inferred receiver yields ZERO (**already-documented deferral**)
[Verified: `var line = "hi"; line.` → 0 items] — this is the conservative gate documented at
`src/lsp/completion/mod.rs:10-13` and `src/lsp/scope.rs:216-217`, pinned by the test
`inferred_or_unknown_receiver_yields_nothing` (`src/lsp/completion/tests.rs:202`), and listed in
`docs/plans/SLICE-STATE.md:1016-1017` as "inferred receivers". **Do not duplicate** — but note it
compounds B1: idiomatic phorj (`var line = f();`) hits B3 even after B1 is fixed.

### B4 — Chained UFCS (`line.trim().`) emits the **GENERAL** list (wrong list, not merely empty)
[Verified: 58 items — `main`, `line`, `String`, then every keyword]
**Root cause:** `context()` (`src/lsp/completion/mod.rs:179-195`) scans back over ident bytes, sees
`.`, then scans back for the receiver ident — but the char before the `.` is `)`, which is not an
ident byte, so `qual` is **empty** and the function falls through to `Ctx::General`
(`mod.rs:192,197`). This actively **violates the stated contract of the project's own test**
`unresolved_lowercase_receiver_emits_neither_module_members_nor_keywords`
(`src/lsp/completion/tests.rs:132-152`, whose comment says "member context … must NOT dump
general/keyword completions after a `.`") — the test only covers the *ident-receiver* shape, so this
path escapes it. Severity: worse than B1 for UX (a list of keywords after `.` looks like the server
is broken).

### B5 — UFCS on a field / dotted chain (`this.s.`, `a.b.`) emits the **GENERAL** list
[Verified: `this.s.` where `s: string` → 57 items = general symbols + keywords]
**Root cause:** the explicit `in_dotted_chain` bail at `src/lsp/completion/mod.rs:191`
(`let in_dotted_chain = j > 0 && b[j - 1] == b'.';`) returns `Ctx::General` rather than "no member
context". Same contract violation as B4. Note `this.s` is exactly the receiver shape
`class_receiver_type` (`src/lsp/scope.rs:275`) already knows how to type — the *lexer* gives up
before the type resolver is reached.

### B6 — `parse_repaired` deletes the declaration on the cursor's own line
[Verified: `this.` inside a one-line method body `public function bar(): void { this. }` returned
only the field `n`; the method `bar` was absent from its own class's member list]
**Root cause:** `parse_repaired` (`src/lsp/completion/mod.rs:299-312`) blanks the **entire cursor
line**. Its doc comment assumes "the receiver's declaration lives on other lines" — true for the
receiver, false for the *enclosing member* when the body is on one line (common for one-liners, and
for the very first characters typed inside a fresh method). Consequence: a member list that is
silently incomplete rather than empty.

### B7 — Module-qualifier completion is NOT import-gated (over-suggests uncompilable calls)
[Verified: `String.` with **no** `import Core.String;` → 45 items; the same program fails
`phg check` with `unknown identifier` / `E-UNKNOWN-IDENT`]
`catalog::module_members` (`src/lsp/catalog.rs:88-97`) filters only by module leaf; it never consults
the buffer's import set. This is the mirror image of B1: the module path over-suggests, the UFCS path
under-suggests. Both diverge from `try_ufcs`'s import rule (`src/checker/calls/ufcs.rs:91`) and from
`catalog.rs`'s own stated charter ("keeps completion aligned with what the language actually accepts
by construction").

### B8 — Hover has **no** native/member support at all (UFCS *or* qualified)
[Verified: hover on the `contains` of `line.contains("ell")` → `null`; hover on the `contains` of
`String.contains("a","b")` → **also** `null`]
**Root cause:** `Server::hover` (`src/lsp/mod.rs:193`) → `resolve_decl` (`mod.rs:179`) → only
`symbols::definition_of` (user top-level) then `scope::local_definition`. There is no branch that
maps an identifier in member position to a `NativeFn` row (which carries `params`/`ret` and could
render a full signature). So the developer gets no signature feedback for **any** stdlib call,
UFCS or qualified.

### B9 — Go-to-definition on a UFCS (or qualified native) member → `null`
[Verified: `textDocument/definition` on `contains` in `line.contains("ell")` → `null`]
`Server::definition` (`src/lsp/mod.rs:225`) uses the same `resolve_decl`. Natives have no `.phg`
source location, so a *jump* is not meaningful — but the current behaviour is indistinguishable from
"symbol unknown". (A user **free function** used via UFCS — `try_ufcs` branch (1),
`src/checker/calls/ufcs.rs:40-59` — *does* have a source location and *should* jump; not measured
separately, but `resolve_decl`'s top-level lookup is name-based, so a UFCS'd free function
`helper` would resolve. [Inferred: `symbols::definition_of` is keyed on the bare name, and the
member-position name equals the free function's name.])

### B10 — No `signatureHelp` at all
[Verified: `INITIALIZE_RESULT` (`src/lsp/mod.rs:462`) declares no `signatureHelpProvider`; an
unknown request would be answered `-32601` by `mod.rs:80`]
So parameter hints are absent for user functions AND natives — no UFCS-specific work exists to
regress. This is the piece that makes `line.contains(` feel dead even once B1 lands.

---

## Grammar ground-truth diff (keyword table)

Ground truth extracted from the compiler, not from docs:

**Reserved keywords** — `src/tokenizer/mod.rs:37-88` `fn keyword()`, 43 words:
`function class enum constructor trait const open abstract sealed public private protected internal
return if else for while do break continue in match import package this true false null new
instanceof interface implements extends mutable static with type throw try catch finally throws`.

**Contextual keywords** (lex as `Ident`, recognised positionally by the parser) — 11 words, each
verified at a parser site:
`var` (`src/parser/mod.rs:186`, `src/parser/stmts.rs:37`), `foreach` (`stmts.rs:42`),
`discard` (`mod.rs:200`, `stmts.rs:45`), `spawn` (`mod.rs:225`), `when` (`stmts.rs:363`,
`patterns.rs:153`), `default` (`patterns.rs:198`), `as` (`stmts.rs:543`, `exprs/climb.rs:168`),
`is` (`exprs/climb.rs:139`), `test` (`items/decls/items.rs:21`), `declare` (`items/decls/items.rs:27`),
`use` (`items/types/traits.rs:60`, `items/types/classes.rs:48`).

**Fault intrinsics** (bare-callable names, not keywords) — `src/checker/common.rs:13-18`
`intrinsic_module_of`: `assert` (`Core.Assert`), `panic`, `todo`, `unreachable` (`Core.Abort`).

**Built-in type names** — `src/checker/common.rs:318-352` `is_builtin_type_name`:
`int float bool string bytes never void empty decimal double i8 i16 i32 i64 u8 u16 u32 u64`
+ nominal `Html Attr List Map Set Error Channel Task`.

### Diff vs `editors/vscode/syntaxes/phorj.tmLanguage.json`

| Word | Compiler status | Grammar (`phorj.tmLanguage.json`) | Verdict |
|---|---|---|---|
| all 43 reserved keywords | reserved | all present (lines 74, 79, 81, 82, 88, 89) | ✅ complete |
| all 11 contextual keywords | contextual | all present (`var`/`use`/`with` L81, `foreach`/`when`/`default`/`spawn` L79, `is`/`as` L82, `test`/`discard` L83, `declare` L68/74) | ✅ complete |
| `assert`, `panic` | intrinsic | L83 `keyword.other.phorj` | ✅ |
| **`todo`** | intrinsic (`common.rs:16`) — [Verified: `phg check` reports `E-UNIMPORTED` for `todo("later")`, i.e. it IS recognised] | **absent** | ❌ MISSING (grammar **and** `src/lsp/keywords.rs`) |
| **`unreachable`** | intrinsic (`common.rs:16`) — [Verified: same probe] | **absent** | ❌ MISSING (grammar **and** `src/lsp/keywords.rs`) |
| `int float string bool bytes void decimal never` | builtin types | L94 `support.type.primitive.phorj` | ✅ |
| **`empty`** | builtin type (`common.rs:327`) — [Verified: `function nop(): empty {}` type-checks clean] | grammar lists **`Empty`** (capital E) | ❌ WRONG CASE — `empty` renders unhighlighted; `Empty` matches nothing real |
| **`double`** | builtin type (`common.rs:332`) — [Verified: `double d = 1.5;` clean] | absent | ❌ MISSING |
| **`i8 i16 i32 i64 u8 u16 u32 u64`** | builtin types (`common.rs:333-340`) — [Verified: `i32 a = 1; u8 b = 2; i64 c = 3;` clean] | absent | ❌ MISSING (8 words) |
| `List Map Set Html Attr Error Channel Task` | builtin nominal | no dedicated rule; swallowed by the generic `\b[A-Z]\w*\b` → `entity.name.type` (L95) | ⚠ cosmetic (renders as a user type, not as stdlib) |
| **`receive`** | **NOT a keyword** — it is a `Channel` method (`src/checker/calls/variants.rs:437` `("Channel","receive")`, `src/interpreter/call.rs:476`). [Verified: `int receive = 1; discard receive;` type-checks **clean** → `receive` is a legal ordinary identifier] | L79 `keyword.control.phorj` | ❌ EXTRA — any variable/field/param named `receive` renders as a control keyword |
| **`\0` escape** | **INVALID** — [Verified: `var s = "a\0b";` → `lex error … invalid escape \0`] | L37 escape alternation includes `0` | ❌ EXTRA — highlights a non-existent escape as valid |
| `\u{HEX}` escape | valid (`src/tokenizer/strings.rs:334` `scan_unicode_escape`) | **absent** from L37 | ❌ MISSING |
| `\xHH` escape | valid **only in `b"…"`** (`strings.rs:503`) | L37 applies it to every string | ⚠ over-broad (cosmetic) |
| `"""…"""` text block | valid (`src/tokenizer/mod.rs:256`, `strings.rs:168` `scan_text_block`) | **no rule** | ❌ MISSING (see B14) |
| `r#"…"#` raw string | valid (`src/tokenizer/mod.rs:226-243`, `strings.rs:296` `scan_raw_string`) | **no rule** (only `r"`, and via the broken `(b\|r)?` prefix) | ❌ MISSING (see B15) |
| `tag"…"` tagged template (ANY ident + `"`) | valid, DEC-212 (`src/tokenizer/mod.rs:277-286`, `strings.rs:415`) | no rule; highlights only by accident of the `\b` bug | ❌ MISSING (see B16) |
| nested block comments | **not** supported (`src/tokenizer/scan.rs:217-240` — first `*/` closes) | non-nesting `begin`/`end` | ✅ matches |

**`src/lsp/keywords.rs` drift** — that file's own doc comment (lines 4-5) states "a keyword
highlighted by the grammar but absent here is drift (Invariant 17)". Absent there but present in the
grammar: **`foreach`**, **`is`**, **`default`**. Also absent everywhere: `todo`, `unreachable`.
[Verified: read all 63 lines of `src/lsp/keywords.rs`; the list contains none of those five.]

---

## Grammar defects (B11..B19)

Every defect below was reproduced by running the **real** `vscode-textmate` engine over the **real**
grammar file. Format: `LINE | "text"⟦scopes⟧`.

### B11 — 🔴 **THE ROOT-CAUSE DEFECT.** `\b` before an *optional* prefix group makes every plain string start at its CLOSING quote

Offending JSON — `editors/vscode/syntaxes/phorj.tmLanguage.json:33-35`:

```json
33          "name": "string.quoted.raw.phorj",
34          "begin": "\\b(b|r)?\"",
35          "end": "\"",
```

`\b` asserts a word boundary. Before the **opening** quote of a plain string the preceding character
is almost always a non-word char (`(`, `=`, `,`, `[`, space) — so `\b` **fails**. Before the
**closing** quote the preceding character is the last char of the string body — usually a word char —
so `\b` **succeeds**. Oniguruma therefore matches the *closing* quote as the string's `begin`, and
then searches forward for the next `"` (the *opening* quote of the next literal) as its `end`.
**String highlighting is inverted, and it leaks across lines.**

[Verified with Python `re` — identical `\b` semantics — the match offsets are the closing quotes:
`'Output.printLine("hi");'` → span (20,21); `'var s = "hi";'` → (11,12); `'  "leading"'` → (10,11);
`'["a"]'` → (3,4). Only `b"…"` / `r"…"` match at offset 0 (a word char *is* the prefix letter).]

**Triggering input** — literally any plain string. Real trace (`vscode-textmate`, current grammar):

```
 7 | "    "⟦-⟧ "string"⟦support.type.primitive⟧ " name "⟦-⟧ "="⟦keyword.operator⟧ " \"world"⟦-⟧ "\""⟦string.quoted.raw.phorj⟧ ";"⟦string.quoted.raw.phorj⟧
 8 | "    Output.printLine("⟦string.quoted.raw.phorj⟧ "\""⟦string.quoted.raw.phorj⟧ "Hello"⟦entity.name.type.phorj⟧ ", {name}"⟦-⟧ "!"⟦keyword.operator⟧ "\");"⟦-⟧
```

Read that carefully: on line 7 the string **body** `world` is unscoped and the closing `"` **plus the
`;`** are `string.quoted`; on line 8 the **code** `    Output.printLine(` is scoped
`string.quoted.raw.phorj` and the string **content** `Hello` is scoped
`entity.name.type.phorj`. In VS Code Dark+ `entity.name.type` is `#4EC9B0` and `support.type` /
`entity.name.tag` are `#569CD6` — i.e. **exactly the "some parts are light blue" the developer
reported**, on regions that are neither types nor strings.

Blast radius, measured repo-wide with the real engine:
- **81 of 266** `examples/**/*.phg` files end **inside an unterminated span** that leaks to EOF
  (whole tails of files rendered as one colour). Repo-wide: **81 / 383** `.phg` files.
- **188 of 266** example files have code punctuation (`;`, `(`, `)`, `=`, `{`, `}`) mis-scoped as
  `string.quoted`.
- **247 of 266** example files tokenize differently once the rule is fixed.
[Verified: `sweep.js` / `sweep2.js` over `/home/user/phorj/examples` and `/home/user/phorj`.]

**Corrected rule** (verified, see the combined fix in B20):
`"begin": "(?<![A-Za-z0-9_])(b|r)?\""` fixes the plain case **but regresses tagged templates**
(`html"…"`, whose `"` *is* preceded by a word char) — [Verified: with that one-char change alone,
`html"<p>{s}</p>"` stops highlighting and its trailing `";` opens a run that swallows the rest of
the file]. The correct fix is the full re-modelled `strings` section in **B20**.

### B12 — 🔴 `//` inside a string comments out the rest of the line (URLs, DSNs, paths)

Offending JSON — `phorj.tmLanguage.json:26` (top-level order `#comments` at line 7, before `#strings`
at line 8):

```json
26        { "name": "comment.line.double-slash.phorj", "match": "//.*$" },
```

This is a **consequence** of B11 (with strings never beginning, `//` is the earliest match on the
line), but it is worth its own row because it is the most visible half of the report.

**Triggering input:** `Output.printLine("https://example.com/a");`

```
 5 | … "printLine"⟦entity.name.function⟧ "(\"https"⟦-⟧ ":"⟦keyword.operator⟧ "//example.com/a\");"⟦comment.line.double-slash.phorj⟧
```

**7 example files affected** — [Verified: `examples/database/mysql.phg`, `examples/database/postgres.phg`,
`examples/fs/walk.phg`, `examples/guide/text-ops.phg`, `examples/guide/uri.phg`,
`examples/guide/validate.phg`, `examples/http-client/fetch.phg`].
**Corrected rule:** none needed here — fixing B11 makes the string `begin` match at an earlier column
than the `//`, so the string wins. [Verified: with the B20 fix the same line tokenizes as
`"("⟦-⟧ "\""⟦string.quoted.double⟧ "https://example.com/a"⟦string.quoted.double⟧ "\""⟦string.quoted.double⟧ ");"⟦-⟧`.]

### B13 — 🔴 an unclosed `/*` inside a string swallows the ENTIRE REST OF THE FILE as a comment

Offending JSON — `phorj.tmLanguage.json:27`:

```json
27        { "name": "comment.block.phorj", "begin": "/\\*", "end": "\\*/" }
```

**Triggering input** (probe `swallow.phg`):

```phorj
Output.printLine("todo /* fixme");
int a = 1;
string b = "still code?";
```

Real trace (current grammar):

```
 3 | … "(\"todo "⟦-⟧ "/*"⟦comment.block.phorj⟧ " fixme\");"⟦comment.block.phorj⟧
 4 | "    int a = 1;"⟦comment.block.phorj⟧
 5 | "    string b = \"still code?\";"⟦comment.block.phorj⟧
 6 | "    Output.printLine(\"more\");"⟦comment.block.phorj⟧
 7 | "}"⟦comment.block.phorj⟧
```

Every subsequent line is comment-coloured. **This is the single most likely mechanism behind the
developer's phrasing "like they are in a comment".** Also downstream of B11 — fixed by B20.
[Verified: same file with the B20 grammar tokenizes fully correctly, lines 4-6 back to normal code scopes.]

### B14 — `"""…"""` text blocks are not modelled at all

The tokenizer supports them (`src/tokenizer/mod.rs:256`, `src/tokenizer/strings.rs:168`
`scan_text_block`, JEP-378 dedent, interpolation + escapes). The grammar has **no** `"""` rule
(`phorj.tmLanguage.json:30-55` is the only string section).

**Triggering input:**
```phorj
var t = """
    text block {s}
    """;
```
Trace (current grammar): `"\"\"\""⟦-⟧`, `"        text block {s}"⟦-⟧`, `"        \"\"\";"⟦-⟧` — the
whole block is tokenized as **code** (unscoped identifiers, `{s}` not an interpolation). Worse, with
the B11 one-char fix alone the un-modelled `"""` becomes three overlapping plain strings and the
file leaks. **Corrected rule:** a dedicated triple-quote rule listed **before** the plain rule (B20).

### B15 — `r#"…"#` raw strings are not modelled; their contents get escapes + interpolation applied

The tokenizer's raw form is `r` + a `#`-run + `"` … `"` + the same `#`-run, with **no escapes and no
interpolation** (`src/tokenizer/mod.rs:226-243`, `src/tokenizer/strings.rs:296`). The grammar's only
raw handling is the `(b|r)?` prefix of the broken rule, which cannot express the `#`-run and applies
the escape + interpolation sub-patterns.

**Triggering input:** `var raw = r#"json {"a": 1}"#;`
Trace (current grammar) shows the inner `"a"` and `{…}` mis-scoped and the delimiter split:
`"    var raw = r#"⟦string.quoted.raw⟧ "\""⟦string.quoted.raw⟧ "json {\"a"⟦-⟧ "\""⟦string.quoted.raw⟧ ": 1}"⟦string.quoted.raw⟧ "\""⟦string.quoted.raw⟧ "#;"⟦-⟧`.
This matters concretely because raw strings are the documented vehicle for **JSON and regex** — the
places most likely to contain `"`/`{`/`//`.
**Corrected rule** (uses an Oniguruma backreference to the captured `#`-run, supported by vscode-textmate):
```json
{ "name": "string.quoted.raw.phorj", "begin": "(?<![A-Za-z0-9_])r(#*)\"", "end": "\"\\1" }
```
[Verified: with this rule the same line tokenizes as one span
`"r#\""⟦string.quoted.raw⟧ "json {\"a\": 1}"⟦string.quoted.raw⟧ "\"#"⟦string.quoted.raw⟧ ";"⟦-⟧`.]

### B16 — tagged templates (`html"…"`, `sql"…"`, DEC-212) have no rule; they highlight only by accident

`src/tokenizer/mod.rs:277-286`: **ANY** `Ident` immediately followed by `"` is a tagged template.
The grammar has no rule; today `html"…"` highlights *by accident* because `\b` succeeds after the
`l` (B11's inversion happens to land correctly here) — and the tag name itself gets **no** scope.
Any fix to B11 that uses a plain "not preceded by an ident char" lookbehind therefore **breaks**
tagged templates. **Corrected rule** — a dedicated rule that captures the tag (B20):
```json
{ "name": "string.quoted.tagged.phorj",
  "begin": "(?<![A-Za-z0-9_])([A-Za-z_][A-Za-z0-9_]*)(\")",
  "beginCaptures": { "1": {"name":"entity.name.function.tag.phorj"},
                     "2": {"name":"punctuation.definition.string.begin.phorj"} },
  "end": "\"", "patterns": [ <escape>, <interpolation> ] }
```
Residual (accepted, note it): a *reserved keyword* immediately followed by `"` (`return"x"`) would be
scoped as a tag, whereas the tokenizer keeps it keyword + string. Pathological input; a
`(?!(?:function|class|…)\")` guard is possible if the developer wants exactness.

### B17 — the escape alternation is wrong in three ways

Offending JSON — `phorj.tmLanguage.json:37`:
```json
37            { "name": "constant.character.escape.phorj", "match": "\\\\(x[0-9A-Fa-f]{2}|[nrt0\\\\\"{}])" },
```
- **Missing `\u{HEX}`** (1-6 hex digits, `src/tokenizer/strings.rs:334`). Consequence, measured: the
  bare `{` of `"\u{1F600}"` opens the **interpolation** rule, so `{1F600}` renders as embedded code.
  [Verified: with only the B11 one-char fix, `"uni \u{1F600} ok"` tokenizes
  `"uni \\u"⟦string.quoted⟧ "{"⟦…interpolation.begin⟧ "1F600"⟦…interpolation⟧ "}"⟦…interpolation.end⟧`.]
- **Includes `0`**, which is **not** a phorj escape [Verified: `phg check` → `lex error … invalid escape \0`].
- Applies `\xHH` to every string, though it is valid only inside `b"…"` (`strings.rs:503`).

**Corrected rule:** `"match": "\\\\(u\\{[0-9A-Fa-f]{1,6}\\}|x[0-9A-Fa-f]{2}|[nrt\\\\\"{}])"`
(and, if exactness is wanted, keep the `x[0-9A-Fa-f]{2}` alternative only in the `b"…"` rule).
[Verified: `"uni \u{1F600} ok"` then tokenizes `"\\u{1F600}"⟦…constant.character.escape⟧`.]

### B18 — an interpolation containing an escaped nested string (`{f(\"{root\}/x\")}`) leaks

This one is **independent of B11** and is the last remaining leak source. The tokenizer explicitly
supports a nested string inside `{…}` (`src/tokenizer/strings.rs:38-46`, "M-DOGFOOD W2"), where the
inner string's quotes are written `\"` and a literal brace inside it is written `\}`. The grammar's
interpolation rule (`phorj.tmLanguage.json:38-51`) `include`s `#strings` and the escape rule, so the
`\"` is eaten as a plain escape and the inner `\}` is eaten as an escape too — the *nested* `{`
never closes.

**Triggering input** — a real repo file, `examples/fs/walk.phg:30`:
```phorj
Output.printLine("main.phg is {FileSystem.size(\"{root\}/src/main.phg\")} bytes");
```
[Verified: with the B20 grammar *minus* this fix, lines 32-51 of `walk.phg` all render inside
`string.quoted…,meta.embedded.interpolation`, i.e. 20 lines of code coloured as an embedded string.]

**Corrected rule** — prepend to the interpolation's `patterns` a nested escaped-quote string rule:
```json
{ "name": "string.quoted.double.nested.phorj",
  "begin": "\\\\\"", "end": "\\\\\"",
  "patterns": [ { "name": "constant.character.escape.phorj", "match": "\\\\[nrt\\\\{}]" } ] }
```
[Verified: with it, `walk.phg:30` tokenizes exactly right —
`"\\\""⟦…string.quoted.double.nested⟧ "{root"⟦…nested⟧ "\\}"⟦…nested,constant.character.escape⟧ "/src/main.phg"⟦…nested⟧ "\\\""⟦…nested⟧` — and `walk.phg` ends clean.]

### B19 — `#` is **NOT** treated as a line comment — the attribute hypothesis is FALSE (explicitly checked)

The brief asked to check this explicitly. **It is not the bug.** The only `#`-anchored comment rule is
`phorj.tmLanguage.json:21` `"match": "\\A#!.*$"`, which requires `#!`. Attributes have their own rule
(`phorj.tmLanguage.json:56-64`, `begin: "#\\["`).

**Triggering input:** `#[Entry(kind: EntryKind.Cli)]` — real trace:
```
 3 | "#["⟦meta.attribute.phorj⟧ "Entry"⟦meta.attribute,entity.name.tag.attribute⟧ "(kind: "⟦meta.attribute⟧ "EntryKind"⟦meta.attribute,entity.name.tag.attribute⟧ "."⟦meta.attribute⟧ "Cli"⟦meta.attribute,entity.name.tag.attribute⟧ ")"⟦meta.attribute⟧ "]"⟦meta.attribute⟧
```
Correct, no comment scope. [Verified: real engine, real grammar.]

Two **residual, cosmetic** attribute notes (not the reported bug):
- `entity.name.tag` is `#569CD6` **blue** in Dark+, so `Entry` / `EntryKind` / `Cli` genuinely do
  render light blue — plausible as a *secondary* contributor to the developer's wording, but the
  surrounding text is *not* mis-coloured, so it is not the defect.
- The attribute body includes only `#strings` (`L62`); it lacks `#numbers` / `#constants` /
  `#keywords`, so `#[Table(name: "u", version: 2)]` leaves `2` unscoped. Also `\A` in
  `phorj.tmLanguage.json:21` anchors to the start of **whatever line is being scanned**
  (vscode-textmate feeds one line at a time), so a mid-file line beginning `#!` would be
  comment-scoped. [Inferred: vscode-textmate's per-line scanning; harmless in practice — `#!` is not
  valid phorj mid-file.]

### B20 — the verified combined corrected `strings` section

Replacing `repository.strings` in `editors/vscode/syntaxes/phorj.tmLanguage.json` with the
following (order matters — TextMate breaks position ties by pattern order, and each prefixed form
begins at an *earlier* column than its bare `"`) resolves B11-B18:

```
1. raw      : begin (?<![A-Za-z0-9_])r(#*)"      end "\1                (no sub-patterns)
2. bytes    : begin (?<![A-Za-z0-9_])b"          end "                  patterns: [ESC]
3. textblock: begin """                           end """                patterns: [ESC, INTERP]
4. tagged   : begin (?<![A-Za-z0-9_])([A-Za-z_]\w*)(")  end "            patterns: [ESC, INTERP]
5. plain    : begin "                             end "                  patterns: [ESC, INTERP]

ESC    = \\(u\{[0-9A-Fa-f]{1,6}\}|x[0-9A-Fa-f]{2}|[nrt\\"{}])
INTERP = begin \{ end \}  patterns: [NESTED, keywords, constants, numbers, strings, functions, types, operators]
NESTED = begin \\"  end \\"  patterns: [ \\[nrt\\{}] ]
```

**Verification of the combined fix** [Verified — ran the real engine over the real corpus]:

| metric | current grammar | with the corrected section |
|---|---|---|
| `examples/**/*.phg` ending inside an unterminated span | **81 / 266** | **0 / 266** |
| all repo `*.phg` ending inside an unterminated span | **81 / 383** | **0 / 383** |
| `//`-in-string comment leak | 7 files | 0 |
| `/*`-in-string file swallow | reproduced | gone |
| escapes / interpolation / text blocks / `r#"…"#` / `html"…"` | broken (B14-B17) | all correct |

(Grammar file used: `scratchpad/probe-lsp/fixed3.tmLanguage.json`; harness: `tm.js` / `sweep3.js` / `sweep4.js`.)

Note one incidental consequence: the scope name for a *plain* string becomes
`string.quoted.double.phorj` (today every string carries `string.quoted.raw.phorj`, which is
semantically wrong for a non-raw string). All are `string.quoted.*`, so every theme colours them
identically — but it IS a user-visible scope rename, so it belongs in the developer's decision.

### B21 — the grammar has **ZERO** automated coverage — the structural reason B11 survived

[Verified: `grep -rn "tmLanguage\|editors/vscode" tests/ scripts/ Cargo.toml build.rs` → **no
matches**; `ls tests/` shows 35 integration files, none grammar-related.]
`phg check ≡ LSP` is test-pinned (DEC-252, `src/lsp/tests.rs`), the formatter is swept over every
repo `.phg` (`every_repo_phg_formats_idempotently_and_safely`), the differential globs
`examples/**/*.phg` — but the **one editor surface with no gate at all** is the grammar. A
`\b`-before-optional-group bug that inverts every string in the language shipped and persisted
because nothing executes the grammar.

---

## File association & client wiring

**VS Code** (`editors/vscode/package.json`) — [Verified: read all 44 lines]
- `contributes.languages[0]`: `id: "phorj"`, `aliases: ["Phorj"]`, `extensions: [".phg"]`,
  **`firstLine: "^#!.*\\bphg\\b"`** ✅ (DEC-336's shebang association, `C-decisions.md:3073`),
  `configuration: "./language-configuration.json"`.
- `contributes.grammars[0]`: `scopeName: "source.phorj"` → `./syntaxes/phorj.tmLanguage.json` ✅.
- `activationEvents: ["onLanguage:phorj"]`, `main: "./extension.js"` ✅.
- `configuration.properties["phorj.serverPath"]` default `"phg"` ✅.
- **Absent:** `icon` / `icons` for the language, `contributes.languages[].filenames` (an
  extensionless entry named e.g. `console` with **no** shebang would not associate — but the
  tokenizer skips a shebang and DEC-336's model *is* shebang-based, so `firstLine` is the right
  mechanism), `contributes.snippets`, `contributes.configurationDefaults`,
  no `.vscodeignore`, no `embeddedLanguages` (relevant if `html"…"` should ever host the HTML grammar).
- **LSP client is wired** ✅ — `editors/vscode/extension.js:12-21`: reads `phorj.serverPath`, spawns
  `{ command, args: ["lsp"], transport: TransportKind.stdio }`, `documentSelector:
  [{ scheme:"file", language:"phorj" }]` (language-id based, so shebang files get the server),
  `synchronize.fileEvents` on `**/*.phg`, `client.start()`. Clean and correct.
- **Version drift:** `package.json` says `"version": "0.5.0"` but `editors/vscode/README.md:46-47`
  still instructs `vsce package` → `phorj-0.4.0.vsix` / `code --install-extension phorj-0.4.0.vsix`.
  [Verified: read both files.] Minor docs staleness.

**`editors/vscode/language-configuration.json`** — [Verified: read all 23 lines]
`comments` (`//`, `/* */`) ✅, `brackets` `{} [] ()` ✅, `autoClosingPairs` incl. `"` with
`notIn:["string"]` ✅, `surroundingPairs` ✅.
**Absent:** `folding` (markers or `offSide`), `wordPattern`, `indentationRules`, `onEnterRules`
(so no auto-continuation of `/* … */`), `autoCloseBefore`, and no `"""` triple-quote pair.
`notIn: ["string"]` depends on the grammar's `string` scope being correct — so B11 also degrades
auto-closing behaviour. [Inferred: VS Code evaluates `notIn` against the tokenizer's scopes;
with B11 the scope at a cursor inside a string is *not* `string`, and the scope just after a closing
quote *is*.]

**PhpStorm / JetBrains** (`editors/phpstorm/README.md`) — [Verified: read all 67 lines]
- No compiled plugin, by design (DEC-181 "LSP-first symmetric, then full-native"; a native plugin is
  a tracked follow-up).
- Highlighting: JetBrains **TextMate Bundles**, pointed at `editors/vscode/` (JetBrains reads its
  `package.json` `grammars` entry). ⇒ **PhpStorm consumes the exact same broken grammar**, so B11-B18
  are identical in both IDEs — consistent with the developer reporting the same symptom in both.
- Intelligence: **LSP4IJ** user-defined server, command `phg lsp`, mapping `*.phg` → language id
  `phorj`. Extensionless shebang entries: manual filename-pattern registration in both File Types
  and the LSP4IJ mapping (documented at README lines 43-52).
- **Doc defects:** (a) README lines 62-65 are **stale** — they claim references/rename are
  single-document ("cross-file is a server follow-up") and that "**instance/type-aware member
  completion** (`myVar.` → the variable's class methods)" is a follow-up; both shipped (DEC-327
  project-wide find-usages, `C-decisions.md:2724`; typed-receiver completion,
  `src/lsp/completion/mod.rs:126-145` + tests at `src/lsp/completion/tests.rs:183`). (b) the
  "Completion" bullet at lines 53-56 is mis-nested under the shebang subsection instead of under
  "Notes". This is an Invariant-17 / DEC-181 "editors always current" violation.

**`editors/README.md`** — [Verified: read all 18 lines] accurate; describes one server + one grammar.

---

## Existing recorded follow-ups (do not duplicate)

Already recorded in the SSOTs — cross-referenced so nothing here is double-tracked:

| Recorded item | Where | Overlaps |
|---|---|---|
| "Also-remaining LSP: **prelude-class members**, **whole-project cached index**, **inferred receivers**" | `docs/plans/SLICE-STATE.md:1016-1017` | **B3** is exactly "inferred receivers" — do not re-open as new |
| "LSP **attribute-arg completion** (`EntryKind.` variants) = follow-up on the existing LSP punch-list" | `docs/plans/SLICE-STATE.md:85-86` (DEC-337) | adjacent to B4/B5 (both are "the receiver isn't a bare ident") — could ride the same context() rework |
| "LSP find-usages project-wide" | `docs/plans/SLICE-STATE.md:1015` | **already DONE** — DEC-327, `C-decisions.md:2724`, `src/lsp/references.rs`. SLICE-STATE item 4 is stale |
| "multi-file **rename** = WorkspaceEdit slice, queued"; documentHighlight still single-buffer | `C-decisions.md:2733` | not touched here |
| DEC-336 shebang/extensionless + editor currency, **BUILT 2026-07-24** | `C-decisions.md:3274-3286`, `SLICE-STATE.md:181-186` | **B-association section confirms it shipped correctly** (`firstLine` present) |
| DEC-252 / Invariant 17: `phg check` ≡ LSP diagnostics; editors both-same-change (DEC-181) | `C-decisions.md:1927`, `:264` | the *norm* that B7 (import gating) and the stale PhpStorm README violate |
| "prelude members, inference" listed as completion follow-ups | `src/lsp/mod.rs:16`, `src/lsp/completion/mod.rs:12-13`, `src/lsp/catalog.rs:16-18` | same as B3 + prelude classes |

**NOT recorded anywhere** — every one of these is a NEW finding:
- **B1 / B2** — UFCS member completion for primitive and container receivers. [Verified:
  `grep -i ufcs docs/plans/SLICE-STATE.md docs/plans/MASTER-PLAN.md KNOWN_ISSUES.md` returns only
  compiler-internal UFCS rows (`rewrite_ufcs`, `E-UFCS-AMBIGUOUS`, the UFCS-vs-import ruling) — nothing
  about completion.] Note `SLICE-STATE.md:1022` asserts **"LSP AUTOCOMPLETE — DONE + COMPREHENSIVE"**;
  that claim is measurably false for the language's *primary* stdlib call syntax.
- **B4 / B5** — the `Ctx::General` fallthrough after a `.`.
- **B6** — `parse_repaired` losing the cursor line's own declaration.
- **B7** — module-member completion not import-gated.
- **B8 / B9 / B10** — native hover, native go-to-definition, signature help.
- **B11-B21** — every grammar defect. [Verified: `grep -rn -i "tmLanguage|syntax highlight|highlighting"`
  over `KNOWN_ISSUES.md`, `SLICE-STATE.md`, `MASTER-PLAN.md` returns only three *forward-looking*
  MASTER-PLAN lines (1480, 1487, 1726) about keeping the grammar current — **no bug is recorded**.]

---

## Options & recommendation per finding

Per Invariant 15, these are options for the developer to rule on, with one recommendation and its why.

### B11-B18 + B21 (grammar) — the reported "light blue" bug
| Option | Trade |
|---|---|
| **(A) RECOMMENDED — replace `repository.strings` wholesale with the B20 section (raw / bytes / textblock / tagged / plain + corrected ESC + nested-interp), and add a grammar gate** | One cohesive edit, empirically verified to take unterminated-span leakage from 81/383 files to **0/383** and to fix B11-B18 together. The gate is what stops the recurrence (B21): a `scripts/grammar-check.mjs` running `vscode-textmate` over every repo `.phg` and asserting "no file ends inside a span" + no code punctuation in a `string`/`comment` scope — dev-only, node-based, so it adds **no Rust dependency** and stays inside the std-only policy; wire it into `pre-push` beside the other sweeps. Complement it with a *pure-Rust* keyword-drift test (parse the JSON with the existing `crate::json` parser, assert the grammar's keyword/primitive lists against `tokenizer::keyword` + `checker::common::is_builtin_type_name`) — that half needs no node at all. Cost: the plain-string scope renames `…raw` → `…double` (theme-neutral; all are `string.quoted.*`). |
| (B) minimal one-character fix (`\b` → `(?<![A-Za-z0-9_])`) only | Fixes B11/B12/B13 but **regresses tagged templates** and leaves `"""` un-modelled → still 29/266 leaking files. [Verified — measured.] Not recommended as a stopping point. |
| (C) fix the grammar, skip the gate | Cheapest now; but B21 is precisely why a language-wide inversion shipped unnoticed. |
| (D) migrate to a semantic-tokens LSP provider and let the grammar be a coarse fallback | Architecturally the strongest end state (the compiler already has exact spans + a comment side-channel, so scopes would be exact by construction and DEC-252-style "one pipeline" would extend to colour) — but a much bigger slice, and the grammar still matters for the "no server" path both READMEs advertise. Worth surfacing as a *follow-on*, not as the fix for this report. |

Why (A): it is the only option measured to zero the leak metric, it fixes the developer's exact
symptom in **both** IDEs (they share the grammar), and it closes the coverage hole that let the bug
exist. The whole change is one JSON section plus test scaffolding — no compiler, no backend, zero
byte-identity surface.

### B1 / B2 (UFCS completion — primitives and containers)
| Option | Trade |
|---|---|
| **(A) RECOMMENDED — add a type→UFCS-candidate leg in `src/lsp/catalog.rs`, import-gated** | New `catalog::ufcs_members(program, text, type_name) -> Vec<(String, u32)>`: for the receiver's declared `Type` head, walk `native::registry()` and keep rows where (i) the module's leaf is in the buffer's import set (reuse `completion::imported_qualifiers`, or better the parsed `Item::Import` list) and (ii) `params[0]`'s `Ty` head matches the receiver's declared type head (`string`→`Ty::String`, `int`→`Ty::Int`, `List<_>`→`Ty::List`, …; a `Ty::Param` first param matches anything, mirroring `ufcs_first_accepts`'s generic case at `src/checker/calls/ufcs.rs:144`). Merge with `class_members` so a user class that ALSO has UFCS free functions shows both. Also fold in `try_ufcs` branch (1): a **user free function** whose first param accepts the receiver (`src/checker/calls/ufcs.rs:40-59`) — otherwise the completion still misses the half of UFCS that is user code. Keeps the "aligned by construction" charter (`src/lsp/catalog.rs:1-9`): a new native appears in completion with no LSP edit. |
| (B) run the real checker on the repaired buffer and use the resolved `Ty` | Maximum fidelity (would also solve **B3** inferred receivers and chained receivers **B4**), and it reuses `ufcs_first_accepts` verbatim rather than re-deriving a head-match. But it needs a checker run per keystroke on a *repaired* buffer — a perf and robustness question the developer should rule on, and `Checker` state isn't currently exposed for this. |
| (C) hardcode a `string`/`int`/`List` → module table in the LSP | Fast, but it is exactly the hand-maintained list `catalog.rs`'s charter forbids; guaranteed drift. Not recommended. |
| (D) skip the import gate (suggest all UFCS-eligible natives, add the import on accept) | Better discovery ("what can I do with a string?") at the cost of suggesting things that don't compile — unless paired with a `additionalTextEdits` auto-import, which is a real (and attractive) slice of its own. Worth a separate question. |

Why (A): it is the smallest change that makes `line.contains(` work, it is registry-driven so it
cannot drift, it is entirely off the byte-identity spine, and it leaves (B)/(D) open as later
upgrades. Blast radius to check: the existing test
`unresolved_lowercase_receiver_emits_neither_module_members_nor_keywords`
(`src/lsp/completion/tests.rs:132`) stays green (its receiver has **no** declared type), but
`inferred_or_unknown_receiver_yields_nothing` (`tests.rs:202`) must keep pinning B3's deferral
explicitly rather than by accident.

### B4 / B5 (`Ctx::General` fallthrough after a `.`)
**Recommended:** introduce a third member outcome — `Ctx::MemberUnresolved` — returned whenever the
lexical scan-back *did* find a `.` but could not name a single typeable receiver (`in_dotted_chain`,
empty `qual`, a `)`/`]` before the dot). Emit `EMPTY` for it. This is a ~10-line change in
`context()` (`src/lsp/completion/mod.rs:157-198`) that makes the code honour the contract its own
test comment already states, and it should land *before* B1 so the fix isn't masked by noise.
Alternative (larger, better): resolve chained/call-result receivers for real, which converges with
option (B) above.

### B6 (`parse_repaired` blanks the cursor's declaration)
**Recommended:** blank only from the last `.` (or the cursor) back to the statement start rather than
the whole line — or, cheaper and safer, blank the line **and** retry with the line truncated at the
receiver's `.` if the first repair yields no members. Alternative: accept as documented (it only
degrades one-liner bodies). Low priority relative to B1/B4.

### B7 (module completion not import-gated)
**Recommended:** gate `Ctx::Member`'s module branch on the buffer's import set (the same predicate
B1's option (A) needs), and — since the developer may prefer discovery over strictness — pair it with
`additionalTextEdits` that insert `import Core.X;` on accept. This is a genuine **user-visible
behaviour decision** (strict vs discoverable) and per Invariant 15 must be the developer's.
Note the two directions must be ruled **together**: B1(D) and B7 are the same question asked from
opposite ends, and answering them differently would leave the LSP internally inconsistent.

### B8 / B9 / B10 (hover, definition, signature help on natives)
**Recommended (single slice):** teach `resolve_decl`'s caller a "member-position native" branch that
looks up `native::registry()` by `(module_leaf, name)` — or by UFCS eligibility for the unqualified
form — and renders `Module.name(params…): ret` from the row's `params`/`ret`
(`src/native/mod.rs:60-62`). That one lookup serves **hover** (a real signature instead of `null`)
and **signature help** (add `signatureHelpProvider` with `triggerCharacters:["(",","]` to
`src/lsp/mod.rs:462`). For **definition** on a native, recommend returning `null` deliberately but
surfacing the signature via hover — a synthetic location would be a lie; alternatively link to the
docs URL for that module if one exists. Ranking: **B8/B10 are the higher-value pair** — a correct
member list (B1) without a signature still leaves the developer guessing the arguments.

### Suggested ordering (not a ruling)
1. **B11-B18 grammar section + B21 gate** — biggest visible win, zero spine risk, fully pre-verified here.
2. **B4/B5** context fix (unblocks and de-noises everything after it).
3. **B1/B2** UFCS candidate leg + **B7** import-gating decision (ruled together).
4. **B8/B10** native hover + signature help.
5. **B6**, then the recorded B3 / prelude-members / attribute-arg follow-ups.
6. Docs currency in the same changes (Invariant 17 / DEC-181): `editors/phpstorm/README.md:53-65`,
   `editors/vscode/README.md:46-47`, `editors/vscode/package.json` version, the
   "LSP AUTOCOMPLETE — DONE + COMPREHENSIVE" claim at `docs/plans/SLICE-STATE.md:1022`, and the stale
   "LSP find-usages project-wide" queue entry at `SLICE-STATE.md:1015`.
