//! The `uri` extension (DEC-273 wave 2) — `Core.UriModule` (DEC-240): the RFC 3986 URI kernel,
//! its natives (`Core.Native.Uri`), the deprecated `Core.Url` compat twins (DEC-279), and the
//! prelude source, colocated per AMENDMENT 2. Transpile twin = PHP 8.5's always-on
//! `Uri\Rfc3986\Uri` (probe record: `docs/research/2026-07-16-uri-twin-probes.md`).

pub mod kernel;
mod kernel_norm;
pub mod natives;
mod registry;
pub mod url_compat;

#[cfg(test)]
#[path = "kernel_tests.rs"]
mod kernel_tests;
#[cfg(test)]
#[path = "url_tests.rs"]
mod url_tests;

pub use registry::uri_natives;
pub use url_compat::url_natives;
