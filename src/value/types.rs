//! The `Value` repertoire: FNV hashing, class layouts, `Value`/closures/instances/enums,
//! hashable keys.

use super::*;

/// A hand-rolled **FNV-1a** hasher for instance field maps (M-perf). Field keys are short identifiers
/// (`"v"`, `"x"`, `"price"`), where std's DoS-resistant SipHash is overkill: FNV-1a is a handful of
/// XOR/multiply per byte with no keying overhead, measurably faster for short keys on the object hot
/// path. Field-map keys come only from a program's own source (never attacker-controlled network
/// input), so SipHash's collision-DoS resistance buys nothing here. Std-only, safe, deterministic.
pub struct FnvHasher(u64);

/// Seeded with the 64-bit FNV offset basis, so a fresh hasher (one per key via `BuildHasherDefault`)
/// starts correct and `write` is a pure XOR/multiply loop (no in-band re-seed).
impl Default for FnvHasher {
    fn default() -> Self {
        FnvHasher(0xcbf2_9ce4_8422_2325)
    }
}

impl Hasher for FnvHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        // FNV-1a per byte: XOR, then multiply by the FNV prime (wrapping). Same constants as
        // `bundle::cross::fnv1a_64`.
        let mut h = self.0;
        for &b in bytes {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        self.0 = h;
    }
}

/// A class's instance-field **slot layout** (M-perf slot-indexed fields, S1b): the bidirectional
/// name↔slot map shared by every instance of one class. Built once per class from
/// [`crate::ast::class_field_layout`] (so the two backends agree) and held behind `Rc` on each
/// [`Instance`], so construction *and* access both resolve `name → slot` against the instance's own
/// **runtime** layout — making slot order irrelevant to correctness (the multiple-inheritance
/// base-offset hazard never arises; slots are always runtime-resolved, never statically baked).
#[derive(Debug, Default)]
pub struct ClassLayout {
    /// field name → slot index, keyed by [`FnvHasher`]. The S2 inline cache fast-paths past this
    /// lookup on a monomorphic site; this map is the slow path / first miss.
    slots: HashMap<String, usize, BuildHasherDefault<FnvHasher>>,
    /// slot index → field name, in the deterministic sorted order from `class_field_layout`. Drives
    /// eq/reflect iteration so two instances of one class compare slot-aligned.
    names: Vec<String>,
}

impl ClassLayout {
    /// Build a layout from an **ordered, deduplicated** field-name list (as `class_field_layout`
    /// produces — sorted). The slot of a name is its index in `names`.
    pub fn new(names: Vec<String>) -> Rc<Self> {
        let slots = names
            .iter()
            .cloned()
            .enumerate()
            .map(|(i, n)| (n, i))
            .collect();
        Rc::new(ClassLayout { slots, names })
    }

    /// Build a layout for a **native carrier** (`Regex`, …) from the field names it populates. The
    /// names are sorted and deduped for self-consistency: two carriers of the same kind get an
    /// identical layout, matching what `class_field_layout` produces for the same field set — so
    /// eq/reflect parity holds.
    pub fn from_sorted_names(names: &[&str]) -> Rc<Self> {
        let mut v: Vec<String> = names.iter().map(|s| (*s).to_string()).collect();
        v.sort();
        v.dedup();
        Self::new(v)
    }

    /// The slot index of `name`, or `None` when the name is not a declared storage field of the class.
    #[inline]
    pub fn slot(&self, name: &str) -> Option<usize> {
        self.slots.get(name).copied()
    }

    /// The field names in slot order (slot `i` is `names()[i]`).
    pub fn names(&self) -> &[String] {
        &self.names
    }

    /// The number of slots (declared storage fields).
    pub fn len(&self) -> usize {
        self.names.len()
    }

    /// Whether the class has no storage fields.
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }
}

