//! The registry's module-registration list (M-Decomp from `mod.rs`, Inv 13 — content unchanged,
//! one `extend` per module family; feature-gated families keep their `cfg`s). Adding a stdlib
//! module = one line here.
use super::NativeFn;

pub(super) fn extend_module_natives(registry: &mut Vec<NativeFn>) {
    registry.extend(super::math::math_natives());
    registry.extend(super::text_registry::text_natives());
    registry.extend(super::file::file_natives());
    registry.extend(super::bytes::bytes_natives());
    registry.extend(super::html::html_natives());
    registry.extend(super::list_registry::list_natives());
    registry.extend(super::map::map_natives());
    registry.extend(super::set::set_natives());
    registry.extend(super::convert::convert_natives());
    #[cfg(feature = "decimal")]
    registry.extend(crate::ext::decimal::decimal_natives());
    #[cfg(feature = "encoding")]
    registry.extend(crate::ext::encoding::encoding_natives());
    #[cfg(feature = "hash")]
    registry.extend(crate::ext::hash::hash_natives());
    #[cfg(feature = "ini")]
    registry.extend(crate::ext::ini::ini_natives());
    #[cfg(feature = "uri")]
    registry.extend(crate::ext::uri::uri_natives());
    #[cfg(feature = "uri")]
    registry.extend(crate::ext::uri::url_natives());
    #[cfg(feature = "path")]
    registry.extend(crate::ext::path::path_natives());
    registry.extend(super::validate::validate_natives());
    #[cfg(feature = "csv")]
    registry.extend(crate::ext::csv::csv_natives());
    registry.extend(super::random::random_natives());
    #[cfg(feature = "json")]
    registry.extend(crate::ext::json::json_natives());
    registry.extend(super::option::option_natives());
    registry.extend(super::result::result_natives());
    registry.extend(super::reflect::reflect_natives());
    registry.extend(super::process::process_natives());
    registry.extend(super::runtime::runtime_natives());
    registry.extend(super::log::log_natives());
    #[cfg(feature = "test")]
    registry.extend(crate::ext::test::test_natives());
    registry.extend(super::time::time_natives());
    #[cfg(feature = "cryptography")]
    registry.extend(crate::ext::cryptography::cryptography_natives());
    #[cfg(feature = "regex")]
    registry.extend(crate::ext::regex::regex_natives());
    #[cfg(feature = "database")]
    registry.extend(crate::ext::database::database_natives());
    #[cfg(feature = "mail")]
    registry.extend(crate::ext::mail::mail_natives());
    #[cfg(feature = "http-client")]
    registry.extend(crate::ext::http_client::http_client_natives());
    registry.extend(super::fs::fs_natives());
    #[cfg(feature = "session")]
    registry.extend(crate::ext::session::session_natives());
    registry.extend(super::input::input_natives());
    // DEC-331 slice 2 — the rich-Request wire natives (std-only baseline; jsonParse's delegate
    // body is `json`-gated inside the module).
    registry.extend(super::http::http_natives());
    #[cfg(feature = "debug")]
    registry.extend(crate::ext::debug::debug_natives());
}
