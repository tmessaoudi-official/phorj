//! Opaque native database resource handles (DEC-208 `Core.DatabaseModule`, the enhanced-PDO primitive).
//!
//! The rusqlite-backed concrete handles (a connection; a lazily-executed prepared statement) live
//! behind `#[cfg(feature = "database")]` in `src/ext/database/natives.rs`. Only this trait and the [`Value::Db`]
//! variant are compiled unconditionally, so the value-model match arms (`type_name`, `eq_val`, the
//! backends' exhaustive `Value` dispatches) never `cfg`-split. With the `database` feature off there are no
//! implementors and the `Core.DatabaseModule` module is not registered, so the variant is unconstructable.
//!
//! A DB handle is a **shared-mutable opaque resource** like [`crate::value::Value::Channel`]: cloning
//! shares the same `Rc` (a statement's accumulated binds are visible through every clone), and it is
//! opaque to the arithmetic / compare / display kernels (the checker forbids using a handle as an
//! operand or interpolating it). Quarantined from the byte-identity differential (`Core.DatabaseModule` natives
//! are `pure: false`); correctness is validated by the `tests/database.rs` fixture, and the transpiler emits
//! faithful PDO (DEC-208, LADDER case 1).

use std::any::Any;
use std::fmt::Debug;

/// An opaque `Core.DatabaseModule` resource handle carried by [`crate::value::Value::Db`]. Implementors are the
/// rusqlite-backed connection / statement types (feature-gated in `src/ext/database/natives.rs`). `as_any` lets a
/// `Core.DatabaseModule` native downcast the type-erased handle back to its concrete type to perform the operation
/// (the same erase-then-downcast shape the value model uses for any opaque native resource).
pub trait DbObject: Debug {
    /// Diagnostic kind — `"db-connection"` or `"db-statement"`. Surfaced by `Value::type_name`.
    fn kind(&self) -> &'static str;
    /// Downcast hook: the concrete handle behind the trait object, for a native to operate on.
    fn as_any(&self) -> &dyn Any;
}
