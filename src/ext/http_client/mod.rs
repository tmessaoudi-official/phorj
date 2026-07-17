//! The `http_client` extension (DEC-273 wave 3): natives + tests colocated per AMENDMENT 2.
//! Registry row: `ext::registry::EXTENSIONS`; build inclusion = its Cargo feature.

pub mod natives;
#[cfg(test)]
mod tests;

pub use natives::http_client_natives;
