//! M-Lift L5 — the round-trip differential gate for the ↑ PHP→Phorj direction.
//!
//! `lift` carries no byte-identity guarantee on its own (it's a best-effort draft), so confidence is
//! *earned* here: for a Tier-1 PHP sample we **lift** it to Phorj, then check that the lifted Phorj
//! behaves exactly like the original PHP — running it three ways (`run` interpreter, the VM VM, and
//! its own transpiled-back PHP) and asserting all three match the **original PHP's** stdout. A full
//! match is evidence the lift preserved behavior; the original program is the source of truth.
//!
//! Gating mirrors the differential oracle: `PHORJ_REQUIRE_PHP=1` makes a missing `php` FAIL (CI),
//! otherwise it skips loudly. `PHORJ_PHP=<path>` overrides the binary. (The tiny php-runner helpers
//! are duplicated from `differential.rs` rather than shared — integration test files here are each
//! self-contained, the same pattern as `process.rs`/`serve.rs`.)

use phorj::cli::{cmd_run, cmd_treewalk};
use phorj::{cli, lift};
use std::process::Command;

/// Resolve the php binary: `PHORJ_PHP` override, else `php` on PATH if `--version` succeeds.
fn php_bin() -> Option<String> {
    // `PHORJ_SKIP_PHP=1` forces the deterministic Rust-only gate (run == vm, no oracle)
    // regardless of what `php` is on PATH — set by the pre-commit hook. The full PHP-oracle spine
    // check moves to pre-push (`PHORJ_REQUIRE_PHP=1` against the 8.5 floor).
    if std::env::var("PHORJ_SKIP_PHP").as_deref() == Ok("1") {
        return None;
    }
    let cand = std::env::var("PHORJ_PHP").unwrap_or_else(|_| "php".to_string());
    let ok = Command::new(&cand)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    ok.then_some(cand)
}

/// The fails-not-skips gate. `Some(php)` ⇒ run; `None` ⇒ caller returns. Under `PHORJ_REQUIRE_PHP=1`
/// a missing php panics instead of skipping.
fn php_or_gate(test: &str) -> Option<String> {
    if let Some(p) = php_bin() {
        return Some(p);
    }
    assert!(
        std::env::var("PHORJ_REQUIRE_PHP").as_deref() != Ok("1"),
        "{test}: php required (PHORJ_REQUIRE_PHP=1) but not found on PATH or $PHORJ_PHP"
    );
    eprintln!("SKIP {test}: php not found — set PHORJ_REQUIRE_PHP=1 to make this a failure");
    None
}

/// The `php` flags for a hermetic run — `-n` (ignore php.ini), plus an explicit `-d extension=bcmath`
/// when bcmath isn't compiled in (it's an ini-loaded shared extension on CI's `setup-php`, which `-n`
/// would otherwise disable). Mirrors `differential.rs::php_n_args`; see its doc for the rationale.
fn php_n_args(php: &str) -> &'static [&'static str] {
    static BUILTIN: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    let has_builtin = *BUILTIN.get_or_init(|| {
        Command::new(php)
            .args(["-n", "-r", "exit(extension_loaded('bcmath') ? 0 : 1);"])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    });
    if has_builtin {
        &["-n"]
    } else {
        // `display_errors=stderr` keeps stdout clean if the `.so` can't be found; see the twin in
        // `differential.rs::php_n_args`.
        &[
            "-n",
            "-d",
            "display_errors=stderr",
            "-d",
            "extension=bcmath",
        ]
    }
}

