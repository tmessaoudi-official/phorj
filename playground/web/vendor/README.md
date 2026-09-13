# Vendored playground dependencies

## `codemirror.js` — CodeMirror 6 editor (single esbuild bundle)

The playground editor uses CodeMirror 6. It is **vendored as one bundled ES module** rather than
imported from a CDN at runtime, because both CDN routes we tried are unreliable for this dep tree:

- **esm.sh** (the original source) builds transitive deps on demand and returned sustained `408`
  timeouts for `@codemirror/view` across every version range its importers request — the editor
  never mounted and the page hung "loading".
- **jsdelivr `/+esm`** serves pre-built modules but splits `@codemirror/state` into a separate copy,
  so `EditorView` and `basicSetup` end up with two `@codemirror/state` instances and boot crashes
  with *"Unrecognized extension value in extension set … multiple instances of @codemirror/state"*.

A single esbuild bundle inlines the whole tree from one `node_modules`, so there is exactly one
`@codemirror/state` (no crash) and no runtime network dependency (no `408`). `main.js` imports
`EditorView` and `basicSetup` from `./vendor/codemirror.js`.

### Rebuild

```sh
mkdir cmbuild && cd cmbuild
printf '{"name":"cmbuild","private":true,"type":"module"}\n' > package.json
npm i codemirror@6.0.2 esbuild
printf 'export { EditorView, basicSetup } from "codemirror";\n' > entry.js
npx esbuild entry.js --bundle --format=esm --minify --legal-comments=none \
  --outfile=../codemirror.js
```

Current bundle (rebuilt 2026-09-13, node 26.8.2, esbuild 0.28.2): `codemirror` 6.0.2 resolving
`@codemirror/state` 6.7.4, `view` 6.43.11, `language` 6.12.4, `commands` 6.11.0, `autocomplete`
6.20.3, `search` 6.7.2, `lint` 6.9.7, `@lezer/common` 1.5.2, `@lezer/highlight` 1.2.3, `@lezer/lr`
1.4.10, `style-mod` 4.1.3, `crelt` 1.0.7, `w3c-keyname` 2.2.8 — `codemirror` only pins `^` ranges, so
the transitive tree is whatever npm resolves on the day; record it here on every rebuild.

Pin the same `codemirror` version as before unless intentionally upgrading; after a rebuild, verify
the editor still mounts (load `playground/web/` over a static server and confirm the CodeMirror pane
appears with no console errors).

> `php-wasm` (the optional "Run PHP" oracle) is still loaded from jsdelivr at runtime, pinned to `php-wasm@0.1.0` (`main.js`, `PHP_WASM_URL`) — it is a
> single pre-built module (no transitive-build hazard) and only fetched when the user enables PHP.
