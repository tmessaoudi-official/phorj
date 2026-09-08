//! `impl Checker` — expr cluster (M-Decomp W2), split by expression family.

use super::*;

mod core;
mod lambda;
mod literals;
mod operators;
mod ordering;
mod tuple_literal;
