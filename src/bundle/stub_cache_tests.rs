use super::*;
use crate::bundle::sha256::sha256_hex;

fn scratch(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("phorj_stub_cache_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

/// A fake checkout: `Cargo.toml` with `manifest`, plus a `phg` built under `target/release/`.
fn checkout(tag: &str, manifest: &str) -> (PathBuf, PathBuf) {
    let dir = scratch(tag);
    std::fs::write(dir.join("Cargo.toml"), manifest).unwrap();
    let exe = dir.join("target").join("release").join("phg");
    std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
    std::fs::write(&exe, b"phg").unwrap();
    (dir, exe)
}

const PHORJ: &str = "[package]\nname = \"phorj\"\nversion = \"1.0.0\"\n";

#[test]
fn a_cargo_project_that_is_not_phorj_is_not_a_checkout() {
    let (dir, exe) = checkout("other", "[package]\nname = \"other\"\n");
    assert!(!is_phorj_checkout(&dir, &exe));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_phorj_manifest_is_not_enough_without_the_running_phg_under_target() {
    let (dir, _) = checkout("elsewhere", PHORJ);
    let away = scratch("elsewhere_exe").join("phg");
    std::fs::write(&away, b"phg").unwrap();
    assert!(!is_phorj_checkout(&dir, &away));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(away.parent().unwrap());
}

#[test]
fn a_phorj_checkout_running_its_own_build_is_a_checkout() {
    let (dir, exe) = checkout("own", PHORJ);
    assert!(is_phorj_checkout(&dir, &exe));
    // Reached through a symlink (a PATH entry pointing into target/) it is still the same build.
    #[cfg(unix)]
    {
        let link = scratch("own_link").join("phg");
        std::os::unix::fs::symlink(&exe, &link).unwrap();
        assert!(is_phorj_checkout(&dir, &link));
        let _ = std::fs::remove_dir_all(link.parent().unwrap());
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn only_the_package_table_name_counts() {
    for manifest in [
        "[workspace.package]\nname = \"phorj\"\n[package]\nname = \"other\"\n",
        "[package]\nname = \"other\"\n[package.metadata.x]\nname = \"phorj\"\n",
        "[package]\nname = \"other\"\n[dependencies]\nphorj = \"1\"\n",
        "[workspace]\nmembers = [\"phorj\"]\n",
    ] {
        let (dir, exe) = checkout("tables", manifest);
        assert!(!is_phorj_checkout(&dir, &exe), "accepted:\n{manifest}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// Stage `bytes` beside `stub` and publish them; returns their hash.
fn publish_bytes(stub: &Path, bytes: &[u8]) -> String {
    std::fs::create_dir_all(stub.parent().unwrap()).unwrap();
    let tmp = stub.with_file_name(".staged");
    std::fs::write(&tmp, bytes).unwrap();
    let hash = sha256_hex(bytes);
    publish(&tmp, stub, &hash).unwrap();
    hash
}

#[test]
fn a_published_stub_carries_its_sidecar_and_verifies() {
    let dir = scratch("publish");
    let stub = dir.join("x86_64-unknown-linux-musl").join("phg");
    let hash = publish_bytes(&stub, b"stub bytes");
    assert_eq!(
        std::fs::read_to_string(sidecar(&stub)).unwrap().trim(),
        hash
    );
    assert!(verified_hit(&stub, None));
    assert!(verified_hit(&stub, Some(&hash)));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_tampered_cache_hit_is_refused_until_republished() {
    let dir = scratch("tamper");
    let stub = dir.join("phg");
    publish_bytes(&stub, b"stub bytes");
    std::fs::write(&stub, b"swapped after publishing").unwrap();
    assert!(!verified_hit(&stub, None));
    // The refill publishes over the refused entry, with no stale temp files left beside it.
    publish_bytes(&stub, b"rebuilt stub bytes");
    assert!(verified_hit(&stub, None));
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, ["phg", "phg.sha256"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_hit_without_a_sidecar_is_refused() {
    let dir = scratch("nosidecar");
    let stub = dir.join("phg");
    std::fs::write(&stub, b"planted").unwrap();
    assert!(!verified_hit(&stub, None));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn off_a_checkout_a_hit_must_also_match_the_manifest() {
    let dir = scratch("pinned");
    let stub = dir.join("phg");
    publish_bytes(&stub, b"stub bytes");
    // Stub and sidecar agree with each other, but not with the baked manifest.
    assert!(!verified_hit(&stub, Some(&"0".repeat(64))));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn downloads_are_https_only_including_redirects() {
    let cmd = curl_https("curl", Path::new("/dev/null"), "https://example.test/x");
    let args: Vec<String> = cmd
        .get_args()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    let pair = |flag: &str| args.windows(2).any(|w| w[0] == flag && w[1] == "=https");
    assert!(pair("--proto"), "initial protocol not https-only: {args:?}");
    assert!(pair("--proto-redir"), "redirects not https-only: {args:?}");
}
