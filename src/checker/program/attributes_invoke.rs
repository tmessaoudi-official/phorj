//! Program pass — `#[Invoke]` / `#[ToString]` attribute validation (DEC-331 D9). Split out of
//! `attributes.rs` (Invariant 13 soft cap). Per-method legality (target + strict `#[ToString]`
//! signature) is checked from `check_attributes`; class-level uniqueness (`E-TOSTRING-DUPLICATE`,
//! `E-INVOKE-DUPLICATE`) runs once per class from the class walk. The recognition predicates are the
//! single source [`crate::ast::Attribute::is_invoke`]/[`crate::ast::Attribute::is_to_string`], and the
//! harvested roles live on `ClassInfo` (`collect_class`), consumed by the call-typing + lowering.

use super::*;

impl Checker {
    /// Validate one `#[Invoke]`/`#[ToString]` attribute on a function/method declaration. Returns
    /// `true` when `attr` is one of these markers (so `check_attributes` treats it as KNOWN and does
    /// not fall through to `E-UNKNOWN-ATTRIBUTE`). Enforces the TARGET rule — an INSTANCE method only
    /// (`self.cur_class` is set and the method is not `static`; a free function or a `static` method
    /// has no receiver to make callable/stringify) — and, for `#[ToString]`, the STRICT signature
    /// (zero parameters, returns `string`). Class-level uniqueness is [`Self::check_invoke_tostring_class`].
    pub(in crate::checker) fn check_invoke_tostring_attr(
        &mut self,
        attr: &crate::ast::Attribute,
        f: &crate::ast::FunctionDecl,
    ) -> bool {
        let is_invoke = attr.is_invoke();
        let is_to_string = attr.is_to_string();
        if !is_invoke && !is_to_string {
            return false;
        }
        let label = if is_invoke { "Invoke" } else { "ToString" };
        let is_method = self.cur_class.is_some();
        let is_static = f.modifiers.contains(&crate::ast::Modifier::Static);
        if !is_method || is_static {
            self.err_coded(
                attr.span,
                format!("`#[{label}]` is only valid on an instance method"),
                "E-ATTRIBUTE-TARGET",
                Some(
                    "put it on a non-static class method — a free function or `static` method has no \
                     receiver to make callable/stringify"
                        .into(),
                ),
            );
            return true;
        }
        if is_to_string {
            let ret_is_string = matches!(
                f.ret.as_ref().map(|t| self.resolve_type(t)),
                Some(Ty::String)
            );
            if !f.params.is_empty() || !ret_is_string {
                self.err_coded(
                    attr.span,
                    "a `#[ToString]` method must take no parameters and return `string`"
                        .to_string(),
                    "E-TOSTRING-SIGNATURE",
                    Some("declare it `#[ToString] function toString(): string { … }`".into()),
                );
            }
        }
        if is_invoke && f.params.iter().any(|p| p.default.is_some() || p.variadic) {
            // Slice 1: a call `x(args)` resolves the `#[Invoke]` set by EXACT arity/type, so a default
            // or variadic param (which blur arity, and across differently-named `#[Invoke]` methods
            // could let `x(5)` pick a different method than `x.method(5)`) is rejected — no silent
            // divergence. The method stays callable by name WITH its defaults. Honoring defaults through
            // the `x(…)` sugar is DEC-331 slice 1b (spec §8). (`#[ToString]` is zero-param, so N/A there.)
            self.err_coded(
                attr.span,
                "an `#[Invoke]` method may not have default or variadic parameters (slice 1)"
                    .to_string(),
                "E-INVOKE-DEFAULTS",
                Some(
                    "give it a fixed parameter list, or call the method by name (`x.method(…)`) to use \
                     its defaults"
                        .into(),
                ),
            );
        }
        true
    }

    /// Class-level uniqueness for the DEC-331 D9 markers — run once per class alongside
    /// `check_class_attributes`. (1) At most one `#[ToString]` method (`E-TOSTRING-DUPLICATE`) — a
    /// class has a single stringification. (2) No two `#[Invoke]` overloads share a parameter
    /// signature (`E-INVOKE-DUPLICATE`), or a call `x(args)` could not resolve. Reads the collected
    /// [`ClassInfo`] (`invoke_methods` + resolved `methods` signatures) so it sees the same overload
    /// sets the call-typing will. `#[Invoke]` marks a method NAME callable, so every overload of a
    /// marked name participates.
    pub(in crate::checker) fn check_invoke_tostring_class(&mut self, c: &crate::ast::ClassDecl) {
        self.check_invoke_tostring_members(&c.name, &c.members, c.span);
    }

    /// Shared by classes ([`Self::check_invoke_tostring_class`]) and traits (both flatten into a
    /// `ClassInfo` via `collect_class`, so both must enforce the "one `#[ToString]` / distinct
    /// `#[Invoke]` signatures" rule — a duplicate entering via a trait would otherwise be silently
    /// dropped in the harvest). `name` keys the collected `ClassInfo`; `members` are scanned for the
    /// per-method `#[ToString]` spans; `owner_span` anchors the invoke-duplicate diagnostic.
    pub(in crate::checker) fn check_invoke_tostring_members(
        &mut self,
        name: &str,
        members: &[crate::ast::ClassMember],
        owner_span: Span,
    ) {
        use crate::ast::ClassMember;
        let ts_spans: Vec<Span> = members
            .iter()
            .filter_map(|m| match m {
                ClassMember::Method(f)
                    if f.attrs.iter().any(crate::ast::Attribute::is_to_string) =>
                {
                    Some(f.span)
                }
                _ => None,
            })
            .collect();
        for sp in ts_spans.iter().skip(1) {
            self.err_coded(
                *sp,
                format!("`{name}` declares more than one `#[ToString]` method"),
                "E-TOSTRING-DUPLICATE",
                Some(
                    "a type has a single stringification — keep exactly one `#[ToString]` method"
                        .into(),
                ),
            );
        }
        // Gather every `#[Invoke]` overload's parameter signature from the collected ClassInfo.
        let candidates: Vec<Vec<Ty>> = match self.classes.get(name) {
            Some(info) => info
                .invoke_methods
                .iter()
                .filter_map(|n| info.methods.get(n))
                .flat_map(|sigs| sigs.iter().map(|s| s.params.clone()))
                .collect(),
            None => return,
        };
        for i in 0..candidates.len() {
            for j in (i + 1)..candidates.len() {
                if candidates[i] == candidates[j] {
                    self.err_coded(
                        owner_span,
                        format!(
                            "`{name}` has two `#[Invoke]` methods with the same parameter signature"
                        ),
                        "E-INVOKE-DUPLICATE",
                        Some(
                            "give the `#[Invoke]` methods distinct parameter signatures so a call \
                             `x(…)` resolves to one of them"
                                .into(),
                        ),
                    );
                    return;
                }
            }
        }
    }
}
