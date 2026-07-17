//! The `cryptography` extension (DEC-273 migration wave): natives + tests colocated per AMENDMENT 2.
//! Registry row: `ext::registry::EXTENSIONS`'s `"cryptography"` entry; build inclusion = the
//! `cryptography` Cargo feature.

pub mod natives;
#[cfg(test)]
mod tests;

pub use natives::crypto_natives;
