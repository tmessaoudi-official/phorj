//! The `json` extension (DEC-273 wave 2): natives + tests colocated per AMENDMENT 2.
//! Registry row: `ext::registry::EXTENSIONS`'s `"json"` entry; build inclusion = the `json`
//! Cargo feature.

mod encode;
pub mod natives;
mod parser;
#[cfg(test)]
mod tests;

pub use natives::json_natives;
pub(crate) use natives::json_parse_str;
pub use natives::materialize_if_lazy;
pub use parser::materialize_lazy;
