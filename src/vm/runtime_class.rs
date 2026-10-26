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
    /// key `CallMethod` resolves through (a hook is never overloaded). Cached per op site exactly as
    /// `CallMethod` caches its target, keyed on the class-name pointer; `NO_HOOK` caches "this class
    /// has no hook", so a monomorphic site does no lookup and no `String` allocation after its first run.
    #[inline]
    fn runtime_hook(
        &self,
        inst: &Instance,
        hook: usize,
        (func, fr): (usize, usize),
    ) -> Option<usize> {
        const NO_HOOK: u32 = u32::MAX;
        let class_ptr: *const u8 = inst.class.as_ptr();
        let cell = &self.method_caches[func][self.frames[fr].ip - 1];
        let (cached_class, cached) = cell.get();
        if cached_class == class_ptr {
            return (cached != NO_HOOK).then_some(cached as usize);
        }
        let key = (inst.class.to_string(), self.program.names[hook].clone());
        let found = self.program.methods.get(&key).copied();
        cell.set((class_ptr, found.map_or(NO_HOOK, |f| f as u32)));
        found
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

    /// The stored-field slot of `names[field]` in `inst`, through this site's `field_caches` entry
    /// exactly as `GetField` resolves it: a monomorphic site skips the layout hash after its first run.
    #[inline]
    fn cached_slot(
        &self,
        inst: &Instance,
        field: usize,
        (func, fr): (usize, usize),
    ) -> Option<usize> {
        let lp: *const crate::value::ClassLayout = Rc::as_ptr(&inst.layout);
        let cell = &self.field_caches[func][self.frames[fr].ip - 1];
        let (cached, cslot) = cell.get();
        if std::ptr::eq(cached, lp) {
            return Some(cslot as usize);
        }
        let s = inst.layout.slot(&self.program.names[field])?;
        cell.set((lp, s as u32));
        Some(s)
    }

    /// `Op::GetFieldOrHook(field, hook)`: `[.., instance]` → the hook's result, or the stored field.
    /// The receiver is read in place: the stored value replaces it, or it becomes the hook's `this`.
    pub(super) fn op_get_field_or_hook(
        &mut self,
        field: usize,
        hook: usize,
        site: (usize, usize),
    ) -> Result<(), String> {
        let top = self.stack.len() - 1;
        let stored = match &self.stack[top] {
            Value::Instance(inst) => match self.runtime_hook(inst, hook, site) {
                Some(f) => Err(f),
                // An absent slot faults exactly as `GetField` does (`no field … on …`).
                None => Ok(self
                    .cached_slot(inst, field, site)
                    .and_then(|s| inst.fields.borrow()[s].clone())
                    .ok_or_else(|| faults::no_field(&self.program.names[field], &inst.class))),
            },
            v => {
                return Err(format!(
                    "cannot read `.{}` on {}",
                    self.program.names[field],
                    v.type_name()
                ))
            }
        };
        match stored {
            Ok(value) => {
                self.stack[top] = value?;
                Ok(())
            }
            Err(f) => self.enter_hook(f, 0),
        }
    }

    /// `Op::SetFieldOrHook(field, hook)`: `[.., instance, value]` → `[.., Unit]`.
    pub(super) fn op_set_field_or_hook(
        &mut self,
        field: usize,
        hook: usize,
        site: (usize, usize),
    ) -> Result<(), String> {
        let receiver = self.stack.len() - 2;
        if let Value::Instance(inst) = &self.stack[receiver] {
            if let Some(f) = self.runtime_hook(inst, hook, site) {
                return self.enter_hook(f, 1);
            }
        }
        let value = self.pop();
        match self.pop() {
            Value::Instance(inst) => {
                if let Some(s) = self.cached_slot(&inst, field, site) {
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
