//! Transpile of DEC-331 D9 attribute-designated magic methods to their native PHP form (split from
//! `classes.rs` per Invariant 13). Slice 1: the `#[ToString]` → PHP `__toString` delegate. (`#[Invoke]`
//! → `__invoke` + the multi-invoke dispatch shim is DEC-331 slice 1b.)

use super::*;

impl Transpiler {
    /// Emit a native PHP `__toString` delegating to the class's `#[ToString]` method, so a phorj object
    /// stringifies idiomatically in a PHP host (`echo $obj` / `(string)$obj`) and round-trips back via
    /// lift. Byte-identity-safe: phorj's OWN emitted code always calls the named method explicitly (the
    /// `resolve_invoke_tostring` rewrite), so this is reached only by external PHP coercion. Emits
    /// nothing unless THIS class declares a `#[ToString]` (an inherited one rides PHP inheritance of the
    /// parent's `__toString`).
    pub(super) fn emit_tostring_delegate(&mut self, c: &ClassDecl) -> Result<(), String> {
        let ts = c.members.iter().find_map(|m| match m {
            ClassMember::Method(f) if f.attrs.iter().any(crate::ast::Attribute::is_to_string) => {
                Some(f.name.clone())
            }
            _ => None,
        });
        if let Some(name) = ts {
            self.line("function __toString(): string {");
            self.indent += 1;
            self.line(&format!("return $this->{name}();"));
            self.indent -= 1;
            self.line("}");
        }
        Ok(())
    }
}
