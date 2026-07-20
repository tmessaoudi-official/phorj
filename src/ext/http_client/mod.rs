//! The `http_client` extension (DEC-273 wave 3): natives + tests colocated per AMENDMENT 2.
//! Registry row: `ext::registry::EXTENSIONS`; build inclusion = its Cargo feature.

mod engine;
pub mod natives;
mod protocol;
#[cfg(test)]
mod tests;

pub use natives::http_client_natives;
