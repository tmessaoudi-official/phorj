//! AST — declarations: functions, attributes, enums, class members/decls, traits,
//! interfaces, items.

use super::*;

mod attributes;
mod classes;
mod enums;
mod functions;
pub use attributes::*;
pub use classes::*;
pub use enums::*;
pub use functions::*;

/// An interface declaration (`interface Speaker { method-sigs } [extends A, B]`). Methods are
/// signatures only — a `FunctionDecl` with an empty body (M-RT S2). Interfaces are nominal types
/// usable as a variable/parameter type; a class that `implements` one is a subtype of it. PHP-absent
/// at runtime: there are no interface instances, so the backends only use interfaces for the
/// `instanceof` table and (the transpiler) for emitting a PHP `interface`.
#[derive(Debug, Clone, PartialEq)]
pub struct InterfaceDecl {
    /// Declaration-level visibility (default `Public`). Loader-enforced; see [`Visibility`].
    pub vis: Visibility,
    pub name: String,
    /// Generic type parameters (`interface Iterator<T>`, DEC-257) — same compile-time-only
    /// discipline as generic classes: a bare name in this set resolves to `Ty::Param` in method
    /// signatures; a class's `implements Iterator<int>` substitutes them for conformance; **erased**
    /// before any backend. Empty for the common non-generic case.
    pub type_params: Vec<String>,
    /// Parent interfaces (`interface Animal extends Speaker, Named`) — flattened transitively.
    pub extends: Vec<String>,
    /// Method signatures (each a `FunctionDecl` with an empty body).
    pub methods: Vec<FunctionDecl>,
    /// `sealed interface` (W5-3) — a closed hierarchy: its permitted implementors are exactly those
    /// declared in the whole program, so a `match` over this interface type is exhaustive with no `_`
    /// (DEC-179). Compile-time-only — PHP emits a plain `interface` (no sealed concept).
    pub sealed: bool,
    /// True for a compiler-INJECTED interface (`Iterator` — added by the `Core.IteratorModule` prelude
    /// when imported), false for a user declaration. Injected interfaces are exempt from the
    /// DEC-202 PHP-builtin-name rejection: the transpiled output is namespaced (`namespace Main;
    /// interface Iterator` never redeclares the root `\Iterator` — verified vs PHP 8.5), and the
    /// name is compiler-owned, not user-chosen.
    pub injected: bool,
    pub span: Span,
}

/// A top-level item in a program.
#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    /// `import a.b.c;` or `import a.b.c as leaf;` — `alias`, when present, overrides the call-site
    /// qualifier (the bound leaf) so colliding leaves from different packages can coexist (M5 S2c,
    /// design O-9). `None` ⇒ the qualifier is `path`'s last segment.
    Import {
        path: Vec<String>,
        alias: Option<String>,
        /// DEC-Q-A wildcard import `import X.Y.*;` — when true, `path` is the PACKAGE PREFIX (not a
        /// member), and the loader expands this to one per-member `Item::Import` (the package's PUBLIC
        /// members only when cross-package — P-Q-A-2; shallow, sorted) BEFORE any backend (Inv 5). A
        /// wildcard never carries an `alias` (`import X.* as Y` = `E-WILDCARD-ALIAS`). Plain imports
        /// set this `false`.
        wildcard: bool,
        /// DEC-Q-A `except { A, B }` on a wildcard — names removed from the expansion set before
        /// binding. Empty for every non-wildcard import (and for a wildcard with no `except`).
        except: Vec<String>,
        span: Span,
    },
    Function(FunctionDecl),
    Enum(EnumDecl),
    Class(ClassDecl),
    Interface(InterfaceDecl),
    /// `trait T { members }` — horizontal reuse composed by a class via `use T;` (M-RT S8). Not a type.
    Trait(TraitDecl),
    /// `type Name = Type;` — a compile-time alias, erased after checking (resolved by the checker
    /// and expanded out of the AST before any backend runs).
    TypeAlias {
        name: String,
        ty: Type,
        span: Span,
    },
    /// `test "name" { stmts }` — a unit test (M-Test T1). `test` is a *contextual* keyword (special
    /// only at item position when immediately followed by a string literal), so it stays usable as an
    /// identifier elsewhere. The body is checked like a `-> void` function body with no `this`. A test
    /// item is valid only under `phg test` (test mode); in a normal build the checker rejects it as
    /// `E-TEST-OUTSIDE-TESTS`. It is never reached by a backend in a normal compile — the `phg test`
    /// runner executes test bodies directly on the interpreter (M-Test T3).
    Test {
        name: String,
        body: Vec<Stmt>,
        span: Span,
    },
}

/// A whole parsed program.
#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    /// The file's package path (`package App.Util;` ⇒ `["App", "Util"]`). empty only for a
    /// malformed file with no declaration — the checker rejects that as `E-NO-PACKAGE` (M5: every
    /// file is packaged, never inferred). The reserved `["Main"]` is the runnable entry (M5 S1).
    pub package: Vec<String>,
    pub items: Vec<Item>,
    pub span: Span,
}
