//! CLI handlers for the package manager verbs (DEC-316): `phg add/install/update/remove`. These are
//! the only network-capable commands (`run`/`check`/`transpile` stay offline — Invariant 10). They
//! operate on the current directory as the project root (where `phorj.json` lives) and drive
//! `crate::pm::ops`.

use crate::pm::manifest::SourceSpec;
use crate::pm::ops::{self, InstallReport};
use crate::pm::semver::VersionReq;
use std::path::PathBuf;

fn root() -> Result<PathBuf, String> {
    std::env::current_dir().map_err(|e| format!("cannot determine the current directory: {e}"))
}

/// Dispatch a package-manager verb (the only network-capable commands). `args` is everything after the
/// subcommand. `vendor` is the retired alias (DEC-282) pointing at the DEC-316 verbs.
pub fn dispatch(cmd: &str, args: &[String]) -> Result<(), String> {
    match cmd {
        "install" => cmd_install(),
        "update" => cmd_update(),
        "add" => cmd_add(args),
        "remove" => cmd_remove(args),
        "vendor" => Err(
            "phg vendor is retired (DEC-282): use `phg add <Publisher/Name>`, \
                         `phg install`, `phg update`, or `phg remove <Publisher/Name>` (DEC-316)."
                .to_string(),
        ),
        other => Err(format!("unknown package command `{other}`")),
    }
}

/// `phg install` — fetch + vendor every dependency from `phorj.json`, write `phorj.lock`.
pub fn cmd_install() -> Result<(), String> {
    report("Installed", &ops::install(&root()?)?);
    Ok(())
}

/// `phg update` — re-resolve from `phorj.json`, taking the newest satisfying versions.
pub fn cmd_update() -> Result<(), String> {
    report("Updated", &ops::install(&root()?)?);
    Ok(())
}

/// `phg remove <Publisher/Name>` — drop a dependency + its vendored tree, then re-resolve.
pub fn cmd_remove(args: &[String]) -> Result<(), String> {
    let name = args.first().ok_or("usage: phg remove <Publisher/Name>")?;
    report("Removed; remaining", &ops::remove(&root()?, name)?);
    Ok(())
}

/// `phg add <Publisher/Name>[@version] [--git <url> --ref <tag>] [--path <dir>]` — add a dependency
/// then install. With no source flags a bare `Name` uses `*`, `Name@^1.2` a registry constraint.
pub fn cmd_add(args: &[String]) -> Result<(), String> {
    let mut name: Option<String> = None;
    let mut version: Option<String> = None;
    let mut git: Option<String> = None;
    let mut git_ref: Option<String> = None;
    let mut path: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--git" => {
                git = Some(take(args, &mut i, "--git")?);
            }
            "--ref" => {
                git_ref = Some(take(args, &mut i, "--ref")?);
            }
            "--path" => {
                path = Some(take(args, &mut i, "--path")?);
            }
            flag if flag.starts_with('-') => return Err(format!("unknown flag `{flag}`")),
            positional => {
                if name.is_some() {
                    return Err(format!("unexpected argument `{positional}`"));
                }
                // `Name@version` splits the registry constraint off the name.
                match positional.split_once('@') {
                    Some((n, v)) => {
                        name = Some(n.to_string());
                        version = Some(v.to_string());
                    }
                    None => name = Some(positional.to_string()),
                }
            }
        }
        i += 1;
    }

    let name = name.ok_or(
        "usage: phg add <Publisher/Name>[@version] [--git <url> --ref <tag>] [--path <dir>]",
    )?;
    let source = build_source(&name, version, git, git_ref, path)?;
    report("Added; installed", &ops::add(&root()?, &name, source)?);
    Ok(())
}

fn build_source(
    name: &str,
    version: Option<String>,
    git: Option<String>,
    git_ref: Option<String>,
    path: Option<String>,
) -> Result<SourceSpec, String> {
    match (path, git) {
        (Some(_), Some(_)) => Err("`--path` and `--git` are mutually exclusive".to_string()),
        (Some(p), None) => Ok(SourceSpec::Path(p)),
        (None, Some(url)) => {
            let git_ref =
                git_ref.ok_or_else(|| format!("`--git` for `{name}` needs a `--ref <tag>`"))?;
            Ok(SourceSpec::Git { url, git_ref })
        }
        (None, None) => {
            let req = VersionReq::parse(version.as_deref().unwrap_or("*"))?;
            Ok(SourceSpec::Registry(req))
        }
    }
}

fn take(args: &[String], i: &mut usize, flag: &str) -> Result<String, String> {
    *i += 1;
    args.get(*i)
        .cloned()
        .ok_or_else(|| format!("`{flag}` needs a value"))
}

fn report(verb: &str, rep: &InstallReport) {
    if rep.installed.is_empty() {
        println!("{verb}: no dependencies.");
        return;
    }
    println!("{verb} {} package(s):", rep.installed.len());
    for p in &rep.installed {
        println!("  {p}");
    }
}
