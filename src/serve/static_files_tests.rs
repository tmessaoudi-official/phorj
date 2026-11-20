//! Guard tests for the docroot static layer (`static_files.rs`). The site is built the way `phg serve <DIR>`
//! sees it (`resolve_site_dir`), inside a directory whose own path has a dot component, so every check is
//! made relative to the canonical docroot, never to the absolute path.

use super::{resolve_site_dir, try_static};
use std::path::{Path, PathBuf};

fn site() -> (PathBuf, PathBuf) {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let base = std::env::temp_dir()
        .join(format!(".phorj-static-{}-{nanos}", std::process::id()))
        .join("site");
    let public = base.join("public");
    std::fs::create_dir_all(public.join(".git")).unwrap();
    std::fs::write(public.join("index.phg"), "SECRET-SOURCE").unwrap();
    std::fs::write(public.join("Other.PHG"), "SECRET-UPPER").unwrap();
    std::fs::write(public.join(".env"), "SECRET-ENV").unwrap();
    std::fs::write(public.join(".git").join("config"), "SECRET-GIT").unwrap();
    std::fs::write(public.join("a.txt"), "hello").unwrap();
    (base, public)
}

/// A file OUTSIDE the docroot in a directory with no dot component anywhere in its path, so only the
/// containment guard can refuse a symlink to it (a dot in the path would trip the dot guard first).
fn outside_file() -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("phorj-static-out-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("outside");
    std::fs::write(&file, "SECRET-OUTSIDE").unwrap();
    assert!(file
        .components()
        .all(|c| !c.as_os_str().to_string_lossy().starts_with('.')));
    file
}

fn get(root: &Path, path: &str) -> Option<(u16, String)> {
    let raw = format!("GET {path} HTTP/1.1\r\nHost: x\r\n\r\n");
    let resp = try_static(root, raw.as_bytes())?;
    let text = String::from_utf8_lossy(&resp).into_owned();
    let status = text
        .split(' ')
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    Some((status, text))
}

#[cfg(unix)]
#[test]
fn a_symlink_never_serves_source_or_a_dotfile() {
    use std::os::unix::fs::symlink;
    let (base, public) = site();
    symlink("index.phg", public.join("leak.txt")).unwrap();
    symlink("Other.PHG", public.join("upper.txt")).unwrap();
    symlink(".env", public.join("env.txt")).unwrap();
    symlink(".git", public.join("g")).unwrap();
    symlink("a.txt", public.join("alias.txt")).unwrap();
    let outside = outside_file();
    symlink(&outside, public.join("out.txt")).unwrap();
    let (root, _entry) = resolve_site_dir(&base).unwrap();
    // A symlink resolving to source or to a dot path: the same 404 as a direct `.phg` request.
    for p in [
        "/leak.txt",
        "/upper.txt",
        "/env.txt",
        "/g/config",
        "/out.txt",
    ] {
        let (status, text) = get(&root, p).unwrap_or_else(|| panic!("{p}: fell through"));
        assert_eq!(status, 404, "{p}: {text}");
        assert!(!text.contains("SECRET"), "{p} leaked: {text}");
    }
    // An ordinary file, directly or through a symlink that stays inside the docroot, is served.
    for p in ["/a.txt", "/alias.txt"] {
        let (status, text) = get(&root, p).unwrap_or_else(|| panic!("{p}: fell through"));
        assert_eq!(status, 200, "{p}: {text}");
        assert!(text.ends_with("hello"), "{p}: {text}");
    }
    let _ = std::fs::remove_dir_all(base.parent().unwrap());
    let _ = std::fs::remove_dir_all(outside.parent().unwrap());
}

#[test]
fn the_request_path_guards_still_hold() {
    let (base, _public) = site();
    let (root, _entry) = resolve_site_dir(&base).unwrap();
    let (status, text) = get(&root, "/index.phg").unwrap();
    assert_eq!(status, 404, "{text}");
    assert!(!text.contains("SECRET"));
    assert_eq!(get(&root, "/Other.PHG").map(|r| r.0), Some(404));
    // A `.phg` path that does not exist is still a 404, never the program: the request-path guard alone
    // decides it (the canonical-path guard needs a file to resolve).
    assert_eq!(get(&root, "/nope.phg").map(|r| r.0), Some(404));
    // A dot path in the request falls through to the program, which decides.
    assert!(get(&root, "/.env").is_none());
    assert!(get(&root, "/.git/config").is_none());
    let _ = std::fs::remove_dir_all(base.parent().unwrap());
}
