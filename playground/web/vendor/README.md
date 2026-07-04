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
npm i codemirror@6.0.1 esbuild
printf 'export { EditorView, basicSetup } from "codemirror";\n' > entry.js
npx esbuild entry.js --bundle --format=esm --minify --legal-comments=none \
  --outfile=../codemirror.js
```

Pin the same `codemirror` version as before unless intentionally upgrading; after a rebuild, verify
the editor still mounts (load `playground/web/` over a static server and confirm the CodeMirror pane
appears with no console errors).

> `php-wasm` (the optional "Run PHP" oracle) is still loaded from jsdelivr at runtime — it is a
> single pre-built module (no transitive-build hazard) and only fetched when the user enables PHP.
