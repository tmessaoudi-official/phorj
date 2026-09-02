//! The `regex` extension (DEC-273 migration wave): natives + tests colocated per AMENDMENT 2.
//! Registry row: `ext::registry::EXTENSIONS`'s `"regex"` entry; build inclusion = the `regex`
//! Cargo feature.

pub mod engine;
pub mod natives;
pub mod reject;
pub mod replace;
#[cfg(test)]
mod tests;

pub use natives::regex_natives;
