//! Row 4j0 — a declaration's `throws` names a TYPE, and the loader resolves every type a declaration
//! names except, until this row, that one. A library package's class is mangled to its FQN, so a
//! `throws BadInputError` left as written named a type that no longer existed: `E-UNKNOWN-TYPE` for a
//! function or method throwing its own package's error, reported against perfectly ordinary code.
//! Lambdas were already covered (DEC-222); functions, methods and constructors were not.

use super::*;

const ERROR: &str = "package Acme.Errors;\n\nimport Core.ErrorModule.RuntimeError;\n\n\
    public class BadInputError extends RuntimeError {\n}\n";

const PARSER: &str = "package Acme.Errors;\n\npublic class Parser {\n    \
    public static function fold(string s): string throws BadInputError {\n        \
    if (s == \"\") {\n            throw new BadInputError(\"empty\");\n        }\n        \
    return s;\n    }\n}\n";

fn checks(u: &Unit) -> Result<String, String> {
    crate::cli::check_program(&u.program, &u.diag_src)
}

#[test]
fn a_method_throwing_its_own_packages_error_type_checks() {
    let tmp = TempDir::new();
    tmp.write("src/Acme/Errors/BadInputError.phg", ERROR);
    let file = tmp.write("src/Acme/Errors/Parser.phg", PARSER);
    let u = load(&file).expect("loads");
    checks(&u).expect("the package-mate error type in `throws` resolves");
}

/// Across packages the type arrives by import — and an import used ONLY by a `throws` clause is a
/// use, not `E-UNUSED-IMPORT`.
#[test]
fn a_throws_type_imported_from_another_package_resolves_and_counts_as_used() {
    let tmp = TempDir::new();
    tmp.write("src/Acme/Errors/BadInputError.phg", ERROR);
    tmp.write("src/Acme/Errors/Parser.phg", PARSER);
    tmp.write(
        "src/Acme/Text/Classifier.phg",
        "package Acme.Text;\n\nimport Acme.Errors.Parser;\nimport Acme.Errors.BadInputError;\n\n\
         public class Classifier {\n    \
         public function classify(string s): string throws BadInputError {\n        \
         return Parser::fold(s)?;\n    }\n}\n",
    );
    let main = tmp.write(
        "src/main.phg",
        "package Main;\n\nimport Core.Runtime.Entry;\nimport Core.Runtime.EntryKind;\nimport Core.Output;\n\
         import Acme.Text.Classifier;\nimport Acme.Errors.BadInputError;\n\n\
         #[Entry(kind: EntryKind.Cli)]\nfunction main(): void {\n    \
         try {\n        Output.print(new Classifier().classify(\"\"));\n    } \
         catch (BadInputError e) {\n        Output.print(\"caught\");\n    }\n}\n",
    );
    let u = load(&main).expect("loads");
    checks(&u).expect("a cross-package `throws` type resolves");
}
