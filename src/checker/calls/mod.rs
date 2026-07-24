//! `impl Checker` — calls cluster (M-Decomp W2), split by call form. See checker/mod.rs
//! for the struct + entry points.

use super::*;

mod args;
mod core;
mod format;
mod invoke;
mod methods;
mod overloads;
mod ufcs;
mod variants;

pub(super) use self::overloads::MethodSig;
use self::ufcs::UfcsNav;
