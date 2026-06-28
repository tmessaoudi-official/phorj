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
