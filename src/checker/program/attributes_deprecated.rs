//! Program pass — `#[Deprecated(message: "…")]` attribute validation (DEC-417). Split out of
//! `attributes.rs` (Invariant 13 soft cap), mirroring the `attributes_invoke.rs` precedent.
//!
//! **Compile-time only (DEC-417.2).** This attribute never reaches a backend: the checker records the
//! deprecation, warns `W-DEPRECATED` at every USE site, and the LSP tags both the declaration and the
//! usages. It is deliberately NOT mapped to PHP's native `#[\Deprecated]` — verified on `php-8.5.8`,
//! that one fires at RUNTIME and prints onto **stdout**, which would break Invariant 1 byte-identity
//! against the two Rust legs (whose warning is compile-time and never enters program output).
//!
//! Recognition is the single source [`crate::ast::Attribute::is_deprecated`]; the message is read once
//! by [`crate::ast::Attribute::deprecation_message`] so the warning, hover and lifter cannot drift.

use super::*;

impl Checker {
    /// Harvest a declaration's `#[Deprecated]` into the [`DeprecationNote`] stored on its `FnSig`
    /// (DEC-417). Called once per declaration at COLLECTION time, so a use site never re-walks
    /// attributes. `None` when the declaration is not deprecated — the common case.
    ///
    /// Recognition and message-reading both go through the `ast::Attribute` single sources, so this
    /// cannot drift from the checker's validation or the LSP's hover text.
    pub(in crate::checker) fn harvest_deprecation(
        attrs: &[crate::ast::Attribute],
    ) -> Option<super::DeprecationNote> {
        attrs
            .iter()
            .find(|a| a.is_deprecated())
            .map(|a| super::DeprecationNote {
                message: a.deprecation_message().map(str::to_string),
            })
    }

    /// Validate one `#[Deprecated]` attribute. Returns `true` when `attr` IS this marker, so
    /// `check_attributes` treats it as KNOWN and does not fall through to `E-UNKNOWN-ATTRIBUTE`.
    ///
    /// Two rejections, both about the compiler being able to READ the message:
    /// - a `message:` that is not a plain string literal (an interpolated `"use {x}"`, or any other
    ///   expression) — there is no runtime to evaluate holes against, so the text would be silently
    ///   lost. Rejected loudly instead (`E-DEPRECATED-MESSAGE`);
    /// - any POSITIONAL argument — the only supported shape is the named `message:`, matching how
    ///   `#[Entry(kind: …)]` spells its argument (DEC-337: nothing in the wind).
    ///
    /// A bare `#[Deprecated]` with no arguments is LEGAL: the warning then names only the symbol.
    pub(super) fn check_deprecated_attr(&mut self, attr: &crate::ast::Attribute) -> bool {
        if !attr.is_deprecated() {
            return false;
        }
        if attr.has_unreadable_deprecation_message() {
            self.err_coded(
                attr.span,
                "`#[Deprecated(message: …)]` needs a plain string literal — an interpolated string \
                 has no runtime to evaluate it against, so its text would be lost"
                    .to_string(),
                "E-DEPRECATED-MESSAGE",
                Some(
                    "write the text out literally, e.g. `#[Deprecated(message: \"use Uri.encodeComponent\")]`"
                        .into(),
                ),
            );
            return true;
        }
        if attr
            .args
            .iter()
            .any(|a| !matches!(a, crate::ast::Expr::NamedArg { .. }))
        {
            self.err_coded(
                attr.span,
                "`#[Deprecated]` takes only the named argument `message:`".to_string(),
                "E-DEPRECATED-MESSAGE",
                Some(
                    "write `#[Deprecated(message: \"…\")]`, or `#[Deprecated]` with no argument"
                        .into(),
                ),
            );
        }
        true
    }
}

impl Checker {
    /// Report a use of a deprecated free function or method (DEC-417). This is the *"show anything
    /// using a deprecated thing as deprecated too"* half of the ruling: the declaration is tagged by
    /// the LSP, and every USE site gets this warning — which carries `DiagnosticTag.Deprecated`, the
    /// thing that makes an editor strike the usage through.
    ///
    /// Overload sets warn only when EVERY signature is deprecated. A set with one live overload must
    /// not warn, because this call may well resolve to that one; warning there would train authors to
    /// ignore the channel.
    ///
    /// `W-DEPRECATED` rides the WARNING channel and never gates: `run`/`check` still succeed (the
    /// DEC-360 rule that warnings do not fail a build), and `--strict` is what promotes it.
    pub(in crate::checker) fn warn_if_deprecated_fn(
        &mut self,
        sigs: &[FnSig],
        label: &str,
        span: Span,
    ) {
        if let Some(note) = Self::deprecation_of_set(sigs) {
            self.warn_deprecated_use(&note, label, span);
        }
    }

    /// The deprecation covering an entire overload SET, or `None`.
    ///
    /// The rule lives here once, shared by the free-function and method call paths: a set warns only
    /// when EVERY signature is deprecated. A set with one live overload must stay silent, because the
    /// call may well resolve to that one — warning there would train authors to ignore the channel.
    pub(in crate::checker) fn deprecation_of_set(sigs: &[FnSig]) -> Option<DeprecationNote> {
        if sigs.is_empty() || !sigs.iter().all(|s| s.deprecated.is_some()) {
            return None;
        }
        sigs.iter().find_map(|s| s.deprecated.clone())
    }

    /// Emit the use-site warning for an already-resolved [`DeprecationNote`]. The message shape is
    /// written once here so the free-function path, the method path and the LSP hover cannot drift.
    ///
    /// `W-DEPRECATED` rides the WARNING channel and never gates: `run`/`check` still succeed (DEC-360
    /// — warnings do not fail a build), and `--strict` is what promotes it.
    pub(in crate::checker) fn warn_deprecated_use(
        &mut self,
        note: &DeprecationNote,
        label: &str,
        span: Span,
    ) {
        let msg = match note.message.as_deref() {
            Some(m) => format!("`{label}` is deprecated: {m}"),
            None => format!("`{label}` is deprecated"),
        };
        self.warn_coded(
            span,
            msg,
            "W-DEPRECATED",
            Some("this still works, but the author marked it for removal".into()),
        );
    }
}