#[derive(Debug, Clone)]
pub enum Value {
    Int(i64),
    Float(f64),
    /// An exact fixed-point **`decimal`** (M-NUM S1): value = `unscaled × 10^(-scale)`. `19.99d` is
    /// `{ unscaled: 1999, scale: 2 }`. A distinct primitive from `float` (no implicit coercion — the
    /// whole point is keeping float out of money math). Arithmetic (`+ - *`) is single-sourced in the
    /// `decimal_*` kernels below; any i128 overflow is a clean [`FAULT_DECIMAL_OVERFLOW`] fault,
    /// byte-identical across both backends and the emitted BCMath PHP (the helper bounds-checks the
    /// result against i128 range and faults the same way). Rendering is [`fmt_decimal`].
    Decimal {
        unscaled: i128,
        scale: u8,
    },
    Bool(bool),
    Str(PhStr),
    /// Raw octet sequence (`bytes`). Shared (like `List`) — cloning is a refcount bump. Distinct from
    /// `Str` (which is UTF-8); converts only via the `core.bytes` natives (M6 W0).
    Bytes(Rc<Vec<u8>>),
    Unit,
    /// `null` — the sole inhabitant of an absent optional (`T?`). A non-optional `T` never holds it
    /// (the checker's non-null discipline); PHP-native, erases to PHP `null` (M3 S2).
    Null,
    /// Shared (M2 P5a): cloning a list value is a refcount bump, not a deep element copy.
    List(Rc<Vec<Value>>),
    /// An **insertion-ordered** key→value map (M-RT S3). The order is part of the value: PHP arrays
    /// preserve insertion order, so a `Vec` of pairs (not a `HashMap`) is what keeps a future
    /// `keys()`/iteration byte-identical with the PHP target (risk R1). Shared via `Rc` like `List`
    /// (cloning is a refcount bump). Built and indexed only through the `build_map`/`map_index`
    /// kernels below, so both backends agree on dedup and lookup semantics.
    Map(Rc<Vec<(HKey, Value)>>),
    /// An **insertion-ordered** set of hashable keys (M-RT S7b). Like `Map`, the order is part of the
    /// value (not a `HashSet`): PHP arrays preserve insertion order, so a `Vec` of keys keeps a future
    /// `Set` iteration / `array_values` byte-identical with the PHP target (risk R1). Shared via `Rc`
    /// like `List`/`Map` (cloning is a refcount bump). Built only through the `build_set` kernel below,
    /// so both backends dedup identically.
    Set(Rc<Vec<HKey>>),
    Instance(Rc<Instance>),
    Enum(Rc<EnumVal>),
    /// A first-class function value: either a tree-walking closure (interpreter),
    /// a bare named-function reference, or a VM bytecode closure (Task 4).
    Closure(Rc<ClosureData>),
    /// A typed FIFO channel (M6 W4 green threads) — a **shared-mutable handle** like [`Instance`]:
    /// cloning a `Value::Channel` shares the *same* buffer, so a `send` through one binding is visible
    /// to a `recv` through another (the whole point of a channel). Carries its scheduler [`ChanId`]
    /// (allocated at `Channel.create()`) so a blocking `recv` can register on the right channel's
    /// wait-list; the `Rc<RefCell<VecDeque>>` is the shared buffer of erased `Value`s. The static
    /// element type is the compile-time-only `Channel<T>` annotation. **Opaque** to the
    /// arithmetic/compare/display kernels (the checker forbids using a channel as an operand), so the
    /// single-sourced `value.rs` kernels are untouched. Never transpiled — green threads have no PHP
    /// target (`E-CONCURRENCY-NO-PHP`); a `spawn`/channel program is quarantined from the PHP oracle.
    Channel(ChanId, Rc<RefCell<VecDeque<Value>>>),
    /// A green-task handle (M6 W4): just its scheduler [`TaskId`]. The task's result lives in the
    /// shared `Coop.results` map (keyed by this id), populated when the task completes — eagerly at
    /// `spawn` in the synchronous-degenerate path, or when the task's coroutine finishes in the
    /// cooperative path. `join` reads it by id. Opaque to the kernels; never transpiled.
    Task(TaskId),
    /// An opaque native database resource handle (DEC-208 `Core.DatabaseModule`) — a connection or a
    /// lazily-executed prepared statement. Shared-mutable like [`Value::Channel`]/[`Value::Instance`]:
    /// cloning shares the same `Rc`, so a statement's accumulated binds are visible through every
    /// binding. **Opaque** to the arithmetic / compare / display kernels (the checker forbids using a
    /// handle as an operand or interpolating it), so the single-sourced `value.rs` kernels are
    /// untouched. The concrete rusqlite-backed impl is feature-gated (`db`) in `src/ext/db/natives.rs`;
    /// with `db` off this variant is unconstructable. Quarantined from the PHP oracle (impure natives);
    /// the transpiler emits faithful PDO (DEC-208, LADDER case 1).
    Db(Rc<dyn DbObject>),
}

