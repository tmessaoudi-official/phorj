# `project/lift-enum-functions/` — a lifted enum with methods outside `package Main`

This tree is the output of `phg lift <dir> -o <out>` on a four-file PHP project (scout row 5i,
DEC-526), passed through `phg format`. The formatter only removes the blank lines between imports:
unlike single-file `phg lift`, the directory lift does not format its drafts. PHP enums carry methods; phorj enums do not, so DEC-509 lowers each method
to a free function. A public enum and public free functions cannot share a file outside `package Main`
(`E-FILE-MIXED-PUBLIC`), so the directory lift writes the functions to a sibling
`<Enum>Functions.phg` in the same package.

```
lift-enum-functions/
└── src/
    ├── main.phg                  # from src/index.php — the entry, re-packaged as Main
    └── Rent/
        ├── Core/                 # package Rent.Core
        │   ├── Tenure.phg        #   the enum, alone: no functions, no imports
        │   ├── TenureFunctions.phg  # isExcluded / fallback / describe, lowered from its methods
        │   └── Policy.phg        #   a same-package caller: t.isExcluded() (UFCS), fallback()
        └── Support/Label.phg     # package Rent.Support
```

Each file imports exactly what its own items use. `Core.Output` (for `fallback`'s `echo`) and
`Rent.Support.Label` (for `describe`'s parameter) moved to `TenureFunctions.phg` with the methods that
need them. Left in `Tenure.phg`, either one would be `E-UNUSED-IMPORT`, a hard error.

The PHP it was lifted from:

```php
<?php
namespace Rent\Core;

use Rent\Support\Label;

enum Tenure: string {
    case LLI = 'LLI';
    case PLAI = 'PLAI';

    public function isExcluded(): bool {
        return match ($this) {
            self::PLAI => true,
            default => false,
        };
    }

    public static function fallback(): self {
        echo "no tenure given, falling back\n";
        return self::PLAI;
    }

    public static function describe(Label $l): string {
        return $l->text();
    }
}
```

`Policy.php` calls `Tenure::fallback()`, `$t->isExcluded()` and `Tenure::describe(new Label())`, and
`index.php` prints `$p->verdict()`. The output is identical from the original PHP, the interpreter,
the VM and the transpiled PHP:

```
no tenure given, falling back
excluded tenure
```

## Not covered here

A caller in **another** package does not check yet. `$t->isExcluded()` lifts to `t.isExcluded()`,
which needs UFCS through a function import (deferred to L7, DEC-527). `Tenure::fallback()` lifts to a
bare `fallback()` with no import. Both are reported by `phg check` (`has no method`,
`unknown function`), not silently wrong. See `KNOWN_ISSUES.md` § LIFT-ENUM-CROSS-PACKAGE.
