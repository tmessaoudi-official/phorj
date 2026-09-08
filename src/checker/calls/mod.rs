//! `impl Checker` — calls cluster (M-Decomp W2), split by call form. See checker/mod.rs
//! for the struct + entry points.

use super::*;

pub(in crate::checker) mod args;
mod core;
mod dispatch_intersection;
mod dispatch_named;
mod format;
mod invoke;
mod lint;
mod member;
mod methods;
mod overloads;
mod regex;
mod subst;
mod tuple_field;
mod ufcs;
mod unify;
mod variants;
mod visibility;

pub(super) use self::overloads::MethodSig;
use self::ufcs::UfcsNav;
