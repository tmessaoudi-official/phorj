//! Expression compilation — a member read `o.name` / `o?.name`: a class constant, a static, a property
//! hook (decided at compile time on an exact class, at run time otherwise — plan row 4b), a backed
//! enum's `.value`, or a stored field.

use super::*;

impl Compiler<'_> {
    pub(in crate::compiler) fn compile_member(
        &mut self,
        object: &Expr,
        name: &str,
        safe: bool,
        line: u32,
    ) -> Result<(), String> {
        // Field read: evaluate the object, then look its field up at runtime by name
        // (decision P4-5). Runtime lookup keeps the compiler untyped; the fault on a miss
        // is byte-identical to the interpreter's. `?.` (safe) short-circuits a null receiver.
        // A `const` class constant (Feature A): inline the literal `Value` via `Op::Const` —
        // no runtime store. Checked before the static path (a const is never a static slot).
        if let Some(v) = self.const_value(object, name) {
            self.emit_const(v, line);
        } else if let Some(idx) = self.static_slot(object, name) {
            // Static read `ClassName.field` (M-mut.7): the head is a class name (not a local),
            // so this is program-level state, not an instance field.
            self.emit(Op::GetStatic(idx), line);
        } else if let Some(getm) = self.hook_get_method(object, name) {
            // Property hook read `o.name` (M-mut.7b) → call the synthetic `<name>$get`
            // 0-arg method, which leaves the computed value on the stack. `?.` short-circuits
            // a null receiver before dispatch (the interpreter does the same).
            if safe {
                self.compile_safe_access(object, line, |c| {
                    let idx = c.field_name_index(&getm)?;
                    c.emit(Op::CallMethod(idx, 0), line);
                    Ok(())
                })?;
            } else {
                self.expr(object)?;
                let idx = self.field_name_index(&getm)?;
                self.emit(Op::CallMethod(idx, 0), line);
            }
        } else if let Some((fidx, hidx)) = self.hook_dynamic(object, name, "get") {
            // Plan row 4b: some class hooks `name` but the receiver's compile-time class
            // does not decide it — dispatch on the RUNTIME class (`vm/runtime_class.rs`).
            if safe {
                self.compile_safe_access(object, line, |c| {
                    c.emit(Op::GetFieldOrHook(fidx, hidx), line);
                    Ok(())
                })?;
            } else {
                self.expr(object)?;
                self.emit(Op::GetFieldOrHook(fidx, hidx), line);
            }
        } else if name == "value" && self.is_backed_enum_receiver(object) {
            // DEC-302 `s.value` on a backed enum → `Op::EnumValue` (backing lookup), not a
            // `GetField` (an enum value has no instance field). `?.value` short-circuits null.
            if safe {
                self.compile_safe_access(object, line, |c| {
                    c.emit(Op::EnumValue, line);
                    Ok(())
                })?;
            } else {
                self.expr(object)?;
                self.emit(Op::EnumValue, line);
            }
        } else if safe {
            self.compile_safe_access(object, line, |c| c.emit_field_read(name, line))?;
        } else {
            self.expr(object)?;
            self.emit_field_read(name, line)?;
        }
        Ok(())
    }
}
