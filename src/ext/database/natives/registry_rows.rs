//! The Row-accessor `Core.Native.Database` registry entries (DEC-208 slices B/E/K + S2): the strict and
//! nullable scalar getters, the exact-money decimal getters, the typed array-column getters, and the
//! `columnNames`/`isNull` introspection primitives. Split out of [`super::registry`] for the file-size
//! cap (Invariant 13); assembled after the connection/statement natives by
//! [`super::registry::database_natives`].

use super::registry::{handle, res};
use super::wrappers::*;
use crate::native::{NativeEval, NativeFn};
use crate::types::Ty;

/// The Row-accessor natives (getters + column introspection).
pub(super) fn row_natives() -> Vec<NativeFn> {
    vec![
        NativeFn {
            module: "Core.Native.Database",
            name: "getInt",
            params: vec![handle(), Ty::String],
            ret: res(Ty::Int),
            pure: false,
            eval: NativeEval::Pure(row_get_int),
            lift_from: &[],
            php: |a| format!("(int) {}[{}]", a[0], a[1]),
        },
        // Typed ARRAY-column accessors (DEC-208 slice K): Postgres `int[]`/`text[]`/`float8[]`/
        // `bool[]` cells → typed `List<scalar>` (strict; NULL elements rejected; `OrNull` admits a
        // whole-array NULL). PHP emitters are placeholders (Core.DatabaseModule is E-TRANSPILE-DB native-only).
        NativeFn {
            module: "Core.Native.Database",
            name: "getIntList",
            params: vec![handle(), Ty::String],
            ret: res(Ty::List(Box::new(Ty::Int))),
            pure: false,
            eval: NativeEval::Pure(row_get_int_list),
            lift_from: &[],
            php: |a| a[0].clone(),
        },
        NativeFn {
            module: "Core.Native.Database",
            name: "getStringList",
            params: vec![handle(), Ty::String],
            ret: res(Ty::List(Box::new(Ty::String))),
            pure: false,
            eval: NativeEval::Pure(row_get_string_list),
            lift_from: &[],
            php: |a| a[0].clone(),
        },
        NativeFn {
            module: "Core.Native.Database",
            name: "getFloatList",
            params: vec![handle(), Ty::String],
            ret: res(Ty::List(Box::new(Ty::Float))),
            pure: false,
            eval: NativeEval::Pure(row_get_float_list),
            lift_from: &[],
            php: |a| a[0].clone(),
        },
        NativeFn {
            module: "Core.Native.Database",
            name: "getBoolList",
            params: vec![handle(), Ty::String],
            ret: res(Ty::List(Box::new(Ty::Bool))),
            pure: false,
            eval: NativeEval::Pure(row_get_bool_list),
            lift_from: &[],
            php: |a| a[0].clone(),
        },
        NativeFn {
            module: "Core.Native.Database",
            name: "getIntListOrNull",
            params: vec![handle(), Ty::String],
            ret: res(Ty::Optional(Box::new(Ty::List(Box::new(Ty::Int))))),
            pure: false,
            eval: NativeEval::Pure(row_get_int_list_or_null),
            lift_from: &[],
            php: |a| a[0].clone(),
        },
        NativeFn {
            module: "Core.Native.Database",
            name: "getStringListOrNull",
            params: vec![handle(), Ty::String],
            ret: res(Ty::Optional(Box::new(Ty::List(Box::new(Ty::String))))),
            pure: false,
            eval: NativeEval::Pure(row_get_string_list_or_null),
            lift_from: &[],
            php: |a| a[0].clone(),
        },
        NativeFn {
            module: "Core.Native.Database",
            name: "getFloatListOrNull",
            params: vec![handle(), Ty::String],
            ret: res(Ty::Optional(Box::new(Ty::List(Box::new(Ty::Float))))),
            pure: false,
            eval: NativeEval::Pure(row_get_float_list_or_null),
            lift_from: &[],
            php: |a| a[0].clone(),
        },
        NativeFn {
            module: "Core.Native.Database",
            name: "getBoolListOrNull",
            params: vec![handle(), Ty::String],
            ret: res(Ty::Optional(Box::new(Ty::List(Box::new(Ty::Bool))))),
            pure: false,
            eval: NativeEval::Pure(row_get_bool_list_or_null),
            lift_from: &[],
            php: |a| a[0].clone(),
        },
        NativeFn {
            module: "Core.Native.Database",
            name: "getString",
            params: vec![handle(), Ty::String],
            ret: res(Ty::String),
            pure: false,
            eval: NativeEval::Pure(row_get_string),
            lift_from: &[],
            php: |a| format!("(string) {}[{}]", a[0], a[1]),
        },
        NativeFn {
            module: "Core.Native.Database",
            name: "getFloat",
            params: vec![handle(), Ty::String],
            ret: res(Ty::Float),
            pure: false,
            eval: NativeEval::Pure(row_get_float),
            lift_from: &[],
            php: |a| format!("(float) {}[{}]", a[0], a[1]),
        },
        NativeFn {
            module: "Core.Native.Database",
            name: "getBool",
            params: vec![handle(), Ty::String],
            ret: res(Ty::Bool),
            pure: false,
            eval: NativeEval::Pure(row_get_bool),
            lift_from: &[],
            php: |a| format!("(bool) {}[{}]", a[0], a[1]),
        },
        // Nullable accessors (DEC-208 S2): a NULL column yields `null`; a wrong non-null type is still
        // a DB error. `ret` is `DatabaseResult<T?>` so the prelude method types as `T?`.
        NativeFn {
            module: "Core.Native.Database",
            name: "getIntOrNull",
            params: vec![handle(), Ty::String],
            ret: res(Ty::Optional(Box::new(Ty::Int))),
            pure: false,
            eval: NativeEval::Pure(row_get_int_or_null),
            lift_from: &[],
            php: |a| format!("(({0}[{1}] === null) ? null : (int) {0}[{1}])", a[0], a[1]),
        },
        NativeFn {
            module: "Core.Native.Database",
            name: "getStringOrNull",
            params: vec![handle(), Ty::String],
            ret: res(Ty::Optional(Box::new(Ty::String))),
            pure: false,
            eval: NativeEval::Pure(row_get_string_or_null),
            lift_from: &[],
            php: |a| {
                format!(
                    "(({0}[{1}] === null) ? null : (string) {0}[{1}])",
                    a[0], a[1]
                )
            },
        },
        NativeFn {
            module: "Core.Native.Database",
            name: "getFloatOrNull",
            params: vec![handle(), Ty::String],
            ret: res(Ty::Optional(Box::new(Ty::Float))),
            pure: false,
            eval: NativeEval::Pure(row_get_float_or_null),
            lift_from: &[],
            php: |a| {
                format!(
                    "(({0}[{1}] === null) ? null : (float) {0}[{1}])",
                    a[0], a[1]
                )
            },
        },
        NativeFn {
            module: "Core.Native.Database",
            name: "getBoolOrNull",
            params: vec![handle(), Ty::String],
            ret: res(Ty::Optional(Box::new(Ty::Bool))),
            pure: false,
            eval: NativeEval::Pure(row_get_bool_or_null),
            lift_from: &[],
            php: |a| format!("(({0}[{1}] === null) ? null : (bool) {0}[{1}])", a[0], a[1]),
        },
        // Decimal accessor (DEC-208 slice E): a `decimal`-typed hydration field maps its column here
        // (exact money — TEXT parsed exactly, never through float). PHP emitters are placeholders
        // (Core.DatabaseModule is spine-quarantined; the transpile is finalized in a later slice).
        NativeFn {
            module: "Core.Native.Database",
            name: "getDecimal",
            params: vec![handle(), Ty::String],
            ret: res(Ty::Decimal),
            pure: false,
            eval: NativeEval::Pure(row_get_decimal),
            lift_from: &[],
            php: |a| format!("__phorj_dec_of((string) {}[{}])", a[0], a[1]),
        },
        NativeFn {
            module: "Core.Native.Database",
            name: "getDecimalOrNull",
            params: vec![handle(), Ty::String],
            ret: res(Ty::Optional(Box::new(Ty::Decimal))),
            pure: false,
            eval: NativeEval::Pure(row_get_decimal_or_null),
            lift_from: &[],
            php: |a| {
                format!(
                    "(({0}[{1}] === null) ? null : __phorj_dec_of((string) {0}[{1}]))",
                    a[0], a[1]
                )
            },
        },
        // Column introspection (DEC-208 slice B). `columnNames` → ordered `List<string>`; `isNull` →
        // `bool`. Used by the `queryScalar`/`queryMap`/nested-hydration desugar; PHP emitters are
        // placeholders (Core.DatabaseModule is spine-quarantined, transpile finalized in a later slice).
        NativeFn {
            module: "Core.Native.Database",
            name: "columnNames",
            params: vec![handle()],
            ret: res(Ty::List(Box::new(Ty::String))),
            pure: false,
            eval: NativeEval::Pure(row_column_names),
            lift_from: &[],
            php: |a| format!("array_keys({})", a[0]),
        },
        NativeFn {
            module: "Core.Native.Database",
            name: "isNull",
            params: vec![handle(), Ty::String],
            ret: res(Ty::Bool),
            pure: false,
            eval: NativeEval::Pure(row_is_null),
            lift_from: &[],
            php: |a| format!("({0}[{1}] === null)", a[0], a[1]),
        },
    ]
}
