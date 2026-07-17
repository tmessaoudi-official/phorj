//! The `crypto` extension (DEC-273 migration wave): natives + tests colocated per AMENDMENT 2.
//! Registry row: `ext::registry::EXTENSIONS`'s `"crypto"` entry; build inclusion = the `crypto`
//! Cargo feature.

pub mod natives;
#[cfg(test)]
mod tests;

pub use natives::crypto_natives;
