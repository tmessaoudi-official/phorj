//! The `ini` extension (DEC-273 PILOT — the first physically-migrated `src/ext/<name>/` folder):
//! `Core.Ini` config parsing. Natives, tests, and (when one exists) prelude source live here,
//! colocated; the registry row is `ext::registry::EXTENSIONS`'s `"ini"` entry; build inclusion is
//! the `ini` Cargo feature (Default tier — in the batteries-included build).

mod natives;
#[cfg(test)]
mod tests;

pub use natives::ini_natives;
