//! `impl Checker` — collect cluster (M-Decomp W2), split by declaration family. See
//! checker/mod.rs for the struct + entry points.

use super::*;

mod entry;
mod functions;
mod inherit;
mod interfaces;
mod types_decls;
