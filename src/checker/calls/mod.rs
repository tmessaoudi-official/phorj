//! `impl Checker` — calls cluster (M-Decomp W2), split by call form. See checker/mod.rs
//! for the struct + entry points.

use super::*;

mod args;
mod core;
mod dispatch_intersection;
mod dispatch_named;
mod format;
mod invoke;
mod lint;
mod member;
mod methods;
mod overloads;
mod subst;
mod ufcs;
mod variants;
mod visibility;

pub(super) use self::overloads::MethodSig;
use self::ufcs::UfcsNav;