/// The data of a first-class function value (M3 S3, Task 3).
///
/// - `Tree`: an expression-body lambda captured from the tree-walking interpreter.
/// - `Named`: a bare named-function reference (the name is resolved at call time).
/// - `Byte`: a bytecode closure constructed by the VM in Task 4; constructing it in the
///   interpreter is a bug — any such path panics with `unreachable!`.
#[derive(Debug, Clone)]
pub enum ClosureData {
    Tree {
        params: Vec<crate::ast::Param>,
        ret: Option<crate::ast::Type>,
        body: crate::ast::LambdaBody,
        env: Vec<(String, Value)>,
        /// The captured receiver when the lambda references `this` (Phase 1 closures slice), else
        /// `None`. It is the same `Rc` instance handle the enclosing method holds, so a field
        /// mutation through it is visible to the closure ("live" capture). Set at closure creation;
        /// restored as `self.this` while the body runs.
        this_capture: Option<Value>,
    },
    Named(String),
    /// Bytecode closure — constructed by the VM (Task 4). The interpreter never constructs
    /// this variant; encountering it at runtime is a bug (`unreachable!`).
    Byte {
        func: usize,
        captures: Vec<Value>,
    },
}

/// A class instance — a **shared-mutable handle** (M-mut.6). The `class` is immutable (set once at
/// construction); only `fields` mutates, so it alone is interior-mutable (`RefCell`). Held in
/// `Rc<Instance>`, so cloning a `Value::Instance` shares the *same* cell: a field write through one
/// binding (`o.f = e`) is visible through every other binding — PHP/Java object semantics (F2).
/// Field reads clone the value out and drop the borrow immediately; writes take a `borrow_mut` only
/// after the value is fully evaluated, so a borrow is never held across a re-entrant `eval`/`run`.
#[derive(Debug, Clone)]
pub struct Instance {
    /// The class name, shared as `Rc<str>` so construction is a refcount bump, not a fresh `String`
    /// allocation per instance (the VM clones it from the compile-time `ClassDesc.class`). Content-
    /// equal to the old `String` on every surface — eq/hash/Display are byte-identical.
    pub class: Rc<str>,
    /// The shared `name → slot` layout for `class` (M-perf S1b). Every instance of one class shares the
    /// same `Rc`, so field access resolves a slot against the receiver's own runtime layout.
    pub layout: Rc<ClassLayout>,
    /// Slot-indexed field storage, `len() == layout.len()`. The `None` sentinel = an unset field
    /// (preserves the old "read of an unpopulated field faults" semantics — an absent HashMap key
    /// before S1b). Interior-mutable for shared-handle field writes (`o.f = e`), same as before.
    pub fields: RefCell<Vec<Option<Value>>>,
}

/// The class name of the injected `Core.Secret` opaque wrapper (`docs/specs` Fork B). Single-sourced
/// here so every value-render surface (`Debug.dump`, the fault-frame `inspect` dump, the debugger REPL,
/// the transpiled PHP twin) recognizes a secret identically — DEC-263. The DRY divergence that let
/// `Debug.dump` leak a secret's plaintext (`src/ext/debug/natives.rs` had a *separate* renderer that missed
/// the redaction `src/inspect.rs` already had) is closed by routing all of them through [`Instance::is_secret`].
pub const SECRET_CLASS: &str = "Secret";

