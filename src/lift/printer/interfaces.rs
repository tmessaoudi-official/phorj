//! Lift printer — interfaces (Lane R-5): `interface Name [extends A, B] { function m(…): T; }`.

use super::*;
use crate::ast::InterfaceDecl;

impl Printer {
    pub(super) fn interface(&mut self, i: &InterfaceDecl) -> Result<(), String> {
        let mut header = format!("interface {}", i.name);
        if !i.extends.is_empty() {
            header.push_str(&format!(" extends {}", i.extends.join(", ")));
        }
        header.push_str(" {");
        self.line(&header);
        self.indent += 1;
        for f in &i.methods {
            let ret = match &f.ret {
                Some(t) => format!(": {}", ty(t)?),
                None => String::new(),
            };
            self.line(&format!(
                "function {}({}){ret};",
                f.name,
                self.params(&f.params)?
            ));
        }
        self.indent -= 1;
        self.line("}");
        Ok(())
    }
}