/// Write `php_src` to a per-label temp file, run it with `php -n` (no php.ini → hermetic), return
/// stdout. Panics if php exits non-zero.
fn run_php(php: &str, php_src: &str, label: &str) -> String {
    let safe: String = label
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    let path = std::env::temp_dir().join(format!("phorj_lift_rt_{safe}.php"));
    std::fs::write(&path, php_src).expect("write temp php");
    let out = Command::new(php)
        .args(php_n_args(php))
        .arg(&path)
        .output()
        .expect("spawn php");
    let _ = std::fs::remove_file(&path);
    assert!(
        out.status.success(),
        "php exited non-zero for {label}:\n{}\n--- php ---\n{php_src}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).expect("utf-8 php stdout")
}

/// The round-trip: lift `php_src` → Phorj, then assert the lifted Phorj run three ways
/// (interpreter, VM, transpiled-back PHP) all equal the **original** PHP's stdout.
fn roundtrip(php: &str, label: &str, php_src: &str) {
    let phorj =
        lift::lifter::lift_source(php_src).unwrap_or_else(|e| panic!("{label}: lift failed: {e}"));

    let expected = run_php(php, php_src, &format!("{label}_orig"));

    let interp = cmd_treewalk(&phorj).unwrap_or_else(|e| {
        panic!("{label}: lifted Phorj failed on the interpreter: {e}\n--- phorj ---\n{phorj}")
    });
    assert_eq!(
        interp, expected,
        "{label}: interpreter ≠ original PHP\n--- phorj ---\n{phorj}"
    );

    let vm =
        cmd_run(&phorj).unwrap_or_else(|e| panic!("{label}: lifted Phorj failed on the VM: {e}"));
    assert_eq!(vm, expected, "{label}: VM ≠ original PHP");

    let php_back = cli::cmd_transpile(&phorj)
        .unwrap_or_else(|e| panic!("{label}: lifted Phorj failed to transpile back: {e}"));
    assert_eq!(
        run_php(php, &php_back, &format!("{label}_back")),
        expected,
        "{label}: transpiled-back PHP ≠ original PHP\n--- php ---\n{php_back}"
    );
}

#[test]
fn lift_recognizes_tostring_as_attribute() {
    // DEC-331 D9b: PHP `__toString` lifts to a phorj `#[ToString]` method named `toString`
    // (the reverse of the transpiler's `__toString` delegate). Pure lift — no php needed.
    let php = "<?php class M { public function __construct(public int $c) {} \
               public function __toString(): string { return \"m\"; } }";
    let phorj = lift::lifter::lift_source(php).expect("lift ok");
    assert!(
        phorj.contains("#[ToString]"),
        "expected #[ToString] in:\n{phorj}"
    );
    assert!(
        phorj.contains("function toString()"),
        "expected a `toString` method in:\n{phorj}"
    );
    assert!(
        !phorj.contains("__toString"),
        "the PHP magic name must be gone:\n{phorj}"
    );
}

#[test]
fn lift_roundtrip_preserves_behavior() {
    let Some(php) = php_or_gate("lift_roundtrip_preserves_behavior") else {
        return;
    };

    // LIFT-ECHO-INT (2026-09-05): `echo <non-string>` now lifts to an interpolation, so an int echo and
    // a `.`-concat with an int operand round-trip too — the `int_echo` case below is the proof, and
    // the old "raw int echo is avoided on purpose" workaround is gone. Floats appear only where
    // phorj's rendering and PHP's precision-14 echo agree (`float_division` below: 3.5, 0.25, an exact
    // 2); a value like `1/3` would hit the known formatting divergence (KNOWN_ISSUES), unrelated to the lift.
    let cases: &[(&str, &str)] = &[
        (
            "int_echo",
            r#"<?php function half(int $n): int { return intdiv($n, 2); } echo half(7) . "|" . half(8) . "|"; echo 42; echo true;"#,
        ),
        (
            "concat",
            r#"<?php function greet(string $n): string { return "Hi, " . $n; } echo greet("Phorj");"#,
        ),
        // Row 5n (§LIFT-UNICODE-ESCAPE): every PHP escape class on BOTH lexer paths, in one program, so a
        // divergence names its escape. The hard bytes (NUL, ESC, VT, FF, a three-escape UTF-8 sequence)
        // also cross the transpile-back leg, which nothing had run before. `\{` keeps its backslash in
        // PHP and `$x` still interpolates after it — PHP has no escaped hole.
        // DEC-534 (row 5q): a union of two Map-typed class constants — scout Text.php's shape — lifts to
        // `Map.union`, and the LEFT value wins on the shared key `é` on every leg, as PHP `+` does.
        (
            "map_union_of_constants",
            r#"<?php
final class Fold {
    private const array LOWER = ['à' => 'a', 'é' => 'e'];
    private const array UPPER = ['À' => 'A', 'é' => 'X'];
    public static function show(): string {
        $out = "";
        foreach (self::LOWER + self::UPPER as $k => $v) { $out .= $k . "=" . $v . ";"; }
        return $out;
    }
}
echo Fold::show();"#,
        ),
        // DEC-524 (row 5j): scout TenureSignal.php's shape — a `readonly` property that is NOT promoted,
        // computed once in the constructor body. It lifts to an immutable field assigned in the
        // constructor, which the ctor-once rule accepts, on every leg.
        (
            "readonly_assigned_in_ctor",
            r#"<?php
final readonly class TenureSignal {
    public int $length;
    public function __construct(public string $evidence, ?int $length = null) {
        $this->length = $length ?? strlen($evidence);
    }
}
$a = new TenureSignal("lease");
$b = new TenureSignal("x", 9);
echo $a->length . "|" . $b->length . "|" . $a->evidence;"#,
        ),
        // DEC-515 row 5d: `@var`-declared locals keep their shape — a call-initialized list read by
        // element field, an empty list filled from a binder, and a nullable tuple replaced under a
        // `=== null ||` guard. Compared against the ORIGINAL PHP's stdout on every leg.
        (
            "var_declared_shaped_locals",
            r#"<?php
/** @return list<array{id: int, name: string}> */
function load(): array { return [['id' => 3, 'name' => 'c'], ['id' => 7, 'name' => 'g'], ['id' => 5, 'name' => 'e']]; }
/** @param list<array{id: int, name: string}> $rows */
function best(array $rows): string {
    /** @var array{id: int, name: string}|null $seen */
    $seen = null;
    foreach ($rows as $r) {
        if ($seen === null || $r['id'] > $seen['id']) {
            $seen = ['id' => $r['id'], 'name' => $r['name']];
        }
    }
    if ($seen !== null) { return $seen['name']; }
    return "-";
}
function total(): int {
    /** @var list<array{id: int, name: string}> $rows */
    $rows = load();
    /** @var list<array{id: int, name: string}> $kept */
    $kept = [];
    foreach ($rows as $r) { if ($r['id'] > 4) { $kept[] = $r; } }
    $sum = 0;
    foreach ($kept as $k) { $sum += $k['id']; }
    return $sum + $rows[0]['id'];
}
function none(): string {
    /** @var list<array{id: int, name: string}> $empty */
    $empty = [];
    return best($empty);
}
echo best(load()) . "|" . total() . "|" . none();"#,
        ),
        // DEC-537 row 5t: PHP's first-match idiom — `$seen` is null-tested and assigned in the block
        // the test narrowed. Before row 5t the lifted assignment failed `E-ASSIGN-TYPE` (`to seen: null`).
        (
            "first_match_under_a_null_test",
            r#"<?php
/** @param list<array{id: int, name: string}> $rows */
function first(array $rows): string {
    /** @var array{id: int, name: string}|null $seen */
    $seen = null;
    foreach ($rows as $r) {
        if ($seen === null) {
            $seen = $r;
        }
    }
    if ($seen === null) {
        return "none";
    }
    return $seen['name'];
}
function noRows(): string {
    /** @var list<array{id: int, name: string}> $rows */
    $rows = [];
    return first($rows);
}
function some(): string {
    /** @var list<array{id: int, name: string}> $rows */
    $rows = [['id' => 1, 'name' => 'a'], ['id' => 2, 'name' => 'b']];
    return first($rows);
}
echo some() . "|" . noRows();"#,
        ),
        // DEC-536 row 5s: scout `Rent/Core/Dedup.php`'s per-key write into a list element —
        // `$kept[$i]['tags'][] = …` lifts to `kept[i].tags = List.append(kept[i].tags, …)`, a named-tuple
        // field write through a mutable place.
        (
            "tuple_field_write_through_a_list_element",
            r#"<?php
/** @param list<array{id: int, tags: list<string>}> $rows */
function dedup(array $rows): string {
    /** @var list<array{id: int, tags: list<string>}> $kept */
    $kept = [];
    foreach ($rows as $r) {
        if (count($kept) > 0 && $kept[0]['id'] === $r['id']) {
            $kept[0]['tags'][] = 'dup';
        } else {
            $kept[] = $r;
        }
    }
    return count($kept) . " " . count($kept[0]['tags']) . " " . $kept[0]['tags'][1];
}
function run(): string {
    /** @var list<array{id: int, tags: list<string>}> $rows */
    $rows = [['id' => 1, 'tags' => ['a']], ['id' => 1, 'tags' => ['b']]];
    return dedup($rows);
}
echo run();"#,
        ),
        // DEC-538 (row 5u): PHP spread over lists lifts to `List.concat`/`List.flatten`. The map case
        // overwrites keys 1 and 3 AFTER insertion, so the lifted `Map.values` must keep PHP's
        // insertion order (an overwrite stays in place) — measured before the build, pinned here.
        (
            "spread_over_lists",
            r#"<?php
/**
 * @param list<int> $a
 * @param list<int> $b
 * @return list<int>
 */
function mix(array $a, array $b, int $x): array { return [...$a, $x, ...$b]; }
/**
 * @param list<int> $a
 * @param list<int> $b
 * @return list<int>
 */
function two(array $a, array $b): array { return [...$b, ...$a]; }
/**
 * @param list<string> $xs
 * @return list<string>
 */
function front(array $xs): array {
    /** @var list<string> $r */
    $r = ["c"];
    array_unshift($r, ...$xs);
    return $r;
}
/** @return list<int> */
function merged(): array {
    $m = [3 => [30], 1 => [10, 11], 2 => [20]];
    $m[1] = [99];
    $m[5] = [50];
    $m[3] = [31, 32];
    return array_merge(...array_values($m));
}
/** @param list<int> $xs */
function show(array $xs): string {
    $s = "";
    foreach ($xs as $x) { $s = $s . $x . ","; }
    return $s;
}
/** @param list<string> $xs */
function words(array $xs): string {
    $s = "";
    foreach ($xs as $x) { $s = $s . $x . ","; }
    return $s;
}
function run(): string {
    /** @var list<int> $a */
    $a = [1, 2];
    /** @var list<int> $b */
    $b = [8, 9];
    return show(mix($a, $b, 5)) . "|" . show(two($a, $b)) . "|" . words(front(["a", "b"])) . "|"
        . show(merged());
}
echo run();"#,
        ),
        // Row 5v: `foreach ($xs as [$a, $b])` / `list($a, $b)` lift to phorj's tuple loop, a nested
        // pair included (distinct synthetic binders), and a binder name reused AFTER the loop is a
        // fresh declaration — PHP's binders leak out of the loop, phorj's are loop-scoped.
        (
            "foreach_destructure",
            r#"<?php
/** @return array{string, int} */
function pair(string $w, int $n): array { return [$w, $n]; }
/** @return list<array{string, int}> */
function pairs(): array { return [pair('a', 1), pair('b', 2), pair('c', 3)]; }
function run(): string {
    $s = "";
    foreach (pairs() as [$w, $n]) { $s = $s . $w . $n . ","; }
    foreach (pairs() as list($w, $n)) {
        foreach (pairs() as [$v, $m]) {
            if ($m === $n) { $s = $s . $v; }
        }
    }
    $w = "|end";
    return $s . $w;
}
echo run();"#,
        ),
        (
            "string_escapes",
            r#"<?php
function esc(): string { return "u\u{2019}|x\xE2\x80\x99|o\101|z\08|e\e|v\v|f\f|d\$|q\'|b\{|n\n"; }
function sq(): string { return 'n\n|t\t|z\0|q\'|s\\|d\"'; }
function hole(string $x): string { return "h\{$x}\u{2019}\$"; }
echo esc(); echo "|"; echo sq(); echo "|"; echo hole("X");"#,
        ),
        // Row 4b: a ternary / `match` in a `.` chain lifts into an interpolation hole, whose block
        // braces must be escaped or the draft does not lex. Covers a non-string arm (the hole must
        // stringify `1` exactly as PHP's `.` does) and scout `NtfyChannel.php`'s nested shape — the
        // conditional inside an inner concat that is a call argument inside the outer hole.
        (
            "concat_conditional_holes",
            r#"<?php
function h(string $s): string { return '[' . $s . ']'; }
function tags(string $t): string { return 'Tags: ' . h(($t === '' ? '' : $t . ',') . 'k'); }
function n(bool $b): string { return 'n=' . ($b ? 1 : 2); }
function v(int $x): string { return 'v=' . match ($x) { 1 => 'one', default => 'many' } . '!'; }
echo tags('hot'); echo "|"; echo tags(''); echo "|"; echo n(true); echo "|"; echo n(false);
echo "|"; echo v(1); echo "|"; echo v(7);"#,
        ),
        // DEC-397 hoist: `$b` is first assigned inside an always-executing `if (true)` and read after
        // it. Before the hoist this lifted to a declaration INSIDE the block, so the outer `$b = …`
        // was `E-ASSIGN-UNKNOWN`. The point of putting it HERE rather than only in a unit test is that
        // this harness compares the lifted program's stdout against the ORIGINAL PHP's on all three
        // legs — which is exactly what catches a hoist that compiles but changes the answer.
        (
            "hoist_out_of_an_always_executing_block",
            r#"<?php
function pick(): string {
    if (true) { $b = "first"; }
    $b = "second";
    return $b;
}
echo pick();"#,
        ),
        // LIFT-NS: a namespaced file with an aliased `use`. Before this slice `namespace`/`use` were in
        // the parser's UNSUPPORTED_KW, so this shape — i.e. every Symfony/Laravel/Doctrine file — could
        // not be lifted at all. The namespace must reach the phorj `package` (PascalCase-ized, since
        // `E-PKG-CASE` is enforced) without changing what the program PRINTS on any of the three legs.
        (
            "namespace_and_aliased_use",
            r#"<?php
namespace app\cli_tools;
use App\Support\Helper as H;
function shout(string $s): string { return $s . "!"; }
echo shout("ns");"#,
        ),
        // Row 5g (DEC-523/529): PHP `/` is FLOAT division, phorj's `/` on two ints is not. The values
        // are chosen so PHP's precision-14 echo and phorj's float rendering agree (`1/3` would not):
        // a non-exact quotient, a negative one, an EXACT one (PHP's int 2 and the lift's float 2.0
        // both print `2`), and `/=` so the compound-assign path is compared against real PHP too.
        (
            "float_division",
            r#"<?php
function ratio(int $a, int $b): float { return $a / $b; }
function halvedTwice(int $n): float { $x = (float) $n; $x /= 2; return $x / -2; }
echo ratio(7, 2), "|", ratio(-7, 2), "|", ratio(6, 3), "|", ratio(1, 4), "|", halvedTwice(10);"#,
        ),
        // DEC-532 (row 5l): PHP 8 throw-expressions in the four positions phorj allows — `??`, a ternary
        // branch, a `match` arm — each thrown AND not thrown. Every throw is caught in its own function
        // (the lifter derives no `throws` clause, KNOWN_ISSUES §LIFT-THROWS), so the draft checks.
        (
            "throw_expressions",
            r#"<?php
function setting(?string $value): string {
    try { return $value ?? throw new \RuntimeException("missing"); }
    catch (\RuntimeException $e) { return "default"; }
}
function half(int $n): int {
    try { return $n % 2 === 0 ? intdiv($n, 2) : throw new \InvalidArgumentException("odd"); }
    catch (\InvalidArgumentException $e) { return -1; }
}
function weekday(int $day): string {
    try { return match ($day) { 1 => "Monday", 2 => "Tuesday", default => throw new \LogicException("none") }; }
    catch (\LogicException $e) { return "?"; }
}
echo setting("dark"), "|", setting(null), "|", half(10), "|", half(7), "|", weekday(2), "|", weekday(9);"#,
        ),
        // DEC-531 (row 5k): reserved-word MEMBERS keep their names (`match`, a promoted `type`, the
        // named argument `type:` on `new`), reserved-word LOCALS become `<word>Value` — and the lifted
        // program prints what the original PHP prints, on all three legs.
        // DEC-533 (row 5m): PHP 8.3 `const array` constants lift to phorj collection constants that
        // keep their names — a string map read through `strtr`-style lookups, nested bands, a
        // `string|null` map, a negative scalar — and print what the original PHP prints.
        (
            "collection_constants",
            r#"<?php
final class Text {
    private const array FOLD = ['à' => 'a', 'é' => 'e'];
    public const array BANDS = ['A' => [10, 20], 'B' => [30]];
    private const array NAMES = ['inli' => 'inli', 'x' => null];
    private const int FLOOR = -5;
    public static function fold(string $c): string { return self::FOLD[$c]; }
    public static function band(string $k, int $i): int { return self::BANDS[$k][$i]; }
    public static function name(string $k): string { return self::NAMES[$k] ?? 'none'; }
    public static function floor(): int { return self::FLOOR * 2; }
}
echo Text::fold('é'), "|", Text::band('A', 1), "|", Text::name('x'), "|", Text::name('inli'), "|", Text::floor();"#,
        ),
        (
            "keyword_names",
            r#"<?php
final class Rule {
    public function __construct(public string $type) {}
    public static function match(int $x): int { return $x * 2; }
}
function describe(string $type, int $class): string { $new = "$type/$class"; return $new; }
$r = new Rule(type: "lease");
echo $r->type, "|", Rule::match(4), "|", describe(class: 3, type: "T");"#,
        ),
        (
            "if_elseif_else",
            r#"<?php
function sign(int $n): string {
    if ($n < 0) { return "neg"; } elseif ($n === 0) { return "zero"; } else { return "pos"; }
}
echo sign(-3) . sign(0) . sign(7);"#,
        ),
        (
            "for_loop_string_build",
            r#"<?php
function stars(int $n): string {
    $s = "";
    for ($i = 0; $i < $n; $i++) { $s = $s . "*"; }
    return $s;
}
echo stars(5);"#,
        ),
        (
            "class_ctor_method",
            r#"<?php
class Box {
    public function __construct(private string $v) {}
    public function get(): string { return $this->v; }
}
$b = new Box("boxed");
echo $b->get();"#,
        ),
        (
            "match_strings",
            r#"<?php
function name(int $c): string {
    return match ($c) { 0 => "red", 1 => "green", 2 => "blue", default => "?" };
}
echo name(1) . name(9);"#,
        ),
        (
            // A-6 ↔ lift: a keyless PHP `foreach ($xs as $x)` lifts to Phorj `foreach (xs as x)`
            // (element type inferred) and round-trips behavior-identically.
            "foreach",
            r#"<?php
function joined(): string {
    $xs = ["a", "b", "c"];
    $out = "";
    foreach ($xs as $x) { $out = $out . $x; }
    return $out;
}
echo joined();"#,
        ),
        (
            // C-1: PHP double-quoted interpolation — simple `$var`, simple public `$o->prop`, and
            // complex `{$o->method()}` — lifts to Phorj `"{…}"` holes and round-trips identically.
            "string_interpolation",
            r#"<?php
class Box {
    public function __construct(public string $label) {}
    public function next(): int { return 42; }
}
function describe(string $who, Box $b): string {
    return "Hi $who, label=$b->label next={$b->next()}";
}
$b = new Box("crate");
echo describe("Ada", $b);"#,
        ),
        (
            // Contextual `var`: a PHP variable/parameter literally named `$var` lifts to a Phorj
            // value named `var` (not mangled) and round-trips — the original motivating bug. PHP
            // allows `$var`; Phorj now does too (`var` is the inference keyword only at a binding
            // start). Here `$var` is a parameter (read) and also a top-level local (a `var var = …`
            // declaration), both kept verbatim.
            "var_as_identifier",
            r#"<?php
function tag(string $var): string {
    return "[" . $var . "]";
}
$var = tag("hi");
echo $var;"#,
        ),
    ];

    for (label, php_src) in cases {
        roundtrip(&php, label, php_src);
    }
}
