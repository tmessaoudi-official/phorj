//! The `database` extension (DEC-273 wave 3) — `Core.DatabaseModule`: multi-driver SQL natives
//! (bundled SQLite default; MySQL/Postgres behind their own flags), colocated per AMENDMENT 2.
//! The prelude source is `crate::ext::database_prelude`; registry row `"database"`.

pub mod natives;

pub use natives::database_natives;
