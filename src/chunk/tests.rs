use super::*;
use crate::value::Value;

#[test]
fn add_const_returns_sequential_indices() {
    let mut c = Chunk::new();
    assert_eq!(c.add_const(Value::Int(1)), 0);
    assert_eq!(c.add_const(Value::Int(2)), 1);
    assert_eq!(c.consts.len(), 2);
}

#[test]
fn add_const_interns_duplicate_scalars() {
    let mut c = Chunk::new();
    // Repeated scalars reuse their slot: the pool grows with distinct values, not occurrences.
    assert_eq!(c.add_const(Value::Int(7)), 0);
    assert_eq!(c.add_const(Value::Int(7)), 0); // same int → same index
    assert_eq!(c.add_const(Value::Float(1.5)), 1);
    assert_eq!(c.add_const(Value::Float(1.5)), 1); // bit-equal float → same index
    assert_eq!(c.add_const(Value::Str("hi".into())), 2);
    assert_eq!(c.add_const(Value::Str("hi".into())), 2); // equal string → same index
    assert_eq!(c.add_const(Value::Int(8)), 3); // distinct value → new slot
    assert_eq!(c.consts.len(), 4);
}

#[test]
fn add_const_does_not_intern_composites() {
    let mut c = Chunk::new();
    // Lists have no dedup key — each gets a fresh slot even if structurally equal.
    assert_eq!(c.add_const(Value::List(vec![Value::Int(1)].into())), 0);
    assert_eq!(c.add_const(Value::List(vec![Value::Int(1)].into())), 1);
    assert_eq!(c.consts.len(), 2);
}

#[test]
fn emit_tracks_code_and_lines() {
    let mut c = Chunk::new();
    c.emit(Op::Const(0), 1);
    c.emit(Op::Return, 2);
    assert_eq!(c.code.len(), 2);
    assert_eq!(c.lines, vec![1, 2]);
}

#[test]
fn validate_accepts_a_well_formed_program() {
    let mut c = Chunk::new();
    let k = c.add_const(Value::Int(1));
    c.emit(Op::Const(k), 1);
    c.emit(Op::Jump(2), 1); // == code_len after the next emit: legal "fall off → return"
    c.emit(Op::Return, 1);
    let prog = BytecodeProgram {
        functions: vec![Function {
            name: "main".into(),
            arity: 0,
            n_captures: 0,
            dyn_params: Vec::new(),

            unchecked: false,
            chunk: c,
        }],
        main: 0,
        main_is_static: false,
        main_params: 0,
        enum_descs: Vec::new(),
        class_descs: Vec::new(),
        names: Vec::new(),
        methods: HashMap::new(),
        class_implements: BTreeMap::new(),
        class_tables: crate::native::ClassTables::default(),
        static_inits: Vec::new(),
        overloads: Vec::new(),
        method_overloads: std::collections::HashMap::new(),
    };
    assert_eq!(prog.validate(), Ok(()));
}

#[test]
fn validate_rejects_out_of_range_const() {
    let mut c = Chunk::new(); // empty const pool
    c.emit(Op::Const(99), 1);
    c.emit(Op::Return, 1);
    let prog = BytecodeProgram {
        functions: vec![Function {
            name: "main".into(),
            arity: 0,
            n_captures: 0,
            dyn_params: Vec::new(),

            unchecked: false,
            chunk: c,
        }],
        main: 0,
        main_is_static: false,
        main_params: 0,
        enum_descs: Vec::new(),
        class_descs: Vec::new(),
        names: Vec::new(),
        methods: HashMap::new(),
        class_implements: BTreeMap::new(),
        class_tables: crate::native::ClassTables::default(),
        static_inits: Vec::new(),
        overloads: Vec::new(),
        method_overloads: std::collections::HashMap::new(),
    };
    let err = prog.validate().unwrap_err();
    assert!(err.contains("invalid bytecode"), "{err}");
    assert!(err.contains("const index 99"), "{err}");
}

#[test]
fn validate_rejects_out_of_range_call_and_bad_main() {
    let mut c = Chunk::new();
    c.emit(Op::Call(7), 1); // only 1 function exists
    c.emit(Op::Return, 1);
    let prog = BytecodeProgram {
        functions: vec![Function {
            name: "main".into(),
            arity: 0,
            n_captures: 0,
            dyn_params: Vec::new(),

            unchecked: false,
            chunk: c,
        }],
        main: 0,
        main_is_static: false,
        main_params: 0,
        enum_descs: Vec::new(),
        class_descs: Vec::new(),
        names: Vec::new(),
        methods: HashMap::new(),
        class_implements: BTreeMap::new(),
        class_tables: crate::native::ClassTables::default(),
        static_inits: Vec::new(),
        overloads: Vec::new(),
        method_overloads: std::collections::HashMap::new(),
    };
    assert!(prog.validate().unwrap_err().contains("call target 7"));

    let bad_main = BytecodeProgram {
        functions: vec![],
        main: 0,
        main_is_static: false,
        main_params: 0,
        enum_descs: Vec::new(),
        class_descs: Vec::new(),
        names: Vec::new(),
        methods: HashMap::new(),
        class_implements: BTreeMap::new(),
        class_tables: crate::native::ClassTables::default(),
        static_inits: Vec::new(),
        overloads: Vec::new(),
        method_overloads: std::collections::HashMap::new(),
    };
    assert!(bad_main.validate().unwrap_err().contains("main index 0"));
}

