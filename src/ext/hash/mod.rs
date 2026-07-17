//! The `hash` extension (DEC-273 wave 2): natives + tests colocated per AMENDMENT 2.
//! Registry row: `ext::registry::EXTENSIONS`'s `"hash"` entry; build inclusion = the `hash`
//! Cargo feature.

pub mod natives;
#[cfg(test)]
mod tests;

pub use natives::hash_natives;
