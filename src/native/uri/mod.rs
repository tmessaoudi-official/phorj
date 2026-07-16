//! `Core.Uri` (DEC-240) — the RFC 3986 URI kernel and its natives. The phorj surface (the
//! injected `Uri` class + the `UriError` taxonomy) lives in the Core.Uri prelude; the natives
//! here are its raw seam. Transpile twin = PHP 8.5's always-on `Uri\Rfc3986\Uri` (probe record:
//! `docs/research/2026-07-16-uri-twin-probes.md`).

pub(super) mod kernel;
mod natives;

pub(crate) use natives::uri_natives;

#[cfg(test)]
#[path = "kernel_tests.rs"]
mod kernel_tests;
