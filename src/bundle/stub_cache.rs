//! Cross-stub trust (audit 2026-10-07, A4): which branch may FILL the per-user stub cache, and what a
//! cache HIT must prove before `phg build --target` embeds it. Split out of `cross.rs`, which drives it.
//!
//! - A cache miss builds from source only inside a phorj checkout, recognised POSITIVELY: the running
//!   `phg` lives under `<dir>/target/` and `<dir>/Cargo.toml`'s `[package]` is named `phorj`. Any other
//!   directory with a `Cargo.toml` used to qualify, so `phg build --target` in an unrelated Rust project
//!   ran that project's cargo build and cached its output as the stub.
//! - Every published stub carries a `<stub>.sha256` sidecar, and a hit is re-hashed against it; off a
//!   checkout the hit must ALSO equal the baked manifest hash. The sidecar catches corruption; the
//!   manifest is what makes tampering with the cache detectable.
//! - Downloads go through [`curl_https`]: https only, redirects included.

use std::path::{Path, PathBuf};

/// Is `dir` a phorj source checkout that `exe` was built in?
pub(crate) fn is_phorj_checkout(dir: &Path, exe: &Path) -> bool {
    // Canonical on both sides: a symlinked PATH entry or a `..` in either path must not flip the answer.
    let (Ok(dir), Ok(exe)) = (dir.canonicalize(), exe.canonicalize()) else {
        return false;
    };
    let Ok(target) = dir.join("target").canonicalize() else {
        return false;
    };
    // `target` itself must not be a link out of `dir`: one pointing into the developer's real
    // checkout would make any directory holding a phorj `Cargo.toml` pass (panel 2026-10-08).
    target.parent() == Some(dir.as_path())
        && exe.starts_with(&target)
        && std::fs::read_to_string(dir.join("Cargo.toml"))
            .is_ok_and(|manifest| package_name(&manifest) == Some("phorj"))
}

/// The `name` key of the `[package]` table — not `[workspace.package]`, `[package.metadata.*]` or
/// any other table that can carry a `name`.
fn package_name(manifest: &str) -> Option<&str> {
    let mut in_package = false;
    for line in manifest.lines().map(str::trim) {
        if line.starts_with('[') {
            in_package = line == "[package]";
        } else if in_package {
            if let Some(value) = line.strip_prefix("name").map(str::trim_start) {
                if let Some(value) = value.strip_prefix('=') {
                    return Some(value.trim().trim_matches('"'));
                }
            }
        }
    }
    None
}

/// `<stub>.sha256` — the sidecar holding the hash the stub had when it was published.
pub(crate) fn sidecar(stub: &Path) -> PathBuf {
    let mut name = stub.file_name().unwrap_or_default().to_os_string();
    name.push(".sha256");
    stub.with_file_name(name)
}

/// Publish the staged stub `tmp` at `stub`: rename its sidecar into place, then the stub, each
/// atomically. `hash` is the sha256 of `tmp`'s bytes. Another `phg build` reading between the two
/// renames sees a mismatch and refills; it never sees a half-written file, and nothing is deleted.
pub(crate) fn publish(tmp: &Path, stub: &Path, hash: &str) -> Result<(), String> {
    let side = sidecar(stub);
    let side_tmp = side.with_extension(format!("sha256.{}", std::process::id()));
    let recorded = std::fs::write(&side_tmp, format!("{hash}\n"))
        .and_then(|()| std::fs::rename(&side_tmp, &side));
    if let Err(e) = recorded {
        let _ = std::fs::remove_file(&side_tmp);
        let _ = std::fs::remove_file(tmp);
        return Err(format!("cannot record the stub's hash in the cache: {e}"));
    }
    std::fs::rename(tmp, stub).map_err(|e| {
        let _ = std::fs::remove_file(tmp);
        format!("cannot publish stub into cache: {e}")
    })
}

/// May the cached `stub` be embedded? Its bytes must hash to its sidecar, and to `pinned` when given
/// (off a checkout: the baked manifest hash). A refused hit is left in place for the refill to
/// overwrite: deleting it could remove the stub a concurrent `phg build` has just published.
pub(crate) fn verified_hit(stub: &Path, pinned: Option<&str>) -> bool {
    match (std::fs::read(stub), std::fs::read_to_string(sidecar(stub))) {
        (Ok(bytes), Ok(recorded)) => {
            let got = crate::bundle::sha256::sha256_hex(&bytes);
            got == recorded.trim() && pinned.is_none_or(|want| got == want)
        }
        _ => false,
    }
}

/// The `curl` invocation every stub and registry download uses.
pub(crate) fn curl_https(curl: &str, dest: &Path, url: &str) -> std::process::Command {
    let mut cmd = std::process::Command::new(curl);
    cmd.args(["-fSL", "--proto", "=https", "--proto-redir", "=https", "-o"])
        .arg(dest)
        .arg(url);
    cmd
}

#[cfg(test)]
#[path = "stub_cache_tests.rs"]
mod tests;
