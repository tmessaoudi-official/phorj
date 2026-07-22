//! Tree-walking interpreter — enum-variant statics (M-Decomp split from `call.rs`, Invariant 13).

use super::*;

impl<'c> Interp<'c> {
    /// DEC-302 enum static methods. `cases()` builds a `List` of every variant (payload-less,
    /// declaration order). `from(x)`/`tryFrom(x)` scan the variants for one whose backing equals `x`
    /// — `from` faults (single-sourced [`crate::value::enum_from_miss`], byte-identical to the VM) on
    /// a miss, `tryFrom` returns `null`. The checker guarantees the method/arity/backing are valid.
    pub(super) fn eval_enum_static(
        &mut self,
        enum_name: &str,
        method: &str,
        argv: Vec<Value>,
    ) -> R<Value> {
        let variants = self
            .enum_variants
            .get(enum_name)
            .cloned()
            .unwrap_or_default();
        let make = |variant: &str| {
            Value::Enum(Rc::new(EnumVal {
                ty: enum_name.into(),
                variant: variant.into(),
                payload: crate::value::Payload::Zero,
            }))
        };
        match method {
            "cases" => {
                let cases: Vec<Value> = variants.iter().map(|(v, _)| make(v)).collect();
                Ok(Value::List(Rc::new(cases)))
            }
            "from" | "tryFrom" => {
                let arg = &argv[0];
                for (v, _) in &variants {
                    if let Some(backing) =
                        self.enum_backing.get(&(enum_name.to_string(), v.clone()))
                    {
                        if backing.eq_val(arg) {
                            return Ok(make(v));
                        }
                    }
                }
                if method == "from" {
                    rt(crate::value::enum_from_miss(enum_name, arg))
                } else {
                    Ok(Value::Null)
                }
            }
            _ => rt(format!(
                "enum `{enum_name}` has no static method `{method}`"
            )),
        }
    }
}
