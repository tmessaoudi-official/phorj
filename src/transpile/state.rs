//! Transpiler state helpers: the [`Transpiler::new`] constructor, the indentation-aware line writer,
//! scope push/pop, local declaration/lookup, and PHP `catch`-type rendering. Split out of
//! `transpile/mod.rs` (M-Decomp) to keep the root under the file-size cap; the `Transpiler` struct +
//! all field types live in the root/siblings and are reached via `use super::*`. Methods called from
//! sibling emit modules are `pub(super)`. Pure code movement — no emit-logic change.

use super::*;

impl Transpiler {
    pub(super) fn new() -> Self {
        Transpiler {
            funcs: HashSet::new(),
            foreign_fns: HashSet::new(),
            foreign_classes: HashSet::new(),
            classes: HashSet::new(),
            consts: HashSet::new(),
            variants: HashSet::new(),
            enums: HashSet::new(),
            keep: None,
            split: split::SplitPass::Off,
            variant_owner: HashMap::new(),
            variant_fields: HashMap::new(),
            src: None,
            out: String::new(),
            indent: 0,
            locals: Vec::new(),
            local_kinds: Vec::new(),
            cur_class: None,
            parent_aliases: None,
            class_field_kinds: HashMap::new(),
            class_parents: HashMap::new(),
            variant_field_kinds: HashMap::new(),
            fn_ret_kinds: HashMap::new(),
            method_ret_kinds: HashMap::new(),
            cur_class_fields: None,
            imports: HashMap::new(),
            gates: HelperGates::default(),
            namespaced: false,
            class_implements: std::collections::BTreeMap::new(),
            class_tables: crate::native::ClassTables::default(),
            debug_enum_rows: Vec::new(),
            decomposed: BTreeSet::new(),
            tmp: 0,
        }
    }

    /// Indentation-aware line writer.
    /// Re-emit a declaration's `/** … */` doc comment as a PHP DOCBLOCK at the current indent
    /// (DEC-419). A no-op when the caller supplied no source, when the declaration has no doc, or when
    /// the span is an INJECTED prelude span — prelude docs are phorj-internal and belong in no user's
    /// generated PHP.
    ///
    /// `/** … */` is PHPDoc's own syntax, so this is a re-emission rather than a translation: the star
    /// column is re-added around the same body. The body cannot contain `*/` by construction (the
    /// extractor stops at the first one), so it cannot terminate the docblock early.
    ///
    /// Emitting comments cannot affect the byte-identity spine — PHP comments produce no output.
    pub(super) fn emit_doc_block(&mut self, span: crate::token::Span) {
        let Some(src) = self.src.as_deref() else {
            return;
        };
        if span.start >= crate::cli::INJECTED_SPAN_BASE {
            return;
        }
        let Some(doc) = crate::doc_comment::doc_markdown_before(src, span.start) else {
            return;
        };
        self.line("/**");
        for l in doc.lines() {
            if l.is_empty() {
                self.line(" *");
            } else {
                self.line(&format!(" * {l}"));
            }
        }
        self.line(" */");
    }

    pub(super) fn line(&mut self, s: &str) {
        for _ in 0..self.indent {
            self.out.push_str("    ");
        }
        self.out.push_str(s);
        self.out.push('\n');
    }

    pub(super) fn push_scope(&mut self) {
        self.locals.push(HashSet::new());
        self.local_kinds.push(HashMap::new());
    }
    pub(super) fn pop_scope(&mut self) {
        self.locals.pop();
        self.local_kinds.pop();
    }
    pub(super) fn declare(&mut self, name: &str) {
        if let Some(s) = self.locals.last_mut() {
            s.insert(name.to_string());
        }
    }
    pub(super) fn is_local(&self, name: &str) -> bool {
        self.locals.iter().any(|s| s.contains(name))
    }
    /// Render a `catch` clause's type for PHP (M-faults 2b): a single class/interface via `php_type_ref`
    /// (FQN if cross-package), a union `A | B` as PHP 8's `A | B`. The built-in `Error` base maps to
    /// `\Exception` (a Phorj `Error` subtype transpiled to `extends \Exception`, and PHP's own `Error`
    /// is a *different* engine class — so `catch (Error e)` must catch `\Exception`, not PHP `\Error`).
    /// M8.5 S3a: a **foreign** exception class (`declare class … implements Error`) is caught by its own
    /// global PHP name (`\DivisionByZeroError`) — NOT the `Error`→`\Exception` mapping — so a foreign
    /// `\Error`-family class (a `\Throwable` that is not an `\Exception`) is caught correctly.
    pub(super) fn php_catch_type(&self, ty: &Type) -> String {
        match ty {
            Type::Named { name, .. } if self.foreign_classes.contains(name) => {
                format!("\\{}", php_class_name(name))
            }
            Type::Named { name, .. } if last_segment(name) == "Error" => "\\Exception".to_string(),
            Type::Named { name, .. } => php_type_ref(name),
            Type::Union(members, _) => members
                .iter()
                .map(|m| self.php_catch_type(m))
                .collect::<Vec<_>>()
                .join(" | "),
            _ => "\\Exception".to_string(), // defensive — the checker requires an Error-typed catch
        }
    }
}
