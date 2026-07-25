//! `impl Checker` — collect cluster (M-Decomp W2), split by declaration family. See
//! checker/mod.rs for the struct + entry points.

use super::*;

mod abstract_traits;
mod class_graph;
mod conformance;
mod entry;
mod functions;
mod inherit;
mod interfaces;
mod overrides;
mod types_decls;