#[test]
fn validate_rejects_out_of_range_enum_desc() {
    let mut c = Chunk::new();
    c.emit(Op::MakeEnum(3), 1); // no descriptors in the table
    c.emit(Op::Return, 1);
    let prog = BytecodeProgram {
        functions: vec![Function {
            name: "main".into(),
            arity: 0,
            n_captures: 0,
            dyn_params: Vec::new(),

            unchecked: false,
            chunk: c,
        }],
        main: 0,
        main_is_static: false,
        main_params: 0,
        enum_descs: Vec::new(),
        class_descs: Vec::new(),
        names: Vec::new(),
        methods: HashMap::new(),
        class_implements: BTreeMap::new(),
        class_tables: crate::native::ClassTables::default(),
        static_inits: Vec::new(),
        overloads: Vec::new(),
        method_overloads: std::collections::HashMap::new(),
    };
    let err = prog.validate().unwrap_err();
    assert!(err.contains("enum descriptor index 3"), "{err}");
}

#[test]
fn validate_rejects_out_of_range_class_and_field() {
    let mut c = Chunk::new();
    c.emit(Op::MakeInstance(2), 1); // no class descriptors
    c.emit(Op::Return, 1);
    let prog = BytecodeProgram {
        functions: vec![Function {
            name: "main".into(),
            arity: 0,
            n_captures: 0,
            dyn_params: Vec::new(),

            unchecked: false,
            chunk: c,
        }],
        main: 0,
        main_is_static: false,
        main_params: 0,
        enum_descs: Vec::new(),
        class_descs: Vec::new(),
        names: Vec::new(),
        methods: HashMap::new(),
        class_implements: BTreeMap::new(),
        class_tables: crate::native::ClassTables::default(),
        static_inits: Vec::new(),
        overloads: Vec::new(),
        method_overloads: std::collections::HashMap::new(),
    };
    assert!(prog
        .validate()
        .unwrap_err()
        .contains("class descriptor index 2"));

    let mut c2 = Chunk::new();
    c2.emit(Op::GetField(5), 1); // empty name pool
    c2.emit(Op::Return, 1);
    let prog2 = BytecodeProgram {
        functions: vec![Function {
            name: "main".into(),
            arity: 0,
            n_captures: 0,
            dyn_params: Vec::new(),

            unchecked: false,
            chunk: c2,
        }],
        main: 0,
        main_is_static: false,
        main_params: 0,
        enum_descs: Vec::new(),
        class_descs: Vec::new(),
        names: Vec::new(),
        methods: HashMap::new(),
        class_implements: BTreeMap::new(),
        class_tables: crate::native::ClassTables::default(),
        static_inits: Vec::new(),
        overloads: Vec::new(),
        method_overloads: std::collections::HashMap::new(),
    };
    assert!(prog2.validate().unwrap_err().contains("field-name index 5"));

    // M-mut.6: `SetField` shares the same name-pool bound as `GetField`.
    let mut c3 = Chunk::new();
    c3.emit(Op::SetField(7), 1); // empty name pool
    c3.emit(Op::Return, 1);
    let prog3 = BytecodeProgram {
        functions: vec![Function {
            name: "main".into(),
            arity: 0,
            n_captures: 0,
            dyn_params: Vec::new(),

            unchecked: false,
            chunk: c3,
        }],
        main: 0,
        main_is_static: false,
        main_params: 0,
        enum_descs: Vec::new(),
        class_descs: Vec::new(),
        names: Vec::new(),
        methods: HashMap::new(),
        class_implements: BTreeMap::new(),
        class_tables: crate::native::ClassTables::default(),
        static_inits: Vec::new(),
        overloads: Vec::new(),
        method_overloads: std::collections::HashMap::new(),
    };
    assert!(prog3.validate().unwrap_err().contains("field-name index 7"));

    // M-mut.7: `GetStatic`/`SetStatic` are bounded by the static-init table length.
    let mut c4 = Chunk::new();
    c4.emit(Op::GetStatic(2), 1); // empty static table
    c4.emit(Op::Return, 1);
    let prog4 = BytecodeProgram {
        functions: vec![Function {
            name: "main".into(),
            arity: 0,
            n_captures: 0,
            dyn_params: Vec::new(),

            unchecked: false,
            chunk: c4,
        }],
        main: 0,
        main_is_static: false,
        main_params: 0,
        enum_descs: Vec::new(),
        class_descs: Vec::new(),
        names: Vec::new(),
        methods: HashMap::new(),
        class_implements: BTreeMap::new(),
        class_tables: crate::native::ClassTables::default(),
        static_inits: Vec::new(),
        overloads: Vec::new(),
        method_overloads: std::collections::HashMap::new(),
    };
    assert!(prog4.validate().unwrap_err().contains("static index 2"));
}

