//! PHP transpiler — function emission: free functions, methods, overload sets.

use super::*;

impl Transpiler {
    pub(super) fn emit_function(
        &mut self,
        f: &FunctionDecl,
        is_method: bool,
    ) -> Result<(), String> {
        self.emit_function_named(f, is_method, None)
    }

    /// Emit a function/method, optionally under an overridden name (M-RT overloading emits each
    /// overload's body under a mangled `<name>__ovl_<i>` name; the dispatcher takes the original).
    pub(super) fn emit_function_named(
        &mut self,
        f: &FunctionDecl,
        is_method: bool,
        name_override: Option<&str>,
    ) -> Result<(), String> {
        // `#[UncheckedOverflow]` (Core.Runtime.Integer.UncheckedOverflow, §14 LADDER): two's-complement wrapping int arithmetic has NO
        // faithful PHP target — PHP silently promotes an overflowing int to float, which would make the
        // transpiled program behave differently than the VM/interpreter (breaking the byte-identity
        // spine). So an `#[UncheckedOverflow]` function is a HARD transpile error (never a silent checked/float
        // lowering); such a program is quarantined from the PHP oracle, exactly like `spawn`.
        if f.attrs.iter().any(|a| a.is_unchecked_overflow()) {
            return Err(format!(
                "E-TRANSPILE-UNCHECKED: `#[UncheckedOverflow]` function `{}` uses wrapping integer arithmetic, which has no PHP equivalent (PHP overflows int→float) — it runs on the Phorj VM/interpreter only",
                f.name
            ));
        }
        let params: Vec<String> = f
            .params
            .iter()
            .map(|p| format!("{} ${}", self.emit_type(&p.ty), p.name))
            .collect();
        // In namespaced mode a top-level function is declared inside its `namespace` block, so emit
        // only its trailing segment (`Acme\Util\compute` ⇒ `compute`). Methods keep their name.
        let disp = match name_override {
            Some(n) => n,
            None if self.namespaced && !is_method => last_segment(&f.name),
            None => &f.name,
        };
        // Batch-1 D: a `static` method must be emitted `static` in PHP, else a class-static entry call
        // (`App::main()`) is a "non-static method called statically" fatal. Safe for every static
        // method: the checker forbids `this` inside one (`E-STATIC-THIS`), and PHP still permits an
        // instance-style call (`$m->staticMethod()`), so the existing `m.square(5)` pattern is unaffected.
        let static_prefix = if is_method && f.modifiers.contains(&Modifier::Static) {
            "static "
        } else {
            ""
        };
        self.line(&format!(
            "{}function {}({}){} {{",
            static_prefix,
            disp,
            params.join(", "),
            self.ret_suffix(&f.ret)
        ));
        self.indent += 1;
        self.push_scope();
        for p in &f.params {
            self.declare(&p.name);
            // T6: a typed param is a known operand kind for native-operator specialization.
            self.declare_kind(&p.name, kind_of_type(&p.ty));
        }
        for s in &f.body {
            self.emit_stmt(s)?;
        }
        self.pop_scope();
        self.indent -= 1;
        self.line("}");
        Ok(())
    }

    /// Emit one free function, grouping M-RT overloads: a name declared more than once in `items`
    /// becomes a single overload set (emitted once, on first occurrence); a unique name emits
    /// directly. `emitted` guards against re-emitting a set as later overloads are walked.
    pub(super) fn emit_free_fn(
        &mut self,
        items: &[Item],
        f: &FunctionDecl,
        emitted: &mut HashSet<String>,
    ) -> Result<(), String> {
        let group: Vec<&FunctionDecl> = items
            .iter()
            .filter_map(|it| match it {
                Item::Function(g) if g.name == f.name => Some(g),
                _ => None,
            })
            .collect();
        if group.len() > 1 {
            if emitted.insert(f.name.clone()) {
                self.emit_overload_set(&f.name, &group, false)?;
            }
            Ok(())
        } else {
            self.emit_function(f, false)
        }
    }

