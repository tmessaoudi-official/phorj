//! `impl Checker` — program cluster (M-Decomp W2), split by pass concern. See
//! checker/mod.rs for the struct + entry points.

use super::*;

mod attributes;
mod totality;
mod type_bodies;
mod walk;
