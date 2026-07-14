//! DEC-208 S2 — typed-generic `Core.Db` result hydration (`queryInto` / `queryOneInto`).
//!
//! A PRE-CHECK desugar (mirrors [`crate::checker::desugar_di`]): it lowers the two type-directed
//! hydration calls into plain, already-working S1 primitives BEFORE the type-checker, so the generated
//! code type-checks like hand-written source and every backend sees the same explicit construction
//! (Inv-5). Byte-identity is trivial (`run ≡ runvm`: both backends run the one desugared AST) and there
//! is no runtime reflection — generics are erased before the backends, so a native could never see `T`'s
//! fields; the field layout is resolved HERE, at compile time, from `T`'s constructor.
//!
//! SURFACE (contextual, no turbofish — DEC-208's `<T>` notation is inferred from the sink type, exactly
//! like DEC-201 empty collections):
//! ```text
//!   List<User> users = stmt.queryInto();      // one User per row
//!   User? one       = stmt.queryOneInto();     // 0 rows → null, 1 → the object, >1 → DbError
//! ```
//! `T` is drawn from the binding's declared type (a typed `var` decl, a `return`, or a lambda expr-body
//! return) — the same three annotation sources `desugar_di` threads. A `queryInto()` with no inferable
//! sink type is `E-DB-INTO-NO-TYPE`; a sink that is not `List<Class>` / `Class?` is `E-DB-INTO-BAD-SINK`.
//!
//! MAPPING (by field NAME, STRICT — DEC-208): `T` is hydrated by calling its constructor, passing every
//! **promoted** constructor parameter extracted from the row by its name via the typed S1 `Row` accessor
//! matching its type (`int`→`getInt`, `string`→`getString`, `float`→`getFloat`, `bool`→`getBool`). The
//! strict semantics are inherited for free from those accessors: a missing column, a type mismatch, or a
//! SQL NULL into a non-optional field each throw `DbError` (row `db.rs` `row_cell`/`get_int`/…). Extra
//! columns are ignored (only named columns are read). Requiring every ctor param to be a promoted field
//! makes "param name == field name" structural, so mapping-by-field-name and construction coincide
//! (`E-DB-HYDRATE-UNPROMOTED` / `E-DB-HYDRATE-NO-CTOR` / `E-DB-HYDRATE-FIELD-TYPE` otherwise).
//!
//! ERROR MECHANISM: the generated helpers are ordinary phorj functions declared `throws DbError`; they
//! reuse the S1 `Statement.query()` + `Row` accessors (each already `throws DbError`) with `?`
//! propagation — no new native, the same catchable model as S1.
//!
//! IMPORT DISCIPLINE (nothing in the wind): active only when `Core.Db` is imported. Under that import
//! `queryInto`/`queryOneInto` are the reserved hydration method names (like `inject` under `Core.DI`);
//! the generated helper takes a `Statement` parameter, so a `queryInto()` on any other receiver is a
//! clean argument-type error rather than silent misbehaviour. Disclosed in KNOWN_ISSUES.
//!
//! INVARIANT — keep the rewriter TOTAL (matching `desugar_di`): `ritem`/`rfn`/`rmember`/`rexpr`/`rstmt`
//! recurse EVERY expression-bearing position so a `queryInto` in any position is either rewritten or
//! reported. A new expression-bearing AST node → add its arm here.

use crate::ast::{
    ctor_plan, BinaryOp, CatchClause, ClassMember, CollKind, CtorParam, Expr, FunctionDecl, Item,
    LambdaBody, MatchArm, MemberSep, Modifier, Param, Program, Stmt, StrPart, Type, Visibility,
};
use crate::diagnostic::{Diagnostic, Stage};
use crate::token::Span;
use std::collections::BTreeMap;

/// Synthetic-span base: every generated node gets a unique `Span.start` at or above this, so it never
/// collides with a real source byte offset (a key into the checker's span-keyed resolution maps —
/// UFCS/`?`-erasure/reified). `usize::MAX / 2` leaves the whole real-source range below and reflect's
/// `usize::MAX` sentinel above, with room for far more nodes than any program could generate.
const SYNTH_BASE: usize = usize::MAX / 2;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    List,
    One,
}

/// One field to hydrate: the promoted-param/field name (= column name), the S1 `Row` accessor for its
/// type, and the field's declared type (for the extracting local's annotation).
struct FieldMap {
    name: String,
    accessor: &'static str,
    ty: Type,
}

/// A resolved hydration helper to synthesize: `phorjQueryInto{List,One}<Class>`.
struct HelperSpec {
    kind: Kind,
    class: String,
    fields: Vec<FieldMap>,
}

