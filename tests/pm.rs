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
    let mut cmd = Command::new("git");
    // Scrub every inherited `GIT_*` first, as `src/pm/fetch.rs::harden` does. Under a git hook the
    // test inherits the CALLER's repository: a pathspec `git commit` hands its hooks an ABSOLUTE
    // `GIT_INDEX_FILE`, so `git add -A` here wrote this scratch repo's `greet.phg` into the phorj
    // commit's index and the commit died with `invalid object … for 'greet.phg'` (2026-09-27).
    for (k, _) in std::env::vars_os() {
        if k.to_string_lossy().starts_with("GIT_") {
            cmd.env_remove(&k);
        }
    }
    let out = cmd
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
    std::fs::write(app.join("main.phg"), "package Main;\n\nimport Core.Output;\nimport Core.Runtime.Entry; import Core.Runtime.EntryKind;\nimport Acme.Greet;\n\n#[Entry(kind: EntryKind.Cli)]\nfunction main(): void {\n  Output.printLine(Greet.hello());\n}\n").unwrap();

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

const GREET_V1: &str = "package Acme.Greet;\n\nfunction hello(): string {\n  return \"hi\";\n}\n";
const GREET_V2: &str =
    "package Acme.Greet;\n\nfunction hello(): string {\n  return \"hello\";\n}\n";

/// Commit `body` as `greet.phg` in `repo` and point `tag` at the new commit (`-f` moves an existing tag).
fn publish_change(repo: &Path, body: &str, tag: &str) {
    std::fs::write(repo.join("greet.phg"), body).unwrap();
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", "change"]);
    git(repo, &["tag", "-f", tag]);
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap()
}

/// Audit A4: `install` used to re-resolve every time and then verify against the lock it had just
/// written, so a moved tag silently replaced the vendored tree. Now it reproduces the lock, and only
/// `update` accepts the new tree.
#[test]
fn install_refuses_a_moved_tag_until_update() {
    if !git_available() {
        eprintln!("SKIP tests/pm.rs: git not on PATH");
        return;
    }
    let ws = workspace("moved_tag");
    let repo = publish_git_package(&ws, &[("greet.phg", GREET_V1)]);
    let app = ws.join("app");
    std::fs::create_dir_all(&app).unwrap();
    let source = SourceSpec::Git {
        url: repo.to_string_lossy().to_string(),
        git_ref: "v1.0.0".into(),
    };
    ops::add(&app, "Acme/Greet", source).expect("install git dep");
    let lock_before = read(&app.join(LOCK_FILE));

    publish_change(&repo, GREET_V2, "v1.0.0");
    let err = ops::install(&app).expect_err("a moved tag must not install silently");
    assert!(
        err.contains("no longer matches phorj.lock") && err.contains("phg update"),
        "{err}"
    );
    assert_eq!(
        read(&app.join(LOCK_FILE)),
        lock_before,
        "the lock is untouched"
    );
    assert_eq!(read(&app.join("vendor/Acme/Greet/greet.phg")), GREET_V1);

    // Through the CLI too: `phg install` refuses, `phg update` is the verb that accepts.
    let phg = |verb: &str| {
        Command::new(env!("CARGO_BIN_EXE_phg"))
            .arg(verb)
            .current_dir(&app)
            .output()
            .expect("run phg")
    };
    let refused = phg("install");
    assert!(
        !refused.status.success()
            && String::from_utf8_lossy(&refused.stderr).contains("phg update"),
        "phg install: {refused:?}"
    );
    let updated = phg("update");
    assert!(updated.status.success(), "phg update: {updated:?}");
    assert_ne!(read(&app.join(LOCK_FILE)), lock_before);
    assert_eq!(read(&app.join("vendor/Acme/Greet/greet.phg")), GREET_V2);
    ops::install(&app).expect("the updated lock installs");
    let _ = std::fs::remove_dir_all(&ws);
}

fn write_index(path: &Path, repo: &Path, versions: &[&str]) {
    let list: Vec<String> = versions
        .iter()
        .map(|v| format!("{{\"version\":\"{v}\",\"tag\":\"v{v}\"}}"))
        .collect();
    std::fs::write(
        path,
        format!(
            "{{\"packages\":{{\"Acme/Greet\":{{\"git\":{:?},\"versions\":[{}]}}}}}}",
            repo.to_string_lossy(),
            list.join(",")
        ),
    )
    .unwrap();
}

fn locked_version(app: &Path) -> String {
    let lock = phorj::pm::LockFile::parse(&read(&app.join(LOCK_FILE))).unwrap();
    lock.get("Acme/Greet").unwrap().version.clone()
}

/// The cargo rule for registry deps: `install` keeps the locked version while it is still published
/// and still satisfies the constraint; `update` takes the newest; a changed constraint re-resolves.
#[test]
fn install_keeps_the_locked_registry_version_and_update_moves_it() {
    if !git_available() {
        eprintln!("SKIP tests/pm.rs: git not on PATH");
        return;
    }
    let ws = workspace("registry_pin");
    let repo = publish_git_package(&ws, &[("greet.phg", GREET_V1)]);
    publish_change(&repo, GREET_V2, "v1.1.0");
    let index = ws.join("index.json");
    // The only test in this binary that resolves a registry dependency, so the variable is not shared.
    std::env::set_var("PHORJ_REGISTRY", &index);
    let app = ws.join("app");
    std::fs::create_dir_all(&app).unwrap();
    let caret = |r: &str| SourceSpec::Registry(phorj::pm::VersionReq::parse(r).unwrap());

    write_index(&index, &repo, &["1.0.0"]);
    ops::add(&app, "Acme/Greet", caret("^1.0")).expect("install 1.0.0");
    assert_eq!(locked_version(&app), "1.0.0");

    write_index(&index, &repo, &["1.0.0", "1.1.0"]);
    ops::install(&app).expect("install with a newer release published");
    assert_eq!(
        locked_version(&app),
        "1.0.0",
        "install keeps the locked version"
    );
    assert_eq!(read(&app.join("vendor/Acme/Greet/greet.phg")), GREET_V1);

    write_index(&index, &repo, &["1.1.0"]);
    let err = ops::install(&app).expect_err("a locked version gone from the index");
    assert!(
        err.contains("no longer lists that version") && err.contains("phg update"),
        "{err}"
    );

    ops::update(&app).expect("update re-resolves");
    assert_eq!(locked_version(&app), "1.1.0");
    assert_eq!(read(&app.join("vendor/Acme/Greet/greet.phg")), GREET_V2);

    // A constraint the locked version no longer satisfies re-resolves that dependency on install.
    write_index(&index, &repo, &["1.0.0", "1.1.0"]);
    ops::add(&app, "Acme/Greet", caret("~1.0.0")).expect("narrowed constraint");
    assert_eq!(locked_version(&app), "1.0.0");
    std::env::remove_var("PHORJ_REGISTRY");
    let _ = std::fs::remove_dir_all(&ws);
}
