//! The `Core.Http` **serve entry-point** fragment — DEC-331 D5 (`Http.serve(cfg, handler)`), built
//! in slice S3.3a.
//!
//! Split into its own file per Invariant 13, for the same reason `serve_config_prelude` was:
//! `http_prelude.rs` sits at 297 lines against the 300 soft cap, so a new fragment starts life
//! beside it rather than pushing it over. It is injected as a fourth `srcs` entry of the `Core.Http`
//! virtual module, so `import Core.Http;` reaches `Http.serve` exactly as D5's §1 surface writes it.
//!
//! WHY THE WRAPPING LIVES HERE AND NOT IN RUST. `Http.serve` takes a TYPED handler
//! (`(Request) => Response`), but the serve runtime's contract is raw `bytes -> bytes`
//! (`serve::Handler`). Something has to bridge the two, and that bridge is HTTP POLICY — which
//! request shapes are malformed, and what a malformed one produces. Keeping it in phorj means all
//! three legs would see one definition by construction, exactly as the legacy `HTTP_RESPOND_BRIDGE`
//! does; the 400-on-unparseable body below is character-for-character the bridge's, so the two paths
//! cannot answer that question differently while both exist (they overlap until S3.3c retires
//! `respond`).
//!
//! WHY A CLASS NAMED `Http`, WHICH IS ALSO THE MODULE QUALIFIER. `Http.serve` is D5's ruled
//! spelling, and a prelude fragment can only define items, so the receiver has to be a class of that
//! name. This is NOT a new pattern: `Core.Input` has shipped a `class Input` under the qualifier
//! `Input` since DEC-281, which is what `Input.readLine()` resolves through. The qualified TYPE form
//! `new Http.ServeConfig()` keeps working alongside it — verified before and after this change, and
//! pinned by `a_class_named_like_its_qualifier_does_not_shadow_the_qualified_type_form`.
//!
//! NOT A TRANSPILE SURFACE. A program that CALLS `Http.serve` is refused by `phg transpile` with
//! `E-TRANSPILE-SERVE` (Invariant 14 tier 2 — the serve loop has no faithful idiomatic PHP mapping;
//! PHP is served BY a web server rather than being one). The refusal is keyed on the call site, not
//! on this fragment: the class below is injected into every `import Core.Http;` program, so keying
//! it on the import — or on `registerServe` appearing anywhere — would refuse the five shipped
//! `examples/web/*` and break the example byte-identity glob.

/// `Http.serve(cfg, handler)` — the D5 web entry point.
///
/// **It REGISTERS and RETURNS**; it does not run an accept loop. The `Web` entry is a closure
/// FACTORY, so the runtime's transport, keep-alive, static-file interception and `(Value, String)`
/// stdout contract are all untouched — `serve::web_handlers` runs the entry once per worker to
/// obtain that worker's handler, and calls it per request. The alternative (an accept loop inside
/// the native) was designed, written down and DISPROVED — see
/// `docs/plans/2026-08-22-s3-3-http-serve.plan.md` §3c: a native cannot call `.serialize()` on the
/// `Response` it gets back, and the `ClosureInvoker` does not outlive the native call, so a native
/// cannot own a loop that invokes the handler.
///
/// `NativeHttp` is the `Core.Native.Http` alias imported by the sibling `http_request_prelude`
/// fragment. Prelude imports are program-wide once injected, so this fragment relies on it exactly
/// as `http_prelude`'s `HeaderSafety.reject` relies on it for `NativeHttp.headerFault` — repeating
/// the import here would be a second, drifting declaration of the same alias.
pub(crate) const HTTP_SERVE_PRELUDE: &str = r#"
class Http {
  static function serve(ServeConfig cfg, (Request) => Response handler): void {
    NativeHttp.registerServe(cfg, function(bytes raw): bytes {
      if (var req = Request.parse(raw)) {
        return handler(req).serialize();
      }
      return Response.text(400, "Bad Request").serialize();
    });
  }
}
"#;
