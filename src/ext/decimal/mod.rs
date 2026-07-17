//! The `decimal` extension (DEC-273 wave 2): natives + tests colocated per AMENDMENT 2.
//! Registry row: `ext::registry::EXTENSIONS`'s `"decimal"` entry; build inclusion = the `decimal`
//! Cargo feature.

pub mod natives;
#[cfg(test)]
mod tests;

pub use natives::decimal_natives;
