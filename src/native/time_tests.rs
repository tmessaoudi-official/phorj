use super::*;
use crate::value::Value;
use std::sync::Mutex;

// `FROZEN` is a process global, so these tests must not interleave their freeze/unfreeze calls.
static CLOCK_LOCK: Mutex<()> = Mutex::new(());

fn now() -> i64 {
    match time_now_millis(&[], &mut String::new()).unwrap() {
        Value::Int(n) => n,
        other => panic!("expected int, got {other:?}"),
    }
}
fn freeze(ms: i64) {
    time_freeze(&[Value::Int(ms)], &mut String::new()).unwrap();
}
fn unfreeze() {
    time_unfreeze(&[], &mut String::new()).unwrap();
}

#[test]
fn frozen_clock_is_deterministic() {
    let _g = CLOCK_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    freeze(1_700_000_000_000);
    assert_eq!(now(), 1_700_000_000_000);
    assert_eq!(now(), 1_700_000_000_000, "frozen clock must not advance");
    unfreeze();
}

#[test]
fn unfrozen_clock_reads_wall_clock() {
    let _g = CLOCK_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    unfreeze();
    // A real wall clock in 2026 is well past 2020-01-01 (1_577_836_800_000 ms).
    assert!(now() > 1_577_836_800_000, "wall clock must be after 2020");
}

#[test]
fn refreeze_overrides() {
    let _g = CLOCK_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    freeze(1000);
    assert_eq!(now(), 1000);
    freeze(2000);
    assert_eq!(now(), 2000);
    unfreeze();
}

#[test]
fn arity_errors() {
    let _g = CLOCK_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    assert!(time_now_millis(&[Value::Int(1)], &mut String::new()).is_err());
    assert!(time_freeze(&[], &mut String::new()).is_err());
    assert!(time_unfreeze(&[Value::Int(1)], &mut String::new()).is_err());
    unfreeze();
}

// ── Time.sleep (DEC-487) ───────────────────────────────────────────────────────────────────────

/// Build the `Duration` value the native receives — an instance with an int `ms`, matching what
/// `TIME_PRELUDE`'s `class Duration { constructor(public int ms) {} }` produces.
#[cfg(test)]
fn duration(ms: i64) -> Value {
    let inst = crate::value::Instance::new(
        "Duration".into(),
        crate::value::ClassLayout::from_sorted_names(&["ms"]),
    );
    inst.set_field("ms", Value::Int(ms));
    Value::Instance(std::rc::Rc::new(inst))
}

/// The frozen-clock NO-OP is the property that keeps shipped examples instant and deterministic. If
/// this regresses, every example carrying a sleep starts really sleeping and the differential's
/// wall-clock cost explodes — slowly enough that nobody attributes it.
#[test]
fn a_frozen_clock_makes_sleep_a_no_op() {
    let _g = CLOCK_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    time_freeze(&[Value::Int(1_700_000_000_000)], &mut String::new()).unwrap();
    let t0 = std::time::Instant::now();
    // An hour, which would obviously be noticed if it were honoured.
    super::time_sleep(&[duration(3_600_000)], &mut String::new()).unwrap();
    assert!(
        t0.elapsed() < std::time::Duration::from_millis(200),
        "a frozen clock must make sleep free, took {:?}",
        t0.elapsed()
    );
    time_unfreeze(&[], &mut String::new()).unwrap();
}

/// Non-positive durations return at once rather than faulting — `Duration.minus` can go negative.
#[test]
fn zero_and_negative_durations_return_immediately() {
    let t0 = std::time::Instant::now();
    super::time_sleep(&[duration(0)], &mut String::new()).unwrap();
    super::time_sleep(&[duration(-5_000)], &mut String::new()).unwrap();
    assert!(t0.elapsed() < std::time::Duration::from_millis(200));
}

/// An unfrozen sleep really waits. Deliberately a *lower* bound only: an upper bound would be a
/// flaky assertion about scheduler latency on a loaded box.
#[test]
fn an_unfrozen_sleep_actually_waits() {
    let _g = CLOCK_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    crate::shutdown::reset_for_test();
    let t0 = std::time::Instant::now();
    super::time_sleep(&[duration(120)], &mut String::new()).unwrap();
    assert!(
        t0.elapsed() >= std::time::Duration::from_millis(100),
        "expected to wait ~120ms, took {:?}",
        t0.elapsed()
    );
}

/// The interruptibility half of DEC-487: a signalled process wakes a sleeper instead of making it
/// serve out its sentence. This is why the native polls in slices rather than issuing one long
/// `thread::sleep`, and why `crate::shutdown` owns the single ctrlc registration.
#[test]
fn a_signalled_process_wakes_a_sleeper_early() {
    let _g = CLOCK_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    crate::shutdown::raise_for_test();
    let t0 = std::time::Instant::now();
    // Ten seconds, which the test would obviously not tolerate if the flag were ignored.
    super::time_sleep(&[duration(10_000)], &mut String::new()).unwrap();
    assert!(
        t0.elapsed() < std::time::Duration::from_millis(500),
        "a signalled process must not sleep on, took {:?}",
        t0.elapsed()
    );
    crate::shutdown::reset_for_test();
}

/// Arity and shape errors are reported, not panicked — a native is reachable from user code.
#[test]
fn sleep_rejects_a_non_duration() {
    assert!(super::time_sleep(&[Value::Int(5)], &mut String::new()).is_err());
    assert!(super::time_sleep(&[], &mut String::new()).is_err());
}
