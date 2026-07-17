//! The `test` extension (DEC-273 wave 2): natives + tests colocated per AMENDMENT 2.
//! Registry row: `ext::registry::EXTENSIONS`'s `"test"` entry; build inclusion = the `test`
//! Cargo feature.

pub mod natives;
#[cfg(test)]
mod tests;

pub use natives::test_natives;
