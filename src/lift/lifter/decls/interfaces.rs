//! Lift an interface (Lane R-5): bodiless methods become bodiless `FunctionDecl`s on a public
//! `InterfaceDecl`; nothing is guessed — the parser already refused constants and properties.

use super::*;
use crate::ast::InterfaceDecl;

pub(super) fn lift_interface(i: &php::PhpInterface) -> Result<InterfaceDecl, String> {
    let mut methods = Vec::new();
    for m in &i.methods {
        methods.push(FunctionDecl {
            modifiers: Vec::new(),
            attrs: Vec::new(),
            vis: crate::ast::Visibility::Public,
            name: m.name.clone(),
            type_params: Vec::new(),
            type_param_bounds: Vec::new(),
            params: lift_params(&m.params)?,
            ret: lift_ret(&m.ret, None)?,
            throws: Vec::new(),
            body: Vec::new(),
            foreign: false,
            generic_ret_from_param: None,
            span: SP,
        });
    }
    Ok(InterfaceDecl {
        vis: crate::ast::Visibility::Public,
        name: i.name.clone(),
        type_params: Vec::new(),
        extends: i.extends.clone(),
        methods,
        sealed: false,
        injected: false,
        span: SP,
    })
}