/// `phorjQueryIntoList<Class>` / `phorjQueryOneInto<Class>` — a camelCase synthetic free-function name
/// (free functions are `E-NAME-CASE`-checked; `Class` is PascalCase, so the whole name is camelCase).
/// Collision with a hand-written `phorjQueryInto…` is astronomically unlikely and disclosed
/// (KNOWN_ISSUES), matching the `phorjInject…` convention.
fn helper_name(kind: Kind, class: &str) -> String {
    match kind {
        Kind::List => format!("phorjQueryIntoList{class}"),
        Kind::One => format!("phorjQueryOneInto{class}"),
    }
}

/// The S1 `Row` accessor for a hydrated field type: the non-nullable accessor for a scalar (`int`→
/// `getInt`, …) — which throws on a SQL NULL — or the nullable accessor for a `T?` scalar (`int?`→
/// `getIntOrNull`, …) — which admits NULL. Any other type has no column accessor (`None`).
fn accessor_for(ty: &Type) -> Option<&'static str> {
    match ty {
        Type::Named { name, args, .. } if args.is_empty() => match name.as_str() {
            "int" => Some("getInt"),
            "string" => Some("getString"),
            "float" => Some("getFloat"),
            "bool" => Some("getBool"),
            _ => None,
        },
        Type::Optional { inner, .. } => match &**inner {
            Type::Named { name, args, .. } if args.is_empty() => match name.as_str() {
                "int" => Some("getIntOrNull"),
                "string" => Some("getStringOrNull"),
                "float" => Some("getFloatOrNull"),
                "bool" => Some("getBoolOrNull"),
                _ => None,
            },
            _ => None,
        },
        _ => None,
    }
}

/// A short human label for a field type, for the `E-DB-HYDRATE-FIELD-TYPE` message.
fn type_label(t: &Type) -> String {
    match t {
        Type::Named { name, args, .. } if args.is_empty() => name.clone(),
        Type::Optional { inner, .. } => format!("{}?", type_label(inner)),
        _ => "<type>".into(),
    }
}

/// True iff the program imports `Core.Db` in any form (module or member) — the gate for the whole pass.
fn imports_core_db(program: &Program) -> bool {
    program.items.iter().any(|it| {
        matches!(it, Item::Import { path, .. }
            if path.len() >= 2 && path[0] == "Core" && path[1] == "Db")
    })
}

pub fn desugar_db(program: Program) -> Result<Program, Vec<Diagnostic>> {
    // A no-op unless `Core.Db` is imported — so a program that never touches the DB is byte-for-byte
    // unchanged and a user method named `queryInto` outside `Core.Db` is never hijacked.
    if !imports_core_db(&program) {
        return Ok(program);
    }
    // Constructor param layout for every class (flattened `ctor_plan` → inherited promoted params too),
    // computed before destructuring so the resolver needs no `&Program`.
    let mut ctor_params: BTreeMap<String, Vec<CtorParam>> = BTreeMap::new();
    for it in &program.items {
        if let Item::Class(c) = it {
            let params: Vec<CtorParam> = ctor_plan(&program, &c.name)
                .into_iter()
                .flat_map(|(ps, _)| ps)
                .collect();
            ctor_params.insert(c.name.clone(), params);
        }
    }
    let Program {
        package,
        items,
        span,
    } = program;
    let mut db = Db {
        ctor_params: &ctor_params,
        helpers: BTreeMap::new(),
        diags: Vec::new(),
        current_ret: None,
        next_span: SYNTH_BASE,
    };
    let mut items: Vec<Item> = items.into_iter().map(|it| db.ritem(it)).collect();
    if !db.diags.is_empty() {
        return Err(db.diags);
    }
    // Append one hydration helper per (kind, class) used, sorted by name (Inv-10 determinism).
    let helpers = std::mem::take(&mut db.helpers);
    for spec in helpers.values() {
        let f = db.synth_helper(spec);
        items.push(f);
    }
    Ok(Program {
        package,
        items,
        span,
    })
}

struct Db<'a> {
    ctor_params: &'a BTreeMap<String, Vec<CtorParam>>,
    /// helper name → spec, deduped (one helper per (kind, class)) and iterated sorted.
    helpers: BTreeMap<String, HelperSpec>,
    diags: Vec<Diagnostic>,
    /// The enclosing function/method/lambda return type — the annotation source for `return
    /// stmt.queryInto();`. Saved/restored across every function and lambda.
    current_ret: Option<Type>,
    /// Monotonic synthetic-span allocator (see [`SYNTH_BASE`]).
    next_span: usize,
}

