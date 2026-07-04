# `phg format` — the width-canonical formatter (DEC-187)

`phg format` prints Phorj source in a single canonical shape, laid out **from the parsed AST** against
a **100-column budget**. Run it over a file, a directory, or stdin:

```bash
phg format src/            # format every *.phg under src/ in place
phg format --check src/    # CI gate: exit 1 if any file is not already canonical (no writes)
phg format - < in.phg      # format stdin → stdout
```

`showcase.phg` in this directory is a runnable program already in canonical form
(`phg run examples/fmt/showcase.phg` → `small=15` / `total=150` / `the very top score`).

## What "width-canonical" means

The formatter decides every line break **from width alone** — like `prettier`, `rustfmt`, and `gofmt`.
It does **not** preserve where you happened to press Enter. Two consequences:

* A construct that **fits** in 100 columns stays on **one line** — even if you hand-broke it:

  ```phorj
  var x = result        //  in:  gratuitously broken
      .map(f)
      .getOrElse(0);
  // →
  var x = result.map(f).getOrElse(0);   // out: collapsed, it fits
  ```

* A construct that **overflows** breaks deterministically, one element per line, indented four
  columns past the statement:

  ```phorj
  var total = accumulateFiveNumbers(
      firstOperandValue,
      secondOperandValue,
      thirdOperandValue,
      fourthOperandValue,
      fifthOperandValue
  );
  ```

Because the layout is a pure function of the AST and the width, formatting is **idempotent**
(`fmt(fmt(x)) == fmt(x)`) and **meaning-preserving** (`phg run x` ≡ `phg run (fmt x)`, enforced across
the whole example corpus by `tests/fmt.rs`). This is why DEC-187 dropped the earlier "preserve author
breaks" idea: honouring hand-inserted breaks would need the original source text, which the
print-from-AST design deliberately does not keep.

## What wraps today

* **call / `new` / `parent` argument lists** — `f(a, b, c)`
* **collection and map literals** — `[a, b, c]`, `[k => v, …]`
* **`match` expressions** — one `arm => body` per line
* **method chains** — a `.`/`?.` spine with **two or more** member accesses breaks before each link:

  ```phorj
  var r = source
      .mapEachValueWithCare(transformer)
      .keepEveryMatching(predicate)
      .collapseInto(combiner)
      .done();
  ```

## Interpolation is never broken

An expression inside a string-interpolation hole (`"total is {a + b + c + …}"`) is always laid out
flat, however long the line — a newline inside the quotes would change the string's value. Meaning
preservation wins over the column budget there.

## Not yet wrapped (tracked follow-ups)

These stay on one line even past 100 columns for now (see `KNOWN_ISSUES.md`): binary-operator chains
(`a + b + c + …`), declaration parameter lists (`function f(…)`), class/interface headers
(`class C extends … implements …`), control-flow conditions (`if (…)`, `while (…)`), and
`var … = …` destructuring initializers. Each is a self-contained extension of the same doc-IR.
