//! `phg build` FLAG parsing — extracted from `main.rs` (Invariant 13, split-as-you-go: that file is a
//! grandfathered size-gate breach, so a new command's dispatch has to pay for itself by shrinking it).
//!
//! Pure parsing: no I/O, no process exit. The caller maps [`FlagError`] onto its own exit behaviour, which
//! is what makes this testable at all — the previous inline loop called `exit(2)` from four places.

/// The parsed flags of `phg build <file> [flags]`.
pub struct BuildFlags {
    /// `-o <path>` — the artifact path.
    pub out: Option<String>,
    /// `--target <triple>` — a single cross-compile target.
    pub target: Option<String>,
    /// `--all` — every known target. Mutually exclusive with `--target`.
    pub all: bool,
    /// M-DX S0: Release by default (secure by construction — value-exposing machinery is gated off in the
    /// artifact); `--dev` opts a debug artifact in.
    pub profile: crate::profile::Profile,
}

/// Why a flag list was rejected. Distinct variants because the two produce different messages, and
/// collapsing them would lose the reason `--sign` is refused.
pub enum FlagError {
    /// Print usage, exit 2 — a dangling value, an unknown flag, or `--all` with `--target`.
    Usage,
    /// `--sign` is reserved for Phase 3.
    SigningIsPhase3,
}

/// Parse the flags that follow `phg build <file>`.
pub fn parse(args: &[String]) -> Result<BuildFlags, FlagError> {
    let mut f = BuildFlags {
        out: None,
        target: None,
        all: false,
        profile: crate::profile::Profile::Release,
    };
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-o" => {
                f.out = Some(args.get(i + 1).ok_or(FlagError::Usage)?.clone());
                i += 2;
            }
            "--target" => {
                f.target = Some(args.get(i + 1).ok_or(FlagError::Usage)?.clone());
                i += 2;
            }
            "--all" => {
                f.all = true;
                i += 1;
            }
            "--dev" => {
                f.profile = crate::profile::Profile::Dev;
                i += 1;
            }
            "--sign" => return Err(FlagError::SigningIsPhase3),
            _ => return Err(FlagError::Usage),
        }
    }
    // `--all` and `--target` are mutually exclusive: one means every target, the other means exactly one.
    if f.all && f.target.is_some() {
        return Err(FlagError::Usage);
    }
    Ok(f)
}
