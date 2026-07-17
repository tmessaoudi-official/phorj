//! The `debug` extension (DEC-273 wave 2): natives + tests colocated per AMENDMENT 2.
//! Registry row: `ext::registry::EXTENSIONS`'s `"debug"` entry; build inclusion = the `debug`
//! Cargo feature.

pub mod natives;
#[cfg(test)]
mod tests;

pub use natives::debug_natives;
