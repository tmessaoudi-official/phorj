//! The `path` extension (DEC-273 wave 2): natives + tests colocated per AMENDMENT 2.
//! Registry row: `ext::registry::EXTENSIONS`'s `"path"` entry; build inclusion = the `path`
//! Cargo feature.

pub mod natives;
#[cfg(test)]
mod tests;

pub use natives::path_natives;
