//! `phg explain` sub-catalog: the DEC-318 `#[Config]` typed-configuration codes (split from
//! `explain.rs` per Invariant 13 — consulted from its fallback arm, same contract).

/// The explanation text for a `E-CONFIG-*` code, or `None` when `code` is not ours.
pub(super) fn text(code: &str) -> Option<&'static str> {
    Some(match code {
        "E-CONFIG-SIG" => {
            "E-CONFIG-SIG — a `#[Config]` provider with the wrong shape.\n\n\
             A typed-config provider (DEC-318) is a top-level function with NO parameters and a\n\
             concrete return type: `#[Config] function appConfig() -> AppConfig { ... }`. The\n\
             runtime injects its result into `#[Entry] function main(config: AppConfig)`.\n"
        }
        "E-CONFIG-DUP" => {
            "E-CONFIG-DUP — two `#[Config]` providers return the same type.\n\n\
             The entry injection resolves a provider BY its return type, so a program declares at\n\
             most one provider per config type (DEC-318). Remove one, or split the config into\n\
             distinct types.\n"
        }
        "E-CONFIG-MISSING" => {
            "E-CONFIG-MISSING — the entry asks for a config type nobody provides.\n\n\
             `#[Entry] function main(config: T)` needs a matching `#[Config] function ... -> T`\n\
             in the project (DEC-318). Declare one:\n\n\
                 import Core.Runtime.Config;\n\
                 #[Config] function appConfig() -> T { return new T(...); }\n"
        }
        "E-VARIANT-AMBIGUOUS" => {
            "E-VARIANT-AMBIGUOUS — a bare variant name is declared by more than one enum.\n\n\
             `new Dup(…)` cannot know which enum you mean (before 2026-07-22 the pick was silently\n\
             arbitrary — a soundness hole, now closed). Qualify the construction:\n\n\
                 new TheEnum.Dup(…)\n"
        }
        "E-TRANSPILE-VARIANT-COLLISION" => {
            "E-TRANSPILE-VARIANT-COLLISION — two top-level PHP class names collide.\n\n\
             Enum variants transpile to enum-SCOPED PHP classes (`Shape.Circle` ⇒\n\
             `final class Shape_Circle extends Shape`, DEC-329.3), so two enums sharing a variant\n\
             name coexist fine. This error is the PATHOLOGICAL composed-name case — a scoped name\n\
             equal to another top-level class in the same namespace (`class Shape_Circle` beside\n\
             `enum Shape { Circle }`, or `enum A_B { C }` beside `enum A { B_C }`). The transpiler\n\
             refuses rather than emitting a fatally-broken program (\u{00a7}14 LADDER); the program\n\
             still runs with `phg run`. Rename one of the colliding declarations.\n"
        }
        "E-CONFIG-TARGET" => {
            "E-CONFIG-TARGET — `#[Config]` on a method.\n\n\
             A config provider runs before any instance exists, so it must be a TOP-LEVEL function\n\
             (DEC-318). Move the provider out of the class.\n"
        }
        _ => return None,
    })
}
