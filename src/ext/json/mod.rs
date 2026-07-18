//! The `json` extension (DEC-273 wave 2): natives + tests colocated per AMENDMENT 2.
//! Registry row: `ext::registry::EXTENSIONS`'s `"json"` entry; build inclusion = the `json`
//! Cargo feature.

pub mod natives;
#[cfg(test)]
mod tests;

pub use natives::json_natives;
pub use natives::{materialize_if_lazy, materialize_lazy};
