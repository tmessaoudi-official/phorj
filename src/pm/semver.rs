//! Minimal semantic-version parsing + constraint matching for the package manager (DEC-316).
//!
//! Std-only (the compiler's external-dependency policy forbids the `semver` crate), hand-rolled like
//! `bundle::sha256`. Supports the constraint forms a composer-style `require` map needs: exact
//! (`1.2.3`), caret (`^1.2.3`), tilde (`~1.2`), and wildcard (`*` / empty = any). Comparison is by
//! `(major, minor, patch)` with any pre-release ordered BELOW the same release triple (SemVer §11);
//! build metadata (`+…`) is ignored for ordering. Range unions (`>=1, <2`) are a documented follow-up.

use std::cmp::Ordering;

/// A parsed `MAJOR.MINOR.PATCH[-prerelease]` version. Build metadata is stripped on parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    /// Dot-separated pre-release identifiers (empty = a normal release).
    pub pre: Vec<String>,
}

impl Version {
    /// Parse `1.2.3`, `1.2.3-rc.1`, or `1.2.3+build` (build dropped). Missing minor/patch default to 0
    /// so `1` and `1.2` parse (a convenience the constraint forms rely on).
    pub fn parse(s: &str) -> Result<Version, String> {
        let s = s.trim();
        let s = s.strip_prefix('v').unwrap_or(s);
        // Split off build metadata (ignored) then pre-release.
        let s = s.split('+').next().unwrap_or(s);
        let (core, pre) = match s.split_once('-') {
            Some((c, p)) => (c, p.split('.').map(|x| x.to_string()).collect()),
            None => (s, Vec::new()),
        };
        let mut it = core.split('.');
        let major = parse_num(it.next(), s)?;
        let minor = it
            .next()
            .map(|x| parse_num(Some(x), s))
            .transpose()?
            .unwrap_or(0);
        let patch = it
            .next()
            .map(|x| parse_num(Some(x), s))
            .transpose()?
            .unwrap_or(0);
        if it.next().is_some() {
            return Err(format!(
                "version `{s}` has too many components (want MAJOR.MINOR.PATCH)"
            ));
        }
        Ok(Version {
            major,
            minor,
            patch,
            pre,
        })
    }

    fn triple(&self) -> (u64, u64, u64) {
        (self.major, self.minor, self.patch)
    }
}

fn parse_num(x: Option<&str>, whole: &str) -> Result<u64, String> {
    let x = x.unwrap_or("").trim();
    if x.is_empty() {
        return Err(format!("version `{whole}` is missing a numeric component"));
    }
    x.parse::<u64>()
        .map_err(|_| format!("version `{whole}` has a non-numeric component `{x}`"))
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        if !self.pre.is_empty() {
            write!(f, "-{}", self.pre.join("."))?;
        }
        Ok(())
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.triple().cmp(&other.triple()) {
            Ordering::Equal => cmp_pre(&self.pre, &other.pre),
            o => o,
        }
    }
}

/// SemVer §11: a release (no pre) ranks ABOVE a pre-release of the same triple; otherwise compare
/// identifiers lexically (numeric identifiers compared numerically, and numeric < alphanumeric).
fn cmp_pre(a: &[String], b: &[String]) -> Ordering {
    match (a.is_empty(), b.is_empty()) {
        (true, true) => Ordering::Equal,
        (true, false) => Ordering::Greater, // release > pre-release
        (false, true) => Ordering::Less,
        (false, false) => {
            for (x, y) in a.iter().zip(b.iter()) {
                let o = match (x.parse::<u64>(), y.parse::<u64>()) {
                    (Ok(nx), Ok(ny)) => nx.cmp(&ny),
                    (Ok(_), Err(_)) => Ordering::Less, // numeric < alphanumeric
                    (Err(_), Ok(_)) => Ordering::Greater,
                    (Err(_), Err(_)) => x.cmp(y),
                };
                if o != Ordering::Equal {
                    return o;
                }
            }
            a.len().cmp(&b.len())
        }
    }
}

/// A version constraint (the value side of a composer-style `require` entry).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionReq {
    /// `*` or empty — matches any release.
    Any,
    /// `1.2.3` — exactly this version.
    Exact(Version),
    /// `^1.2.3` — `>= v`, `< (leftmost-non-zero+1)`. The composer/cargo default.
    Caret(Version),
    /// `~1.2` / `~1.2.3` — `>= v`, patch-flexible within the stated minor (or minor-flexible for `~1`).
    Tilde(Version, TildeWidth),
}

/// How many components a `~` constraint pinned, which sets its upper bound.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TildeWidth {
    /// `~1` → `>=1.0.0, <2.0.0`.
    Major,
    /// `~1.2` or `~1.2.3` → `>=…, <1.3.0`.
    Minor,
}