    /// Emit an overloaded free-function / method set (M-RT dynamic dispatch): each overload's body
    /// under a mangled `<leaf>__ovl_<i>` name, then one dispatcher under the original name that
    /// selects on the runtime argument types (`is_int`/`is_string`/`instanceof`), branches ordered
    /// most-specific-first — so the emitted PHP picks the same body the backends' `select_overload`
    /// does for every resolvable call. (An *ambiguous* call faults in the backends; the PHP chain
    /// would take the first match — a transpile-only divergence on faulting input, never in a runnable
    /// example. Overloads that erase to the same PHP test — `string`/`bytes`, or `List`/`Map`/`Set`,
    /// all of which become PHP `string`/`array` — likewise cannot be told apart in PHP; KNOWN_ISSUES.)
    pub(super) fn emit_overload_set(
        &mut self,
        name: &str,
        ovls: &[&FunctionDecl],
        is_method: bool,
    ) -> Result<(), String> {
        let leaf = last_segment(name).to_string();
        for (i, f) in ovls.iter().enumerate() {
            let mangled = format!("{leaf}__ovl_{i}");
            self.emit_function_named(f, is_method, Some(&mangled))?;
        }
        let kinds: Vec<Vec<ParamKind>> = ovls
            .iter()
            .map(|f| {
                f.params
                    .iter()
                    .map(|p| crate::dispatch::param_kind(&p.ty))
                    .collect()
            })
            .collect();
        let mut order: Vec<usize> = (0..ovls.len()).collect();
        order.sort_by(|&a, &b| {
            if crate::dispatch::dominates(&kinds[a], &kinds[b], &self.class_implements) {
                std::cmp::Ordering::Less
            } else if crate::dispatch::dominates(&kinds[b], &kinds[a], &self.class_implements) {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Equal
            }
        });
        let disp = if self.namespaced && !is_method {
            leaf.clone()
        } else {
            name.to_string()
        };
        // Statics-B: a static overload set's dispatcher must itself be `static` (so `Class::m(args)`
        // is not a "non-static method called statically" fatal), and its branches call the mangled
        // bodies through `self::` rather than `$this->`. All overloads agree on static-ness
        // (`E-OVERLOAD-STATIC-MIX`), so the first is representative. Non-method (free) overloads are
        // never static.
        let is_static = is_method && ovls[0].modifiers.contains(&Modifier::Static);
        let static_prefix = if is_static { "static " } else { "" };
        let ret = self.ret_suffix(&ovls[0].ret);
        self.line(&format!("{static_prefix}function {disp}(...$args){ret} {{"));
        self.indent += 1;
        for &i in &order {
            let test = self.overload_branch_test(&kinds[i]);
            let mangled = format!("{leaf}__ovl_{i}");
            let target = if is_static {
                format!("self::{mangled}")
            } else if is_method {
                format!("$this->{mangled}")
            } else {
                mangled
            };
            self.line(&format!("if ({test}) {{ return {target}(...$args); }}"));
        }
        self.line(&format!(
            "throw new \\LogicException(\"no matching overload for {leaf}\");"
        ));
        self.indent -= 1;
        self.line("}");
        Ok(())
    }

    /// The PHP boolean test that an argument tuple matches one overload's parameter kinds (M-RT).
    pub(super) fn overload_branch_test(&self, kinds: &[ParamKind]) -> String {
        let mut conds = vec![format!("count($args) === {}", kinds.len())];
        for (k, kind) in kinds.iter().enumerate() {
            let a = format!("$args[{k}]");
            conds.push(match kind {
                ParamKind::Int => format!("is_int({a})"),
                ParamKind::Float => format!("is_float({a})"),
                ParamKind::Bool => format!("is_bool({a})"),
                // `bytes` erases to a PHP string, so it shares `string`'s test (indistinguishable).
                ParamKind::Str | ParamKind::Bytes => format!("is_string({a})"),
                // `List`/`Map`/`Set` all erase to a PHP array (indistinguishable).
                ParamKind::List | ParamKind::Map | ParamKind::Set => format!("is_array({a})"),
                ParamKind::Fn => format!("({a} instanceof \\Closure)"),
                ParamKind::Named(n) => {
                    // The built-in `Error` marker is a PHP `\Throwable`; a class/interface/enum uses
                    // its (possibly cross-package FQN) name.
                    let ty = if last_segment(n) == "Error" {
                        "\\Throwable".to_string()
                    } else {
                        php_type_ref(n)
                    };
                    format!("({a} instanceof {ty})")
                }
                ParamKind::Any => "true".to_string(),
            });
        }
        conds.join(" && ")
    }
}
