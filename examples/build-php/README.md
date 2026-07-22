# `phg build --php` — transpile INTO a live PHP app (DEC-320 v1)

The compile-time adoption lever, straight from the TS→JS playbook: your `.phg` files emit `.php`
siblings inside an existing PHP application, so a team migrates one file per pull-request while
the app keeps running on PHP. Compile-time only — there is no runtime bridge, no rebuild hook,
no interop layer; the generated PHP is ordinary PHP.

## What one build emits

```
src/Billing/Invoice.phg  ──►  src/Billing/Invoice.php     (sibling; every class/enum/interface/trait)
                              src/_phorj/runtime.php      (ONE shared file per project)
```

* **Siblings** hold the types their `.phg` declares — at the exact path a PSR-4 autoloader
  expects (the folder=package law and the public-surface file rule already enforce the layout).
  Each carries a first-line `@generated` marker; add `*.php linguist-generated` to
  `.gitattributes` if you commit them.
* **`_phorj/runtime.php`** holds everything the siblings share: the `__phorj_*` helpers the
  project actually uses, the injected preludes (`Result`, `Option`, …), **every free function**
  (PHP autoloads classes, never functions), the runtime-static initializer (runs at include
  time), and a generated **classmap autoloader** covering every sibling class — including an
  enum's several classes per file, which plain PSR-4 cannot address. Because of that classmap,
  the ONE composer edit below is the only wiring the host app ever needs.

## Walkthrough (using `examples/project/shapes`)

```sh
$ phg build examples/project/shapes/src/main.phg --php
wrote   …/src/Acme/Geometry/Paint.php
wrote   …/src/Acme/Geometry/Rect.php
wrote   …/src/Acme/Geometry/Shape.php
wrote   …/src/_phorj/runtime.php
4 file(s) written, 0 already current

Register the shared runtime ONCE in the host composer.json (phg never edits it):

    "autoload": { "files": ["…/src/_phorj/runtime.php"] }
```

Apply that diff by hand, `composer dump-autoload`, and the host app can construct
`new \Acme\Geometry\Rect(3, 4)` or call `\Main\describe($r)` like any other PHP code. Re-running
the build is a no-op for unchanged files (`N already current` — content compare, CI-friendly).

## The v1 contract

* **No `#[Entry]` bootstrap is emitted.** A split build embeds phorj code in a host app that owns
  the request lifecycle; a phorj `main` stays a plain callable (`\Main\main()`).
* **Typing not-yet-migrated PHP**: the shipped `declare` interop is the v1 surface
  (`declare class Cart { function total(): float; }`) — foreign refs emit `\Cart` and are never
  re-declared. `phg stubs` (the `.d.ts` analog) and `phg watch` are the recorded v2 slices.
* **Native-only modules refuse** exactly like `phg transpile` (THE LADDER RULE): a project
  importing `Core.DatabaseModule`/`Mail`/`HttpClient`/`Session` is refused loudly, never
  silently degraded.