/// The universal redaction sentinel a `Secret<T>` renders as on EVERY surface (DEC-263). Never the
/// wrapped value — `.expose()` is the sole read path. Byte-identical to the string the transpiled PHP
/// twin emits, so a dumped secret agrees across `run`/`runvm`/PHP.
pub const SECRET_REDACTED: &str = "Secret(<redacted>)";

impl Instance {
    /// True when this instance is the injected `Core.Secret` wrapper — the single redaction predicate
    /// shared by every render surface (DEC-263). Over-redaction is security-safe: a user's own class
    /// named `Secret` would also redact (never leak), whereas a missed real secret is the vulnerability.
    /// A future `#[Redacted]` attribute could generalize this to any opt-in type (recorded REOPENABLE
    /// in DEC-263); today the security primitive is the sole redacted type.
    pub fn is_secret(&self) -> bool {
        self.class.as_ref() == SECRET_CLASS
    }

    /// Allocate an instance of `class` with every slot unset (`None`).
    pub fn new(class: Rc<str>, layout: Rc<ClassLayout>) -> Self {
        let n = layout.len();
        Instance {
            class,
            layout,
            fields: RefCell::new(vec![None; n]),
        }
    }

    /// Read field `name`, cloning the value out (and dropping the borrow before returning, preserving
    /// handle semantics). `None` if the name is not in the layout *or* the slot is unset — the caller
    /// turns that into the same runtime fault as a pre-S1b absent key.
    pub fn get_field(&self, name: &str) -> Option<Value> {
        self.layout
            .slot(name)
            .and_then(|i| self.fields.borrow()[i].clone())
    }

    /// Write field `name`. Returns `false` when `name` is not a declared storage slot — checker-
    /// unreachable for a valid program (the layout is a superset of declared fields), so callers may
    /// ignore the result; surfacing it keeps the write total/panic-free (EV-7).
    pub fn set_field(&self, name: &str, v: Value) -> bool {
        match self.layout.slot(name) {
            Some(i) => {
                self.fields.borrow_mut()[i] = Some(v);
                true
            }
            None => false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct EnumVal {
    /// Enum type + variant names, shared as `Rc<str>` (built once in `EnumDesc`, cloned as a refcount
    /// bump per construction instead of two fresh `String` allocations). Content-equal to the old
    /// `String` — eq/hash/Display byte-identical.
    pub ty: Rc<str>,
    pub variant: Rc<str>,
    pub payload: Vec<Value>,
}

/// Hashable key subset for `Map`/`Set` (`Value` can't derive `Hash`/`Eq`: it
/// holds `f64`). Unused by the M1 sample but required by the value-type signatures.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum HKey {
    Int(i64),
    Bool(bool),
    Str(PhStr),
}

impl HKey {
    /// Project a runtime `Value` onto the hashable key subset, or `None` if it isn't a valid map key
    /// (`float`, list, instance, …). The checker forbids non-`{int,bool,string}` key *types*
    /// (`E-MAP-KEY`) and types the index of `m[k]` against the map's key type, so a `None` here is
    /// checker-unreachable — the callers turn it into a clean fault rather than a panic (EV-7).
    pub fn from_value(v: &Value) -> Option<HKey> {
        match v {
            Value::Int(n) => Some(HKey::Int(*n)),
            Value::Bool(b) => Some(HKey::Bool(*b)),
            Value::Str(s) => Some(HKey::Str(s.clone())),
            _ => None,
        }
    }

    /// Inverse of [`HKey::from_value`] — used when a key flows back out as a `Value` (a future
    /// `keys()` native). Total: every `HKey` variant maps to exactly one `Value`.
    pub fn to_value(&self) -> Value {
        match self {
            HKey::Int(n) => Value::Int(*n),
            HKey::Bool(b) => Value::Bool(*b),
            HKey::Str(s) => Value::Str(s.clone()),
        }
    }
}
