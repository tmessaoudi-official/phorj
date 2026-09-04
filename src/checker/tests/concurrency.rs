//! `spawn` / `Task<T>` / `Channel<T>` diagnostics. The runtime half is byte-identical across the two
//! native backends and quarantined from PHP (`E-CONCURRENCY-NO-PHP`, already asserted); these pin
//! the checker's refusals, none of which had an assertion before.
use super::support::*;

fn has(src: &str, code: &str) {
    let e = errors_of(src);
    assert!(
        e.iter().any(|d| d.code == Some(code)),
        "expected {code}, got {e:?}"
    );
}

#[test]
fn spawn_needs_a_call() {
    // `spawn` is a contextual keyword recognised only before an identifier, so the non-call operand
    // the checker refuses is a bare name, never a literal (`spawn 42` is a parse error).
    has(
        "function work() -> int { return 1; } function main() -> void { Task<int> t = spawn work; }",
        "E-SPAWN-NOT-CALL",
    );
}

#[test]
fn a_spawned_call_must_return_a_value() {
    has(
        "function work() -> void { } function main() -> void { Task<int> t = spawn work(); }",
        "E-SPAWN-VOID",
    );
}

#[test]
fn channel_create_needs_an_annotation_to_fix_t() {
    has(
        "function main() -> void { var ch = Channel.create(); }",
        "E-CHANNEL-ANNOTATION",
    );
}

#[test]
fn channel_has_no_other_static_method() {
    has(
        "function main() -> void { var ch = Channel.make(); }",
        "E-CONCURRENCY-METHOD",
    );
}

#[test]
fn send_takes_exactly_one_argument() {
    has(
        "function main() -> void { Channel<int> ch = Channel.create(); ch.send(1, 2); }",
        "E-CONCURRENCY-ARITY",
    );
}

#[test]
fn receive_and_join_take_no_arguments() {
    has(
        "function main() -> void { Channel<int> ch = Channel.create(); int v = ch.receive(1); }",
        "E-CONCURRENCY-ARITY",
    );
    has(
        "function work() -> int { return 1; } function main() -> void { Task<int> t = spawn work(); int v = t.join(1); }",
        "E-CONCURRENCY-ARITY",
    );
}

#[test]
fn channel_new_takes_no_arguments_and_yields_a_channel() {
    // The codes keep their M6 W4 names; the constructor they guard is `Channel.create()` — `new`
    // is a keyword and cannot be a member name — which is what `is_channel_new` matches.
    has(
        "function main() -> void { Channel<int> ch = Channel.create(1); }",
        "E-CHANNEL-NEW-ARITY",
    );
    has(
        "function main() -> void { int ch = Channel.create(); }",
        "E-CHANNEL-NEW-TYPE",
    );
}
