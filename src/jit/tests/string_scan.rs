//! String-SCAN verticals (DEC-332 stringcontains/isemail/isurl flips) — delivery-path proofs
//! (`hits>0` on each bench shape: the dedicated zero-alloc helpers, not the boxed bridge) +
//! the edges: rotating hit/miss needles, empty needle (always contained), needle longer than
//! the haystack, valid/invalid probes across both the ≤22-byte arena-slot and the longer
//! untagged-handle string representations. Sibling of `hof_filter_map.rs` (Invariant 13).

use super::*;

fn assert_jit_hits(src: &str, label: &str) -> String {
    let jit_out = crate::cli::cmd_run(src).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(src).expect("interpreter oracle ok");
    assert_eq!(jit_out, oracle, "{label}: jit output must match the oracle");
    let program = compile_source(src);
    let cache = std::rc::Rc::new(std::cell::RefCell::new(crate::vm::JitCache::new()));
    let manual = crate::vm::Vm::new(&program)
        .with_jit(cache.clone())
        .run()
        .expect("manual jit-wired run ok");
    assert_eq!(manual, oracle, "{label}: manual jit output must match");
    assert!(
        cache.borrow().hits > 0,
        "{label}: must actually hit the JIT — else the perf flip is unproven"
    );
    jit_out
}

#[test]
fn phg_run_hook_hits_the_jit_on_the_stringcontains_vertical() {
    // The exact `bench/micro/stringcontains.phg` shape: constant haystack, rotating needles
    // (hits and misses), the checksum counts hits.
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.List;\n\
        import Core.String;\n\
        function bench(int iters): int {\n\
          string hay = \"the quick brown fox jumps over the lazy dog\";\n\
          List<string> needles = [\"fox\", \"cat\", \"lazy\", \"zzz\", \"the\", \"qux\"];\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            if (String.contains(hay, needles[i % 6])) {\n\
              acc = acc + 1;\n\
            }\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(1800)}\"); }";
    assert_jit_hits(SRC, "stringcontains vertical");
}

#[test]
fn jit_stringcontains_edge_needles_match_the_oracle() {
    // Empty needle (always true), needle == haystack, needle LONGER than the haystack
    // (always false), and a needle crossing the 22-byte slot/untagged representation line.
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.List;\n\
        import Core.String;\n\
        function bench(int iters): int {\n\
          string hay = \"abcdef\";\n\
          List<string> needles = [\"\", \"abcdef\", \"abcdefg\", \"this needle is far longer than the haystack\", \"cde\"];\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            if (String.contains(hay, needles[i % 5])) {\n\
              acc = acc + 1;\n\
            }\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(600)}\"); }";
    assert_jit_hits(SRC, "stringcontains edges");
}

#[test]
fn phg_run_hook_hits_the_jit_on_the_isemail_vertical() {
    // The exact `bench/micro/isemail.phg` shape: two valid, four invalid probes (consecutive
    // dots, no @, dotless domain), several longer than the 22-byte slot cap.
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.List;\n\
        import Core.Validation;\n\
        function bench(int iters): int {\n\
          List<string> probes = [\n\
            \"a@b.co\",\n\
            \"user@localhost\",\n\
            \"a..b@c.com\",\n\
            \"x.y+z@mail.example.org\",\n\
            \"no-at-sign\",\n\
            \"bad@dom..com\"\n\
          ];\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            if (Validation.isEmail(probes[i % 6])) {\n\
              acc = acc + 1;\n\
            }\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(1800)}\"); }";
    assert_jit_hits(SRC, "isemail vertical");
}

#[test]
fn phg_run_hook_hits_the_jit_on_the_isurl_vertical() {
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.List;\n\
        import Core.Validation;\n\
        function bench(int iters): int {\n\
          List<string> probes = [\n\
            \"https://example.org/path?q=1\",\n\
            \"http://a.b\",\n\
            \"ftp://nope.example\",\n\
            \"https://\",\n\
            \"not a url at all\",\n\
            \"https://sub.domain.example.co.uk/deep/path\"\n\
          ];\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            if (Validation.isUrl(probes[i % 6])) {\n\
              acc = acc + 1;\n\
            }\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(1800)}\"); }";
    assert_jit_hits(SRC, "isurl vertical");
}

#[test]
fn jit_string_memo_survives_direct_mapped_collisions() {
    // 12 distinct (hay, needle) pairs + interleaved isEmail probes share the 8-entry
    // direct-mapped memo region: colliding lines EVICT and re-install from the full memo —
    // results must stay exact through every eviction round.
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.List;\n\
        import Core.String;\n\
        import Core.Validation;\n\
        function bench(int iters): int {\n\
          string hay = \"the quick brown fox jumps over the lazy dog\";\n\
          List<string> needles = [\"fox\", \"cat\", \"lazy\", \"zzz\", \"the\", \"qux\",\n\
                                  \"dog\", \"own f\", \"jumps\", \"x j\", \"over\", \"nope\"];\n\
          List<string> mails = [\"a@b.co\", \"nope\", \"x.y@z.io\"];\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            if (String.contains(hay, needles[i % 12])) {\n\
              acc = acc + 2;\n\
            }\n\
            if (Validation.isEmail(mails[i % 3])) {\n\
              acc = acc + 1;\n\
            }\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(1200)}\"); }";
    assert_jit_hits(SRC, "string memo collisions");
}

#[test]
fn jit_stringcontains_still_works_through_interpolated_owned_strings() {
    // An OWNED haystack built per iteration (interpolation → accumulator record) exercises the
    // helper's ACC-record byte read + the free_mask release of an owned operand.
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.String;\n\
        function bench(int iters): int {\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            string hay = \"row-{i % 10}-tail\";\n\
            if (String.contains(hay, \"5-ta\")) {\n\
              acc = acc + 1;\n\
            }\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(600)}\"); }";
    assert_jit_hits(SRC, "stringcontains owned haystack");
}
