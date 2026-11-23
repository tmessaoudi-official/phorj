//! `impl Checker` — stmt cluster, split by statement family.

use super::*;

mod core;
mod flow;
mod jumps;
mod literals;
mod narrow;
mod using;
mod var_decl;
