//! Lift of PHP magic methods to phorj attribute-designated methods (DEC-331 D9). Split from
//! `decls.rs` (Invariant 13). `__construct` stays in `decls.rs` (it lifts to a `constructor`, a
//! distinct member kind); the attribute-marked magic methods live here.

use super::*;

impl Lifter {
    /// If `m` is a PHP magic method with a phorj attribute analog, lift it and return `Some(result)`;
    /// otherwise `None` (an ordinary method — the caller continues). Currently: `__toString` → a
    /// `#[ToString]` method named `toString` (the reverse of the transpiler's `__toString` delegate,
    /// so a phorj `#[ToString]` class exported to PHP and lifted back keeps its role). `__invoke` →
    /// `#[Invoke]` is DEC-331 slice 1b (deferred with the transpile emit side, to stay symmetric).
    pub(super) fn lift_magic_method(
        &mut self,
        m: &php::PhpMethod,
    ) -> Option<Result<ClassMember, String>> {
        if m.name != "__toString" {
            return None;
        }
        let mut declared = HashSet::new();
        let params = match lift_params(&m.params) {
            Ok(p) => p,
            Err(e) => return Some(Err(e)),
        };
        for p in &params {
            declared.insert(p.name.clone());
        }
        let body = match &m.body {
            Some(b) => match self.lift_block(b, &mut declared) {
                Ok(b) => b,
                Err(e) => return Some(Err(e)),
            },
            None => Vec::new(),
        };
        Some(Ok(ClassMember::Method(FunctionDecl {
            modifiers: Vec::new(),
            attrs: vec![crate::ast::Attribute {
                name: "ToString".to_string(),
                args: Vec::new(),
                span: SP,
            }],
            vis: crate::ast::Visibility::Public,
            name: "toString".to_string(),
            type_params: Vec::new(),
            type_param_bounds: Vec::new(),
            params,
            ret: Some(Type::Named {
                name: "string".to_string(),
                args: Vec::new(),
                span: SP,
            }),
            throws: Vec::new(),
            body,
            foreign: false,
            generic_ret_from_param: None,
            span: SP,
        })))
    }
}
