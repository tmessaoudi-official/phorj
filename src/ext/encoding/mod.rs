//! The `encoding` extension (DEC-273 migration wave): natives + tests colocated per AMENDMENT 2.
//! Registry row: `ext::registry::EXTENSIONS`'s `"encoding"` entry; build inclusion = the `encoding`
//! Cargo feature.

pub mod natives;
#[cfg(test)]
mod tests;

pub use natives::encoding_natives;
