//! The `db` extension (DEC-273 wave 3) — `Core.DatabaseModule`: multi-driver SQL natives
//! (bundled SQLite default; MySQL/Postgres behind their own flags), colocated per AMENDMENT 2.
//! The prelude source is `crate::ext::db_prelude`; registry row `"db"`.

pub mod natives;

pub use natives::db_natives;
