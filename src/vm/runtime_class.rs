//! Instance ops decided by the receiver's RUNTIME class rather than its compile-time type (plan
//! row 4b, VM-WITH-STATIC-CLASS): `with` on a receiver whose class may be a subclass, and property
//! hooks read or written through a supertype or an erased (`CTy::Other`) receiver. The tree-walker
//! — the oracle — always dispatches on the runtime class; these ops make the VM do the same at the
//! sites where the compiler cannot prove the class.

use super::*;
use crate::value::faults;

impl Vm<'_> {
    /// `Op::CloneInstance`: pop an instance, push a shallow copy (same class, layout, field values).
    pub(super) fn op_clone_instance(&mut self) -> Result<(), String> {
        match self.pop() {
            Value::Instance(base) => {
                let copy = Instance {
                    class: base.class.clone(),
                    layout: base.layout.clone(),
                    fields: RefCell::new(base.fields.borrow().clone()),
                };
                self.stack.push(Value::Instance(Rc::new(copy)));
                Ok(())
            }
            // Same text as the interpreter's `with` on a non-instance.
            other => Err(format!(
                "`with` requires a class instance, got {}",
                other.type_name()
            )),
        }
    }

    /// The runtime class's method named `names[hook]`, if it has one — the same `methods` table and
    /// key `CallMethod` resolves through. A hook is never overloaded, so no overload selection.
    fn runtime_hook(&self, receiver: usize, hook: usize) -> Option<usize> {
        match &self.stack[receiver] {
            Value::Instance(inst) => {
                let key = (inst.class.to_string(), self.program.names[hook].clone());
                self.program.methods.get(&key).copied()
            }
            _ => None,
        }
    }

    /// Enter `func` with the `argc + 1` values on top of the stack as its slots (receiver = `this`),
    /// exactly as `CallMethod` does once it has resolved its target.
    fn enter_hook(&mut self, func: usize, argc: usize) -> Result<(), String> {
        if self.frames.len() >= MAX_CALL_DEPTH {
            return Err(faults::FAULT_STACK_OVERFLOW.to_string());
        }
        let slot_base = self.pop_n_start(argc + 1);
        self.frames.push(Frame {
            func,
            ip: 0,
            slot_base,
        });
        Ok(())
    }

    /// `Op::GetFieldOrHook(field, hook)`: `[.., instance]` → the hook's result, or the stored field.
    pub(super) fn op_get_field_or_hook(&mut self, field: usize, hook: usize) -> Result<(), String> {
        let receiver = self.stack.len() - 1;
        if let Some(f) = self.runtime_hook(receiver, hook) {
            return self.enter_hook(f, 0);
        }
        match self.pop() {
            Value::Instance(inst) => {
                let name = &self.program.names[field];
                let value = match inst.layout.slot(name) {
                    Some(s) => inst.fields.borrow()[s].clone(),
                    None => None,
                };
                // An absent slot faults exactly as `GetField` does (`no field … on …`).
                let v = value.ok_or_else(|| faults::no_field(name, &inst.class))?;
                self.stack.push(v);
                Ok(())
            }
            v => Err(format!(
                "cannot read `.{}` on {}",
                self.program.names[field],
                v.type_name()
            )),
        }
    }

    /// `Op::SetFieldOrHook(field, hook)`: `[.., instance, value]` → `[.., Unit]`.
    pub(super) fn op_set_field_or_hook(&mut self, field: usize, hook: usize) -> Result<(), String> {
        let receiver = self.stack.len() - 2;
        if let Some(f) = self.runtime_hook(receiver, hook) {
            return self.enter_hook(f, 1);
        }
        let value = self.pop();
        match self.pop() {
            Value::Instance(inst) => {
                if let Some(s) = inst.layout.slot(&self.program.names[field]) {
                    inst.fields.borrow_mut()[s] = Some(value);
                }
                // A name not in the layout is checker-unreachable — dropped, as `SetField` does.
                self.stack.push(Value::Unit);
                Ok(())
            }
            v => Err(format!(
                "cannot set `.{}` on {}",
                self.program.names[field],
                v.type_name()
            )),
        }
    }
}
