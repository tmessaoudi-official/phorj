//! Hermetic end-to-end tests for the package manager (DEC-316). No real network: a dependency is
//! published into a LOCAL git repo, then `pm::ops::install` fetches it via `git` into `vendor/`, and the
//! DEC-282 loader resolves the vendored import. Also covers the git-source path, lock reproducibility,
//! and integrity tamper detection — the parts the in-crate unit tests can't exercise without git.

use phorj::pm::manifest::SourceSpec;
use phorj::pm::{ops, LOCK_FILE, MANIFEST_FILE};
use std::path::{Path, PathBuf};
use std::process::Command;

fn workspace(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("phorj_pm_it_{}_{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(args)
        // Deterministic identity + no dependence on the host's global git config.
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@t")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@t")
        .current_dir(dir)
        .output()
        .expect("run git");
    assert!(
        out.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Publish a package into a fresh local git repo at tag `v1.0.0`; returns the repo dir (usable as a
/// git URL).
fn publish_git_package(ws: &Path, files: &[(&str, &str)]) -> PathBuf {
    let repo = ws.join("greet-repo");
    std::fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-q"]);
    for (rel, body) in files {
        let path = repo.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, body).unwrap();
    }
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "publish"]);
    git(&repo, &["tag", "v1.0.0"]);
    repo
}

fn git_available() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[test]
fn install_git_dependency_then_loader_resolves_it() {
    if !git_available() {
        eprintln!("SKIP tests/pm.rs: git not on PATH");
        return;
    }
    let ws = workspace("git");
    // A dependency package `Acme/Greet` (functions only — no public type, so file=type is satisfied).
    let repo = publish_git_package(
        &ws,
        &[(
            "greet.phg",
            "package Acme.Greet;\n\nfunction hello(): string {\n  return \"hi\";\n}\n",
        )],
    );

    // An app that requires it by git.
    let app = ws.join("app");
    std::fs::create_dir_all(&app).unwrap();
    std::fs::write(app.join("main.phg"), "package Main;\n\nimport Core.Output;\nimport Core.Runtime.Entry;\nimport Acme.Greet;\n\n#[Entry]\nfunction main(): void {\n  Output.printLine(Greet.hello());\n}\n").unwrap();

    let source = SourceSpec::Git {
        url: repo.to_string_lossy().to_string(),
        git_ref: "v1.0.0".into(),
    };
    let report = ops::add(&app, "Acme/Greet", source).expect("install git dep");
    assert_eq!(report.installed.len(), 1);

    // vendored tree + lock present, with a resolved commit + a 64-hex integrity hash.
    assert!(
        app.join("vendor/Acme/Greet/greet.phg").exists(),
        ".git-stripped source vendored"
    );
    assert!(
        !app.join("vendor/Acme/Greet/.git").exists(),
        ".git stripped from vendored tree"
    );
    let lock_txt = std::fs::read_to_string(app.join(LOCK_FILE)).unwrap();
    assert!(lock_txt.contains("Acme/Greet"));
    assert!(lock_txt.contains("\"commit\""), "git dep records a commit");
    let manifest_txt = std::fs::read_to_string(app.join(MANIFEST_FILE)).unwrap();
    assert!(manifest_txt.contains("Acme/Greet") && manifest_txt.contains("v1.0.0"));

    // The DEC-282 loader resolves the vendored import (proves the vendored tree is loadable).
    let unit =
        phorj::loader::load(&app.join("main.phg")).expect("app loads with the vendored package");
    // Running it prints the dependency's output — a true end-to-end proof.
    let out = phorj::cli::treewalk_program(&unit).expect("run app");
    assert_eq!(out, "hi\n");

    // Offline verify passes; a re-install is idempotent (same lock).
    ops::verify_locked(&app).expect("fresh vendor verifies");
    let lock_before = std::fs::read_to_string(app.join(LOCK_FILE)).unwrap();
    ops::install(&app).expect("re-install");
    assert_eq!(
        std::fs::read_to_string(app.join(LOCK_FILE)).unwrap(),
        lock_before,
        "install is reproducible"
    );

    // Tampering a vendored file trips the integrity gate.
    std::fs::write(
        app.join("vendor/Acme/Greet/greet.phg"),
        "package Acme.Greet;\n// tampered\n",
    )
    .unwrap();
    assert!(ops::verify_locked(&app)
        .unwrap_err()
        .contains("integrity check failed"));

    let _ = std::fs::remove_dir_all(&ws);
}
