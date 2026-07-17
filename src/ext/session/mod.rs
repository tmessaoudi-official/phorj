//! The `session` extension (DEC-273 wave 3): natives colocated per AMENDMENT 2.
//! Registry row: `ext::registry::EXTENSIONS`; build inclusion = its Cargo feature.

pub mod natives;

pub use natives::session_natives;