impl VersionReq {
    pub fn parse(s: &str) -> Result<VersionReq, String> {
        let s = s.trim();
        if s.is_empty() || s == "*" {
            return Ok(VersionReq::Any);
        }
        if let Some(rest) = s.strip_prefix('^') {
            return Ok(VersionReq::Caret(Version::parse(rest)?));
        }
        if let Some(rest) = s.strip_prefix('~') {
            let width = if rest.split('.').count() <= 1 {
                TildeWidth::Major
            } else {
                TildeWidth::Minor
            };
            return Ok(VersionReq::Tilde(Version::parse(rest)?, width));
        }
        // A bare version with no operator is treated as exact (composer semantics for a pinned dep).
        Ok(VersionReq::Exact(Version::parse(s)?))
    }

    /// Does `v` satisfy this constraint? A pre-release only matches a constraint whose lower bound is
    /// itself a pre-release of the same triple (cargo/npm rule — pre-releases are opt-in).
    pub fn matches(&self, v: &Version) -> bool {
        match self {
            VersionReq::Any => v.pre.is_empty(),
            VersionReq::Exact(w) => v == w,
            VersionReq::Caret(w) => in_range(v, w, caret_upper(w)),
            VersionReq::Tilde(w, width) => in_range(v, w, tilde_upper(w, *width)),
        }
    }
}

/// `[low, high)` with the pre-release opt-in rule.
fn in_range(v: &Version, low: &Version, high: (u64, u64, u64)) -> bool {
    if !v.pre.is_empty() && v.triple() != low.triple() {
        return false; // pre-releases only match at the exact lower-bound triple
    }
    v >= low && v.triple() < high
}

fn caret_upper(w: &Version) -> (u64, u64, u64) {
    if w.major > 0 {
        (w.major + 1, 0, 0)
    } else if w.minor > 0 {
        (0, w.minor + 1, 0)
    } else {
        (0, 0, w.patch + 1)
    }
}

fn tilde_upper(w: &Version, width: TildeWidth) -> (u64, u64, u64) {
    match width {
        TildeWidth::Major => (w.major + 1, 0, 0),
        TildeWidth::Minor => (w.major, w.minor + 1, 0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap()
    }

    #[test]
    fn parses_and_defaults_components() {
        assert_eq!(v("1.2.3").triple(), (1, 2, 3));
        assert_eq!(v("1.2").triple(), (1, 2, 0));
        assert_eq!(v("1").triple(), (1, 0, 0));
        assert_eq!(v("v2.0.0").triple(), (2, 0, 0));
        assert_eq!(v("1.2.3+build").triple(), (1, 2, 3));
        assert_eq!(v("1.2.3-rc.1").pre, vec!["rc".to_string(), "1".to_string()]);
        assert!(Version::parse("1.2.3.4").is_err());
        assert!(Version::parse("1.x").is_err());
    }

    #[test]
    fn orders_prerelease_below_release() {
        assert!(v("1.0.0-rc.1") < v("1.0.0"));
        assert!(v("1.0.0-alpha") < v("1.0.0-beta"));
        assert!(v("1.0.0-1") < v("1.0.0-alpha")); // numeric < alphanumeric
        assert!(v("1.0.1") > v("1.0.0"));
    }

    #[test]
    fn caret_bounds() {
        let r = VersionReq::parse("^1.2.3").unwrap();
        assert!(r.matches(&v("1.2.3")));
        assert!(r.matches(&v("1.9.0")));
        assert!(!r.matches(&v("2.0.0")));
        assert!(!r.matches(&v("1.2.2")));
        // zero-major caret narrows to the minor.
        let r0 = VersionReq::parse("^0.2.3").unwrap();
        assert!(r0.matches(&v("0.2.9")));
        assert!(!r0.matches(&v("0.3.0")));
    }

    #[test]
    fn tilde_bounds() {
        let r = VersionReq::parse("~1.2").unwrap();
        assert!(r.matches(&v("1.2.0")));
        assert!(r.matches(&v("1.2.9")));
        assert!(!r.matches(&v("1.3.0")));
        let rm = VersionReq::parse("~1").unwrap();
        assert!(rm.matches(&v("1.9.9")));
        assert!(!rm.matches(&v("2.0.0")));
    }

    #[test]
    fn any_and_exact() {
        assert!(VersionReq::parse("*").unwrap().matches(&v("9.9.9")));
        assert!(!VersionReq::parse("*").unwrap().matches(&v("1.0.0-rc.1"))); // pre opt-in
        let e = VersionReq::parse("1.2.3").unwrap();
        assert!(e.matches(&v("1.2.3")));
        assert!(!e.matches(&v("1.2.4")));
    }

    #[test]
    fn prerelease_optin_only_at_lower_bound() {
        let r = VersionReq::parse("^1.2.3-rc.1").unwrap();
        assert!(r.matches(&v("1.2.3-rc.2")));
        assert!(r.matches(&v("1.5.0")));
        assert!(!r.matches(&v("1.3.0-rc.1"))); // a different triple's pre-release is excluded
    }
}