#[test]
fn validate_rejects_out_of_range_native() {
    let mut c = Chunk::new();
    c.emit(Op::CallNative(9999, 1), 1); // far past the registry length
    c.emit(Op::Return, 1);
    let prog = BytecodeProgram {
        functions: vec![Function {
            name: "main".into(),
            arity: 0,
            n_captures: 0,
            dyn_params: Vec::new(),

            unchecked: false,
            chunk: c,
        }],
        main: 0,
        main_is_static: false,
        main_params: 0,
        enum_descs: Vec::new(),
        class_descs: Vec::new(),
        names: Vec::new(),
        methods: HashMap::new(),
        class_implements: BTreeMap::new(),
        class_tables: crate::native::ClassTables::default(),
        static_inits: Vec::new(),
        overloads: Vec::new(),
        method_overloads: std::collections::HashMap::new(),
    };
    assert!(prog.validate().unwrap_err().contains("native index 9999"));
}

#[test]
fn validate_rejects_out_of_range_closure() {
    // `MakeClosure` carries a function-table index; the only function is `main` (index 0),
    // so index 4 is out of range. Guards the EV-7 bound that the exhaustive `validate` match
    // keeps (the closure arm), distinct from `Op::Call`'s.
    let mut c = Chunk::new();
    c.emit(Op::MakeClosure(4), 1);
    c.emit(Op::Return, 1);
    let prog = BytecodeProgram {
        functions: vec![Function {
            name: "main".into(),
            arity: 0,
            n_captures: 0,
            dyn_params: Vec::new(),

            unchecked: false,
            chunk: c,
        }],
        main: 0,
        main_is_static: false,
        main_params: 0,
        enum_descs: Vec::new(),
        class_descs: Vec::new(),
        names: Vec::new(),
        methods: HashMap::new(),
        class_implements: BTreeMap::new(),
        class_tables: crate::native::ClassTables::default(),
        static_inits: Vec::new(),
        overloads: Vec::new(),
        method_overloads: std::collections::HashMap::new(),
    };
    let err = prog.validate().unwrap_err();
    assert!(err.contains("closure target 4"), "{err}");
}

#[test]
fn validate_accepts_unchecked_no_index_ops() {
    // The no-index arm returns `None` (no rejection) for ops that carry a count or local slot
    // rather than a pool index — e.g. a large `CallValue` arg count and a high `GetLocal`
    // slot. This pins the "behaviour unchanged" half of making the match exhaustive: these
    // are validated elsewhere (frame sizing / runtime), never by `validate`.
    let mut c = Chunk::new();
    c.emit(Op::GetLocal(9999), 1);
    c.emit(Op::CallValue(250), 1);
    c.emit(Op::Return, 1);
    let prog = BytecodeProgram {
        functions: vec![Function {
            name: "main".into(),
            arity: 0,
            n_captures: 0,
            dyn_params: Vec::new(),

            unchecked: false,
            chunk: c,
        }],
        main: 0,
        main_is_static: false,
        main_params: 0,
        enum_descs: Vec::new(),
        class_descs: Vec::new(),
        names: Vec::new(),
        methods: HashMap::new(),
        class_implements: BTreeMap::new(),
        class_tables: crate::native::ClassTables::default(),
        static_inits: Vec::new(),
        overloads: Vec::new(),
        method_overloads: std::collections::HashMap::new(),
    };
    assert!(prog.validate().is_ok());
}

#[test]
fn bytecode_program_holds_functions_and_main_index() {
    let mut c = Chunk::new();
    c.emit(Op::Return, 1);
    let prog = BytecodeProgram {
        functions: vec![Function {
            name: "main".into(),
            arity: 0,
            n_captures: 0,
            dyn_params: Vec::new(),

            unchecked: false,
            chunk: c,
        }],
        main: 0,
        main_is_static: false,
        main_params: 0,
        enum_descs: Vec::new(),
        class_descs: Vec::new(),
        names: Vec::new(),
        methods: HashMap::new(),
        class_implements: BTreeMap::new(),
        class_tables: crate::native::ClassTables::default(),
        static_inits: Vec::new(),
        overloads: Vec::new(),
        method_overloads: std::collections::HashMap::new(),
    };
    assert_eq!(prog.functions[prog.main].name, "main");
    assert_eq!(prog.functions[0].arity, 0);
}
