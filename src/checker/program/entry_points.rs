//! Program pass — `#[Entry]` validation (DEC-331 D1 / DEC-337).

use super::*;

impl Checker {
    /// DEC-331 D1 / DEC-337: entries are declared by `#[Entry(kind: EntryKind.Cli|Web)]` — the ROLE is declared,
    /// not inferred (bare `#[Entry]` is retired, FULLY BREAKING). Validate every attributed
    /// candidate here: an instance-method `#[Entry]` is `E-ENTRY-TARGET`; a missing/unknown/
    /// reserved `kind:` is `E-ENTRY-KIND-{REQUIRED,UNKNOWN,RESERVED}`; a signature that
    /// disagrees with the declared kind is `E-ENTRY-SIG`; more than one entry OF THE SAME KIND
    /// is `E-DUPLICATE-ENTRY-KIND` (one Cli + one Web may coexist — run/serve each pick theirs).
    pub(in crate::checker) fn check_entry_points(&mut self, program: &Program) {
        use crate::ast::Item;
        let mut cli_seen = false;
        let mut web_seen = false;
        // DEC-337: `EntryKind` is import-gated — a qualified `EntryKind.<Variant>` kind requires
        // the member import `import Core.Runtime.EntryKind;` (or the whole-module `Core.Runtime`).
        let entrykind_imported = program.items.iter().any(|it| {
            matches!(it, Item::Import { path, .. } if
                path.first().map(String::as_str) == Some("Core")
                && path.get(1).map(String::as_str) == Some("Runtime")
                && (path.len() == 2
                    || (path.len() == 3 && path[2] == crate::ast::ENTRY_KIND_ENUM)))
        });
        for item in &program.items {
            let mut check_entry = |f: &crate::ast::FunctionDecl, instance_method: bool| {
                let Some(attr) = f.attrs.iter().find(|a| crate::ast::is_entry_attr(a)) else {
                    return;
                };
                if instance_method {
                    self.err_coded(
                        attr.span,
                        "`#[Entry]` on an instance method — an entry runs without an instance"
                            .to_string(),
                        "E-ENTRY-TARGET",
                        Some(
                            "make the method `static`, or move the entry to a top-level function"
                                .into(),
                        ),
                    );
                    return;
                }
                // DEC-337: the kind must be the QUALIFIED injected variant `EntryKind.<Variant>`,
                // never "in the wind" — enforce the surface form + import before classifying the
                // variant name. A bare `kind: Cli` is `E-INJECTED-VARIANT-BARE` (the same rule that
                // governs `Option.Some`); a wrong qualifier is `E-ENTRY-KIND-UNKNOWN`; an
                // unimported `EntryKind` is `E-UNIMPORTED`. `Missing` falls through to the
                // `E-ENTRY-KIND-REQUIRED` arm below.
                match crate::ast::entry_kind_form(attr) {
                    crate::ast::EntryKindForm::Missing => {}
                    crate::ast::EntryKindForm::Malformed => {
                        self.err_coded(
                            attr.span,
                            "`#[Entry]`'s `kind:` must be an `EntryKind` variant".to_string(),
                            "E-ENTRY-KIND-REQUIRED",
                            Some("write `#[Entry(kind: EntryKind.Cli)]` or `#[Entry(kind: EntryKind.Web)]`".into()),
                        );
                        return;
                    }
                    crate::ast::EntryKindForm::Bare(n) => {
                        // `kind: EntryKind` — the enum NAME with no variant — is a missing
                        // variant, not a bare variant; suggesting `EntryKind.EntryKind` would
                        // be nonsensical, so route it to the missing-kind diagnostic instead.
                        if n == crate::ast::ENTRY_KIND_ENUM {
                            self.err_coded(
                                attr.span,
                                "`kind:` names the `EntryKind` enum but no variant".to_string(),
                                "E-ENTRY-KIND-REQUIRED",
                                Some("write `#[Entry(kind: EntryKind.Cli)]` or `#[Entry(kind: EntryKind.Web)]`".into()),
                            );
                            return;
                        }
                        self.err_coded(
                            attr.span,
                            format!("`{n}` is an injected `EntryKind` variant and must be written qualified as `EntryKind.{n}`"),
                            "E-INJECTED-VARIANT-BARE",
                            Some(format!("write `#[Entry(kind: EntryKind.{n})]` and `import Core.Runtime.EntryKind;`")),
                        );
                        return;
                    }
                    crate::ast::EntryKindForm::Qualified { qual, name } => {
                        // Two accepted qualifiers, mirroring the `#[Entry]` attribute's forms:
                        // the short `EntryKind` (member-imported) and the self-gating
                        // fully-qualified `Core.Runtime.EntryKind` (needs no import).
                        let is_short = qual == crate::ast::ENTRY_KIND_ENUM;
                        let is_fq = qual == crate::ast::ENTRY_KIND_ENUM_FQ;
                        if !is_short && !is_fq {
                            self.err_coded(
                                attr.span,
                                format!("unknown entry-kind qualifier `{qual}` — the kind is an `EntryKind` variant"),
                                "E-ENTRY-KIND-UNKNOWN",
                                Some(format!("write `#[Entry(kind: EntryKind.{name})]`")),
                            );
                            return;
                        }
                        // The short form is import-gated ("nothing in the wind"); the
                        // fully-qualified form is self-gating (no import), like `#[Core.Runtime.Entry]`.
                        // A compiler-SYNTHESIZED entry (zero span — the test-runner's driver, a
                        // lifted draft) is exempt too: the user never wrote it, so the wind rule
                        // doesn't apply (the same exemption the `#[Entry]` marker itself carries).
                        if is_short && !entrykind_imported && attr.span.line != 0 {
                            self.err_coded(
                                attr.span,
                                "`EntryKind` is used without importing it".to_string(),
                                "E-UNIMPORTED",
                                Some("add `import Core.Runtime.EntryKind;` (or import the whole module `import Core.Runtime;`)".into()),
                            );
                            return;
                        }
                    }
                }
                // DEC-331 D1: the role comes from the declared `kind:`, not the signature.
                // Bare `#[Entry]` / unknown / reserved kinds are hard errors; an active kind
                // still must AGREE with the signature shape (`entry_role` is now the validator).
                let role = match crate::ast::parse_entry_kind(attr) {
                    crate::ast::EntryKind::Missing => {
                        self.err_coded(
                            attr.span,
                            "`#[Entry]` requires a `kind:` — the entry role is declared, not inferred"
                                .to_string(),
                            "E-ENTRY-KIND-REQUIRED",
                            Some("write `#[Entry(kind: EntryKind.Cli)]` for a `phg run` entry, or `#[Entry(kind: EntryKind.Web)]` for `phg serve`".into()),
                        );
                        return;
                    }
                    crate::ast::EntryKind::Unknown(n) => {
                        self.err_coded(
                            attr.span,
                            format!("unknown entry kind `{n}` — active kinds are `Cli` and `Web`; `Desktop`/`Mobile`/`Worker`/`Embedded` are reserved"),
                            "E-ENTRY-KIND-UNKNOWN",
                            Some("use `#[Entry(kind: EntryKind.Cli)]` or `#[Entry(kind: EntryKind.Web)]`".into()),
                        );
                        return;
                    }
                    crate::ast::EntryKind::Reserved(n) => {
                        self.err_coded(
                            attr.span,
                            format!("entry kind `{n}` is reserved but not yet implemented — the active kinds are `Cli` and `Web`"),
                            "E-ENTRY-KIND-RESERVED",
                            Some("use `#[Entry(kind: EntryKind.Cli)]` or `#[Entry(kind: EntryKind.Web)]` for now".into()),
                        );
                        return;
                    }
                    crate::ast::EntryKind::Active(role) => role,
                };
                // The declared kind must AGREE with the signature shape. `entry_shape_matches`, not
                // `entry_role`: since S3.1 retired DEC-191 inference the question is "is this shape
                // legal FOR the declared role?", and `(): void` is legal for both Cli and Web (a Web
                // entry calls `Http.serve(cfg, handler)` in its body — DEC-331 S3.3b).
                if !crate::ast::entry_shape_matches(f, role) {
                    let (kind_name, shape) = match role {
                        crate::ast::EntryRole::Cli => (
                            "Cli",
                            "`(): void`, `(): int`, or `(List<string>): void|int`",
                        ),
                        crate::ast::EntryRole::Web => {
                            ("Web", "`(): void`, or the legacy `(Request): Response`")
                        }
                    };
                    self.err_coded(
                        f.span,
                        format!(
                            "`#[Entry(kind: EntryKind.{kind_name})]` function `{}`'s signature doesn't match — a `{kind_name}` entry is {shape}",
                            f.name
                        ),
                        "E-ENTRY-SIG",
                        Some("adjust the signature to the declared kind's shape".into()),
                    );
                    return;
                }
                // At most one entry per kind (DEC-331 §3.1).
                let (dup, kind_name) = match role {
                    crate::ast::EntryRole::Cli => {
                        let d = cli_seen;
                        cli_seen = true;
                        (d, "Cli")
                    }
                    crate::ast::EntryRole::Web => {
                        let d = web_seen;
                        web_seen = true;
                        (d, "Web")
                    }
                };
                if dup {
                    self.err_coded(
                        f.span,
                        format!("duplicate `#[Entry(kind: EntryKind.{kind_name})]` — a program has at most one entry per kind"),
                        "E-DUPLICATE-ENTRY-KIND",
                        Some("remove the extra entry, or give it a different kind".into()),
                    );
                }
            };
            match item {
                Item::Function(f) => check_entry(f, false),
                Item::Class(c) => {
                    for m in &c.members {
                        if let crate::ast::ClassMember::Method(f) = m {
                            let is_static = f.modifiers.contains(&crate::ast::Modifier::Static);
                            check_entry(f, !is_static);
                        }
                    }
                }
                _ => {}
            }
        }
    }
}
