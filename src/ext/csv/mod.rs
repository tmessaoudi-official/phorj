//! The `csv` extension (DEC-273 migration wave): natives + tests colocated per AMENDMENT 2.
//! Registry row: `ext::registry::EXTENSIONS`'s `"csv"` entry; build inclusion = the `csv`
//! Cargo feature.

pub mod natives;
#[cfg(test)]
mod tests;

pub use natives::csv_natives;