impl Db<'_> {
    fn sp(&mut self) -> Span {
        let s = self.next_span;
        self.next_span += 1;
        Span {
            start: s,
            len: 0,
            line: 0,
            col: 0,
        }
    }

    fn diag(&mut self, span: Span, msg: String, code: &'static str, hint: String) {
        self.diags.push(
            Diagnostic::new(Stage::Type, msg, span.line, span.col)
                .with_code(code)
                .with_hint(hint),
        );
    }

    /// A non-cascading placeholder returned after a diagnostic (the non-empty `diags` aborts the
    /// pipeline before the checker ever sees it).
    fn placeholder(&mut self) -> Expr {
        Expr::Null(self.sp())
    }

    // ── recognition + rewrite ────────────────────────────────────────────────────────────────────

    /// If `callee` is a nullary `recv.queryInto()` / `recv.queryOneInto()` member call, its kind.
    fn query_into_kind(callee: &Expr) -> Option<Kind> {
        match callee {
            Expr::Member {
                name, safe: false, ..
            } => match name.as_str() {
                "queryInto" => Some(Kind::List),
                "queryOneInto" => Some(Kind::One),
                _ => None,
            },
            _ => None,
        }
    }

    /// Rewrite `recv.queryInto()` / `recv.queryOneInto()` into a call to the synthesized hydration
    /// helper (`helper(recv)`), drawing `T` from `expected`. On any resolution failure a diagnostic is
    /// recorded and a placeholder is returned (the pipeline aborts on the non-empty `diags`).
    fn rewrite(&mut self, callee: Expr, kind: Kind, expected: Option<&Type>, span: Span) -> Expr {
        let Expr::Member { object, .. } = callee else {
            unreachable!("query_into_kind guarantees a Member callee");
        };
        let recv = self.rexpr(*object);
        let expected = expected.cloned();
        let Some(class) = self.resolve_target(kind, expected.as_ref(), span) else {
            return self.placeholder();
        };
        let Some(name) = self.ensure_helper(kind, &class, span) else {
            return self.placeholder();
        };
        let helper = Expr::Ident(name, span);
        Expr::Call {
            callee: Box::new(helper),
            args: vec![recv],
            span,
        }
    }

    /// Resolve the row class `T` from the sink type: `List<T>` for `queryInto`, `T?` for `queryOneInto`.
    fn resolve_target(
        &mut self,
        kind: Kind,
        expected: Option<&Type>,
        span: Span,
    ) -> Option<String> {
        let (method, want) = match kind {
            Kind::List => ("queryInto", "List<Row>"),
            Kind::One => ("queryOneInto", "Row?"),
        };
        let Some(expected) = expected else {
            self.diag(
                span,
                format!("`{method}()` has no type to infer its row class from"),
                "E-DB-INTO-NO-TYPE",
                format!(
                    "bind it to a typed declaration whose class it maps into — e.g. `{want} rows = stmt.{method}();`"
                ),
            );
            return None;
        };
        let inner: &Type = match (kind, expected) {
            (Kind::List, Type::Named { name, args, .. }) if name == "List" && args.len() == 1 => {
                &args[0]
            }
            (Kind::One, Type::Optional { inner, .. }) => inner,
            _ => {
                self.diag(
                    span,
                    format!(
                        "`{method}()` maps rows into `{want}`, but the binding's type is `{}`",
                        type_label(expected)
                    ),
                    "E-DB-INTO-BAD-SINK",
                    match kind {
                        Kind::List => "declare the binding `List<YourClass>`".into(),
                        Kind::One => "declare the binding `YourClass?`".into(),
                    },
                );
                return None;
            }
        };
        match inner {
            Type::Named { name, args, .. }
                if args.is_empty() && self.ctor_params.contains_key(name) =>
            {
                Some(name.clone())
            }
            _ => {
                self.diag(
                    span,
                    format!(
                        "`{method}()` must map into a user class; `{}` is not one",
                        type_label(inner)
                    ),
                    "E-DB-INTO-BAD-SINK",
                    "name a class with a promoted-field constructor as the row type".into(),
                );
                None
            }
        }
    }

    /// Ensure a helper spec exists for `(kind, class)` (validating its constructor), returning its name.
    fn ensure_helper(&mut self, kind: Kind, class: &str, span: Span) -> Option<String> {
        let name = helper_name(kind, class);
        if self.helpers.contains_key(&name) {
            return Some(name);
        }
        let params = self.ctor_params.get(class).cloned().unwrap_or_default();
        if params.is_empty() {
            self.diag(
                span,
                format!("cannot hydrate `{class}`: it has no constructor to map columns into"),
                "E-DB-HYDRATE-NO-CTOR",
                format!(
                    "give `{class}` a promoted-field constructor — `constructor(public string name, public int age) {{}}`"
                ),
            );
            return None;
        }
        let mut fields = Vec::new();
        let mut ok = true;
        for p in &params {
            let promoted = p.modifiers.iter().any(|m| {
                matches!(
                    m,
                    Modifier::Public | Modifier::Private | Modifier::Protected
                )
            });
            if !promoted {
                ok = false;
                self.diag(
                    p.span,
                    format!(
                        "cannot hydrate `{class}`: constructor parameter `{}` is not a promoted field",
                        p.name
                    ),
                    "E-DB-HYDRATE-UNPROMOTED",
                    "every constructor parameter must be a promoted field (carry `public`/`private`/`protected`) so its name is the column name".into(),
                );
                continue;
            }
            match accessor_for(&p.ty) {
                Some(accessor) => fields.push(FieldMap {
                    name: p.name.clone(),
                    accessor,
                    ty: p.ty.clone(),
                }),
                None => {
                    ok = false;
                    self.diag(
                        p.span,
                        format!(
                            "cannot hydrate field `{}` of `{class}`: type `{}` has no DB column accessor",
                            p.name,
                            type_label(&p.ty)
                        ),
                        "E-DB-HYDRATE-FIELD-TYPE",
                        "a hydrated field must be `int`, `string`, `float`, `bool`, or their `?` forms (e.g. `int?`)".into(),
                    );
                }
            }
        }
        if !ok {
            return None;
        }
        self.helpers.insert(
            name.clone(),
            HelperSpec {
                kind,
                class: class.to_string(),
                fields,
            },
        );
        Some(name)
    }

    // ── synthesis (all nodes get unique synthetic spans) ─────────────────────────────────────────

    fn named(&mut self, n: &str) -> Type {
        let span = self.sp();
        Type::Named {
            name: n.into(),
            args: Vec::new(),
            span,
        }
    }

    fn generic1(&mut self, n: &str, arg: Type) -> Type {
        let span = self.sp();
        Type::Named {
            name: n.into(),
            args: vec![arg],
            span,
        }
    }

    fn list_ty(&mut self, class: &str) -> Type {
        let c = self.named(class);
        self.generic1("List", c)
    }

    fn opt_ty(&mut self, class: &str) -> Type {
        let c = self.named(class);
        let span = self.sp();
        Type::Optional {
            inner: Box::new(c),
            span,
        }
    }

    fn row_list_ty(&mut self) -> Type {
        let r = self.named("Row");
        self.generic1("List", r)
    }

    /// A fresh-span clone of a field type (scalars / optional scalars) — keeps every synthetic node's
    /// span unique even though the type text came from user source.
    fn retype(&mut self, t: &Type) -> Type {
        match t {
            Type::Named { name, args, .. } if args.is_empty() => self.named(name),
            Type::Optional { inner, .. } => {
                let i = self.retype(inner);
                let span = self.sp();
                Type::Optional {
                    inner: Box::new(i),
                    span,
                }
            }
            other => other.clone(),
        }
    }

    fn ident(&mut self, n: &str) -> Expr {
        let span = self.sp();
        Expr::Ident(n.into(), span)
    }

    fn str_lit(&mut self, s: &str) -> Expr {
        let span = self.sp();
        Expr::Str(vec![StrPart::Literal(s.into())], span)
    }

    fn int_lit(&mut self, n: i64) -> Expr {
        let span = self.sp();
        Expr::Int(n, span)
    }

    /// `obj.method(args)` — an instance method call (UFCS-resolved downstream).
    fn member_call(&mut self, obj: Expr, method: &str, args: Vec<Expr>) -> Expr {
        let msp = self.sp();
        let callee = Expr::Member {
            object: Box::new(obj),
            name: method.into(),
            safe: false,
            sep: MemberSep::Dot,
            span: msp,
        };
        let span = self.sp();
        Expr::Call {
            callee: Box::new(callee),
            args,
            span,
        }
    }

    /// `Qualifier.method(args)` — a qualified module/native call (`List.append`, `List.length`).
    fn qual_call(&mut self, qualifier: &str, method: &str, args: Vec<Expr>) -> Expr {
        let q = self.ident(qualifier);
        self.member_call(q, method, args)
    }

    fn propagate(&mut self, inner: Expr) -> Expr {
        let span = self.sp();
        Expr::Propagate {
            inner: Box::new(inner),
            span,
        }
    }

    /// `new Class(args)` — construction (an `Expr::New` wrapping the plain `Call`, like the parser emits).
    fn new_obj(&mut self, class: &str, args: Vec<Expr>) -> Expr {
        let callee = self.ident(class);
        let csp = self.sp();
        let call = Expr::Call {
            callee: Box::new(callee),
            args,
            span: csp,
        };
        let span = self.sp();
        Expr::New(Box::new(call), span)
    }

    /// The per-field extracting locals (`T name = row.getT("name")?;`) and the constructor arguments
    /// (`Ident(name)` for each). Locals are named after the fields — disjoint from the fixed `phorj…`
    /// locals below.
    fn field_binds(&mut self, spec: &HelperSpec, row_var: &str) -> (Vec<Stmt>, Vec<Expr>) {
        let fields: Vec<(String, &'static str, Type)> = spec
            .fields
            .iter()
            .map(|f| (f.name.clone(), f.accessor, f.ty.clone()))
            .collect();
        let mut binds = Vec::new();
        let mut args = Vec::new();
        for (name, accessor, ty) in fields {
            let row = self.ident(row_var);
            let col = self.str_lit(&name);
            let acc = self.member_call(row, accessor, vec![col]);
            let init = self.propagate(acc);
            let ty = self.retype(&ty);
            let span = self.sp();
            binds.push(Stmt::VarDecl {
                ty,
                name: name.clone(),
                init,
                mutable: false,
                span,
            });
            args.push(self.ident(&name));
        }
        (binds, args)
    }

    fn synth_helper(&mut self, spec: &HelperSpec) -> Item {
        let class = spec.class.clone();
        let stmt_ty = self.named("Statement");
        let psp = self.sp();
        let param = Param {
            ty: stmt_ty,
            name: "phorjStmt".into(),
            default: None,
            span: psp,
        };
        let (ret, body) = match spec.kind {
            Kind::List => {
                let ty = self.list_ty(&class);
                (ty, self.list_body(spec))
            }
            Kind::One => {
                let ty = self.opt_ty(&class);
                (ty, self.one_body(spec))
            }
        };
        let throws = vec![self.named("DbError")];
        let span = self.sp();
        Item::Function(FunctionDecl {
            modifiers: Vec::new(),
            attrs: Vec::new(),
            vis: Visibility::Public,
            name: helper_name(spec.kind, &class),
            type_params: Vec::new(),
            type_param_bounds: Vec::new(),
            params: vec![param],
            ret: Some(ret),
            throws,
            body,
            foreign: false,
            generic_ret_from_param: None,
            span,
        })
    }

    /// `List<Row> phorjRows = phorjStmt.query()?;` — the shared first statement of both helpers.
    fn query_rows_stmt(&mut self) -> Stmt {
        let s = self.ident("phorjStmt");
        let q = self.member_call(s, "query", vec![]);
        let init = self.propagate(q);
        let ty = self.row_list_ty();
        let span = self.sp();
        Stmt::VarDecl {
            ty,
            name: "phorjRows".into(),
            init,
            mutable: false,
            span,
        }
    }

    fn list_body(&mut self, spec: &HelperSpec) -> Vec<Stmt> {
        let mut body = Vec::new();
        body.push(self.query_rows_stmt());
        // mutable List<Class> phorjOut = new List<Class>();
        let out_ty = self.list_ty(&spec.class);
        let ct = self.named(&spec.class);
        let coll_span = self.sp();
        let coll = Expr::NewColl {
            kind: CollKind::List,
            args: vec![ct],
            span: coll_span,
        };
        let out_span = self.sp();
        body.push(Stmt::VarDecl {
            ty: out_ty,
            name: "phorjOut".into(),
            init: coll,
            mutable: true,
            span: out_span,
        });
        // for (Row phorjRow in phorjRows) { <field binds>; phorjOut = List.append(phorjOut, new Class(..)); }
        let (mut loop_body, args) = self.field_binds(spec, "phorjRow");
        let newc = self.new_obj(&spec.class, args);
        let out_ref = self.ident("phorjOut");
        let appended = self.qual_call("List", "append", vec![out_ref, newc]);
        let target = self.ident("phorjOut");
        let assign_span = self.sp();
        loop_body.push(Stmt::Assign {
            target,
            value: appended,
            span: assign_span,
        });
        let for_ty = self.named("Row");
        let iter = self.ident("phorjRows");
        let for_span = self.sp();
        body.push(Stmt::For {
            ty: for_ty,
            name: "phorjRow".into(),
            val: None,
            iter,
            body: loop_body,
            span: for_span,
        });
        let ret_val = self.ident("phorjOut");
        let ret_span = self.sp();
        body.push(Stmt::Return {
            value: Some(ret_val),
            span: ret_span,
        });
        body
    }

    fn one_body(&mut self, spec: &HelperSpec) -> Vec<Stmt> {
        let mut body = Vec::new();
        body.push(self.query_rows_stmt());
        // int phorjN = List.length(phorjRows);
        let rows_ref = self.ident("phorjRows");
        let len = self.qual_call("List", "length", vec![rows_ref]);
        let int_ty = self.named("int");
        let n_span = self.sp();
        body.push(Stmt::VarDecl {
            ty: int_ty,
            name: "phorjN".into(),
            init: len,
            mutable: false,
            span: n_span,
        });
        // if (phorjN == 0) { return null; }
        let n0 = self.ident("phorjN");
        let zero = self.int_lit(0);
        let eq_span = self.sp();
        let eq = Expr::Binary {
            op: BinaryOp::Eq,
            lhs: Box::new(n0),
            rhs: Box::new(zero),
            span: eq_span,
        };
        let null_val = self.placeholder();
        let ret_span = self.sp();
        let null_ret = Stmt::Return {
            value: Some(null_val),
            span: ret_span,
        };
        let if0_span = self.sp();
        body.push(Stmt::If {
            cond: eq,
            bind: None,
            then_block: vec![null_ret],
            else_block: None,
            span: if0_span,
        });
        // if (phorjN > 1) { throw new DbError("…"); }
        let n1 = self.ident("phorjN");
        let one = self.int_lit(1);
        let gt_span = self.sp();
        let gt = Expr::Binary {
            op: BinaryOp::Gt,
            lhs: Box::new(n1),
            rhs: Box::new(one),
            span: gt_span,
        };
        let msg = self.str_lit(&format!(
            "Core.Db.queryOneInto: expected at most one row for `{}`",
            spec.class
        ));
        let dberr = self.new_obj("DbError", vec![msg]);
        let throw_span = self.sp();
        let throw = Stmt::Throw {
            value: dberr,
            span: throw_span,
        };
        let if1_span = self.sp();
        body.push(Stmt::If {
            cond: gt,
            bind: None,
            then_block: vec![throw],
            else_block: None,
            span: if1_span,
        });
        // Row phorjRow = phorjRows[0];
        let rows_ref2 = self.ident("phorjRows");
        let idx0 = self.int_lit(0);
        let idx_span = self.sp();
        let index = Expr::Index {
            object: Box::new(rows_ref2),
            index: Box::new(idx0),
            span: idx_span,
        };
        let row_ty = self.named("Row");
        let row_span = self.sp();
        body.push(Stmt::VarDecl {
            ty: row_ty,
            name: "phorjRow".into(),
            init: index,
            mutable: false,
            span: row_span,
        });
        // <field binds>; return new Class(..);
        let (binds, args) = self.field_binds(spec, "phorjRow");
        body.extend(binds);
        let newc = self.new_obj(&spec.class, args);
        let ret_span = self.sp();
        body.push(Stmt::Return {
            value: Some(newc),
            span: ret_span,
        });
        body
    }

    // ── the total walk (mirrors desugar_di) ──────────────────────────────────────────────────────

    fn ritem(&mut self, it: Item) -> Item {
        match it {
            Item::Function(mut f) => {
                self.rfn(&mut f);
                Item::Function(f)
            }
            Item::Class(mut c) => {
                for m in &mut c.members {
                    self.rmember(m);
                }
                Item::Class(c)
            }
            Item::Trait(mut t) => {
                for m in &mut t.members {
                    self.rmember(m);
                }
                Item::Trait(t)
            }
            other => other,
        }
    }

    fn rfn(&mut self, f: &mut FunctionDecl) {
        let prev_ret = std::mem::replace(&mut self.current_ret, f.ret.clone());
        let body = std::mem::take(&mut f.body);
        f.body = self.rblock(body);
        for p in &mut f.params {
            if let Some(d) = p.default.take() {
                p.default = Some(Box::new(self.rexpr(*d)));
            }
        }
        self.current_ret = prev_ret;
    }

    fn rmember(&mut self, m: &mut ClassMember) {
        match m {
            ClassMember::Method(f) => self.rfn(f),
            ClassMember::Field { init, .. } => {
                if let Some(e) = init.take() {
                    *init = Some(self.rexpr(e));
                }
            }
            ClassMember::Constructor { body, .. } => {
                let b = std::mem::take(body);
                *body = self.rblock(b);
            }
            ClassMember::Hook { get, set, .. } => {
                if let Some(e) = get.take() {
                    *get = Some(self.rexpr(e));
                }
                if let Some((_param, stmts)) = set {
                    let b = std::mem::take(stmts);
                    *stmts = self.rblock(b);
                }
            }
        }
    }

    fn rblock(&mut self, stmts: Vec<Stmt>) -> Vec<Stmt> {
        stmts.into_iter().map(|s| self.rstmt(s)).collect()
    }

    /// An expression in an *annotation position* (typed var init, `return`, lambda expr-body): a
    /// `queryInto`/`queryOneInto` here draws `T` from `expected`. `Type::Infer` (`var`) is not an
    /// annotation → stripped to `None`.
    fn rexpr_expected(&mut self, e: Expr, expected: Option<&Type>) -> Expr {
        let expected = expected.filter(|t| !matches!(t, Type::Infer(_)));
        match e {
            Expr::Call { callee, args, span } if args.is_empty() => {
                match Self::query_into_kind(&callee) {
                    Some(kind) => self.rewrite(*callee, kind, expected, span),
                    None => {
                        let callee = Box::new(self.rexpr(*callee));
                        Expr::Call {
                            callee,
                            args: Vec::new(),
                            span,
                        }
                    }
                }
            }
            // `List<User> u = stmt.queryInto()?;` — a `?`-propagation in annotation position: look
            // THROUGH the `?` so the sink type still reaches the recognizer, then keep the `?` on the
            // rewritten throwing helper call (the idiomatic form inside a `throws DbError` function).
            Expr::Propagate { inner, span } => match *inner {
                Expr::Call {
                    callee,
                    args,
                    span: cspan,
                } if args.is_empty() && Self::query_into_kind(&callee).is_some() => {
                    let kind = Self::query_into_kind(&callee).expect("guarded above");
                    let rewritten = self.rewrite(*callee, kind, expected, cspan);
                    Expr::Propagate {
                        inner: Box::new(rewritten),
                        span,
                    }
                }
                other => Expr::Propagate {
                    inner: Box::new(self.rexpr(other)),
                    span,
                },
            },
            other => self.rexpr(other),
        }
    }

    fn rexpr(&mut self, e: Expr) -> Expr {
        match e {
            // A nullary call may be a `recv.queryInto()` — but here there is no annotation, so it is
            // `E-DB-INTO-NO-TYPE` (via `rewrite` with `expected = None`).
            Expr::Call { callee, args, span } if args.is_empty() => {
                match Self::query_into_kind(&callee) {
                    Some(kind) => self.rewrite(*callee, kind, None, span),
                    None => {
                        let callee = Box::new(self.rexpr(*callee));
                        Expr::Call {
                            callee,
                            args: Vec::new(),
                            span,
                        }
                    }
                }
            }
            Expr::Call { callee, args, span } => Expr::Call {
                callee: Box::new(self.rexpr(*callee)),
                args: args.into_iter().map(|a| self.rexpr(a)).collect(),
                span,
            },
            Expr::ParentCall {
                ancestor,
                method,
                args,
                span,
            } => Expr::ParentCall {
                ancestor,
                method,
                args: args.into_iter().map(|a| self.rexpr(a)).collect(),
                span,
            },
            Expr::OverloadSelect { ty, call, span } => Expr::OverloadSelect {
                ty,
                call: Box::new(self.rexpr(*call)),
                span,
            },
            Expr::Str(parts, span) => Expr::Str(self.rparts(parts), span),
            Expr::Html(parts, span) => Expr::Html(self.rparts(parts), span),
            Expr::TaggedTemplate { tag, parts, span } => Expr::TaggedTemplate {
                tag,
                parts: self.rparts(parts),
                span,
            },
            Expr::List(items, span) => {
                Expr::List(items.into_iter().map(|e| self.rexpr(e)).collect(), span)
            }
            Expr::Map(pairs, span) => Expr::Map(
                pairs
                    .into_iter()
                    .map(|(k, v)| (self.rexpr(k), self.rexpr(v)))
                    .collect(),
                span,
            ),
            Expr::NewColl { kind, args, span } => Expr::NewColl { kind, args, span },
            Expr::Unary { op, expr, span } => Expr::Unary {
                op,
                expr: Box::new(self.rexpr(*expr)),
                span,
            },
            Expr::Binary { op, lhs, rhs, span } => Expr::Binary {
                op,
                lhs: Box::new(self.rexpr(*lhs)),
                rhs: Box::new(self.rexpr(*rhs)),
                span,
            },
            Expr::InstanceOf {
                value,
                type_name,
                span,
            } => Expr::InstanceOf {
                value: Box::new(self.rexpr(*value)),
                type_name,
                span,
            },
            Expr::Cast {
                value,
                type_name,
                span,
            } => Expr::Cast {
                value: Box::new(self.rexpr(*value)),
                type_name,
                span,
            },
            Expr::Member {
                object,
                name,
                safe,
                sep,
                span,
            } => Expr::Member {
                object: Box::new(self.rexpr(*object)),
                name,
                safe,
                sep,
                span,
            },
            Expr::Index {
                object,
                index,
                span,
            } => Expr::Index {
                object: Box::new(self.rexpr(*object)),
                index: Box::new(self.rexpr(*index)),
                span,
            },
            Expr::Force { inner, span } => Expr::Force {
                inner: Box::new(self.rexpr(*inner)),
                span,
            },
            Expr::Propagate { inner, span } => Expr::Propagate {
                inner: Box::new(self.rexpr(*inner)),
                span,
            },
            Expr::Match {
                scrutinee,
                arms,
                span,
            } => Expr::Match {
                scrutinee: Box::new(self.rexpr(*scrutinee)),
                arms: arms
                    .into_iter()
                    .map(|a| MatchArm {
                        pattern: a.pattern,
                        guard: a.guard.map(|g| self.rexpr(g)),
                        body: self.rexpr(a.body),
                        span: a.span,
                    })
                    .collect(),
                span,
            },
            Expr::Range {
                start,
                end,
                inclusive,
                span,
            } => Expr::Range {
                start: Box::new(self.rexpr(*start)),
                end: Box::new(self.rexpr(*end)),
                inclusive,
                span,
            },
            Expr::If {
                cond,
                then_expr,
                else_expr,
                span,
            } => Expr::If {
                cond: Box::new(self.rexpr(*cond)),
                then_expr: Box::new(self.rexpr(*then_expr)),
                else_expr: Box::new(self.rexpr(*else_expr)),
                span,
            },
            Expr::Lambda {
                params,
                ret,
                body,
                span,
            } => {
                let prev_ret = std::mem::replace(&mut self.current_ret, ret.clone());
                let new_body = match body {
                    LambdaBody::Expr(e) => {
                        let expected = self.current_ret.clone();
                        LambdaBody::Expr(Box::new(self.rexpr_expected(*e, expected.as_ref())))
                    }
                    LambdaBody::Block(stmts) => LambdaBody::Block(self.rblock(stmts)),
                };
                self.current_ret = prev_ret;
                Expr::Lambda {
                    params,
                    ret,
                    body: new_body,
                    span,
                }
            }
            Expr::CloneWith {
                object,
                fields,
                span,
            } => Expr::CloneWith {
                object: Box::new(self.rexpr(*object)),
                fields: fields
                    .into_iter()
                    .map(|(n, e)| (n, self.rexpr(e)))
                    .collect(),
                span,
            },
            Expr::New(inner, span) => Expr::New(Box::new(self.rexpr(*inner)), span),
            Expr::Spawn { call, span } => Expr::Spawn {
                call: Box::new(self.rexpr(*call)),
                span,
            },
            // Leaves (Int/Float/Decimal/Bool/Null/Bytes/Ident/This) and `Inject` (already expanded by
            // `desugar_di` upstream) carry no `queryInto` to rewrite.
            leaf => leaf,
        }
    }

    fn rparts(&mut self, parts: Vec<StrPart>) -> Vec<StrPart> {
        parts
            .into_iter()
            .map(|p| match p {
                StrPart::Expr(e) => StrPart::Expr(Box::new(self.rexpr(*e))),
                lit => lit,
            })
            .collect()
    }

    fn rstmt(&mut self, s: Stmt) -> Stmt {
        match s {
            Stmt::VarDecl {
                ty,
                name,
                init,
                mutable,
                span,
            } => {
                let init = self.rexpr_expected(init, Some(&ty));
                Stmt::VarDecl {
                    ty,
                    name,
                    init,
                    mutable,
                    span,
                }
            }
            Stmt::Assign {
                target,
                value,
                span,
            } => Stmt::Assign {
                target: self.rexpr(target),
                value: self.rexpr(value),
                span,
            },
            Stmt::Return { value, span } => Stmt::Return {
                value: value.map(|e| {
                    let expected = self.current_ret.clone();
                    self.rexpr_expected(e, expected.as_ref())
                }),
                span,
            },
            Stmt::If {
                cond,
                bind,
                then_block,
                else_block,
                span,
            } => Stmt::If {
                cond: self.rexpr(cond),
                bind,
                then_block: self.rblock(then_block),
                else_block: else_block.map(|b| self.rblock(b)),
                span,
            },
            Stmt::For {
                ty,
                name,
                val,
                iter,
                body,
                span,
            } => Stmt::For {
                ty,
                name,
                val,
                iter: self.rexpr(iter),
                body: self.rblock(body),
                span,
            },
            Stmt::While {
                cond,
                body,
                post_cond,
                span,
            } => Stmt::While {
                cond: self.rexpr(cond),
                body: self.rblock(body),
                post_cond,
                span,
            },
            Stmt::CFor {
                init,
                cond,
                step,
                body,
                span,
            } => Stmt::CFor {
                init: init.map(|s| Box::new(self.rstmt(*s))),
                cond: cond.map(|e| self.rexpr(e)),
                step: step.map(|s| Box::new(self.rstmt(*s))),
                body: self.rblock(body),
                span,
            },
            Stmt::Block(stmts, span) => Stmt::Block(self.rblock(stmts), span),
            Stmt::Destructure {
                pat,
                init,
                else_block,
                span,
            } => Stmt::Destructure {
                pat,
                init: self.rexpr(init),
                else_block: else_block.map(|b| self.rblock(b)),
                span,
            },
            Stmt::Expr(e, span) => Stmt::Expr(self.rexpr(e), span),
            Stmt::Discard(e, span) => Stmt::Discard(self.rexpr(e), span),
            Stmt::Throw { value, span } => Stmt::Throw {
                value: self.rexpr(value),
                span,
            },
            Stmt::Try {
                body,
                catches,
                finally_block,
                span,
            } => Stmt::Try {
                body: self.rblock(body),
                catches: catches
                    .into_iter()
                    .map(|c| CatchClause {
                        ty: c.ty,
                        name: c.name,
                        body: self.rblock(c.body),
                        span: c.span,
                    })
                    .collect(),
                finally_block: finally_block.map(|b| self.rblock(b)),
                span,
            },
            leaf => leaf, // Break / Continue carry no expression
        }
    }
}
