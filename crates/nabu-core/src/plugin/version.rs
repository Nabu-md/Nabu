//! Version management for the plugin architecture.
//!
//! Provides semantic versioning ([`Version`]), version requirements
//! ([`VersionReq`]), and compatibility negotiation for future plugins.
//! This is metadata-only — no plugin loading occurs.

use std::fmt;
use std::str::FromStr;

/// Error returned when parsing a version string fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionParseError {
    input: String,
    message: String,
}

impl fmt::Display for VersionParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Failed to parse version '{}': {}", self.input, self.message)
    }
}

impl std::error::Error for VersionParseError {}

/// A semantic version following the MAJOR.MINOR.PATCH format.
///
/// Used by [`PluginManifest`](super::PluginManifest) and compatibility
/// validation to negotiate version requirements.
///
/// # Format
///
/// `MAJOR.MINOR.PATCH[-PRERELEASE]`
///
/// - MAJOR: incompatible API changes
/// - MINOR: backward-compatible functionality additions
/// - PATCH: backward-compatible bug fixes
/// - PRERELEASE: optional pre-release identifier (e.g., "alpha", "beta.1")
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Version {
    /// Major version — incremented on incompatible API changes.
    pub major: u64,
    /// Minor version — incremented on backward-compatible additions.
    pub minor: u64,
    /// Patch version — incremented on backward-compatible bug fixes.
    pub patch: u64,
    /// Optional pre-release identifier (e.g., "alpha", "rc.1").
    pub pre: Option<String>,
}

impl Version {
    /// Creates a new version from major, minor, and patch components.
    pub const fn new(major: u64, minor: u64, patch: u64) -> Self {
        Self {
            major,
            minor,
            patch,
            pre: None,
        }
    }

    /// Creates a new pre-release version.
    pub fn with_pre(major: u64, minor: u64, patch: u64, pre: impl Into<String>) -> Self {
        Self {
            major,
            minor,
            patch,
            pre: Some(pre.into()),
        }
    }

    /// Returns `true` if this is a pre-release version.
    pub fn is_pre_release(&self) -> bool {
        self.pre.is_some()
    }

    /// Returns `true` if this version is compatible with the given requirement.
    ///
    /// Compatibility rules:
    /// - Same major version → compatible (may have higher minor/patch)
    /// - Higher major version → incompatible
    /// - Pre-release versions are only compatible with exact matches
    pub fn is_compatible_with(&self, requirement: &VersionReq) -> bool {
        requirement.matches(self)
    }

    /// Checks if this version satisfies a minimum version requirement.
    pub fn satisfies_minimum(&self, minimum: &Version) -> bool {
        self.major > minimum.major
            || (self.major == minimum.major && self.minor > minimum.minor)
            || (self.major == minimum.major && self.minor == minimum.minor && self.patch >= minimum.patch)
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        if let Some(pre) = &self.pre {
            write!(f, "-{}", pre)?;
        }
        Ok(())
    }
}

impl FromStr for Version {
    type Err = VersionParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let err = |msg: &str| VersionParseError {
            input: s.to_string(),
            message: msg.to_string(),
        };

        let pre_part = if let Some(idx) = s.find('-') {
            let pre = s[idx + 1..].to_string();
            if pre.is_empty() {
                return Err(err("Empty pre-release identifier"));
            }
            Some(pre)
        } else {
            None
        };

        let version_part = if let Some(idx) = s.find('-') {
            &s[..idx]
        } else {
            s
        };

        let parts: Vec<&str> = version_part.split('.').collect();
        if parts.len() != 3 {
            return Err(err("Version must have exactly three dot-separated components (MAJOR.MINOR.PATCH)"));
        }

        let major = parts[0].parse::<u64>().map_err(|_| err("Invalid major version"))?;
        let minor = parts[1].parse::<u64>().map_err(|_| err("Invalid minor version"))?;
        let patch = parts[2].parse::<u64>().map_err(|_| err("Invalid patch version"))?;

        Ok(Version {
            major,
            minor,
            patch,
            pre: pre_part,
        })
    }
}

/// A version requirement that describes a range of compatible versions.
///
/// Supports:
/// - Exact version: `=1.2.3`
/// - Caret requirement: `^1.2.3` (compatible with 1.x.y where y >= 2)
/// - Tilde requirement: `~1.2.3` (compatible with 1.2.x where x >= 3)
/// - Minimum version: `>=1.0.0`
/// - Any version: `*`
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum VersionReq {
    /// Any version is acceptable.
    Any,
    /// Exactly this version.
    Exact(Version),
    /// Compatible with this version (^): allows changes in the least significant
    /// non-zero version component. `^1.2.3` allows 1.2.3 through 1.x.x.
    Compatible(Version),
    /// Approximately equivalent (~): allows only patch changes.
    /// `~1.2.3` allows 1.2.3 through 1.2.x.
    PatchCompatible(Version),
    /// Minimum version requirement (>=).
    Minimum(Version),
}

impl VersionReq {
    /// Creates a requirement matching any version.
    pub fn any() -> Self {
        VersionReq::Any
    }

    /// Creates a requirement for exactly this version.
    pub fn exact(version: Version) -> Self {
        VersionReq::Exact(version)
    }

    /// Creates a compatible requirement (^).
    pub fn compatible(version: Version) -> Self {
        VersionReq::Compatible(version)
    }

    /// Creates a patch-compatible requirement (~).
    pub fn patch_compatible(version: Version) -> Self {
        VersionReq::PatchCompatible(version)
    }

    /// Creates a minimum version requirement (>=).
    pub fn minimum(version: Version) -> Self {
        VersionReq::Minimum(version)
    }

    /// Returns `true` if the given version satisfies this requirement.
    pub fn matches(&self, version: &Version) -> bool {
        match self {
            VersionReq::Any => true,
            VersionReq::Exact(req) => version == req,
            VersionReq::Compatible(req) => {
                version.major == req.major
                    && version.minor >= req.minor
                    && version.patch >= req.patch
            }
            VersionReq::PatchCompatible(req) => {
                version.major == req.major
                    && version.minor == req.minor
                    && version.patch >= req.patch
            }
            VersionReq::Minimum(req) => version.satisfies_minimum(req),
        }
    }
}

impl FromStr for VersionReq {
    type Err = VersionParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        if s == "*" {
            return Ok(VersionReq::Any);
        }
        if let Some(rest) = s.strip_prefix('=') {
            return Ok(VersionReq::Exact(rest.parse()?));
        }
        if let Some(rest) = s.strip_prefix('^') {
            return Ok(VersionReq::Compatible(rest.parse()?));
        }
        if let Some(rest) = s.strip_prefix('~') {
            return Ok(VersionReq::PatchCompatible(rest.parse()?));
        }
        if let Some(rest) = s.strip_prefix(">=") {
            return Ok(VersionReq::Minimum(rest.parse()?));
        }
        // Default to compatible (caret) requirement
        Ok(VersionReq::Compatible(s.parse()?))
    }
}

impl fmt::Display for VersionReq {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VersionReq::Any => write!(f, "*"),
            VersionReq::Exact(v) => write!(f, "={}", v),
            VersionReq::Compatible(v) => write!(f, "^{}", v),
            VersionReq::PatchCompatible(v) => write!(f, "~{}", v),
            VersionReq::Minimum(v) => write!(f, ">={}", v),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_parsing() {
        let v: Version = "1.2.3".parse().unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
        assert!(v.pre.is_none());
    }

    #[test]
    fn version_with_pre_release() {
        let v: Version = "2.0.0-alpha".parse().unwrap();
        assert_eq!(v.major, 2);
        assert_eq!(v.pre, Some("alpha".to_string()));
        assert!(v.is_pre_release());
    }

    #[test]
    fn version_display() {
        let v = Version::new(1, 0, 0);
        assert_eq!(v.to_string(), "1.0.0");

        let v = Version::with_pre(1, 0, 0, "rc.1");
        assert_eq!(v.to_string(), "1.0.0-rc.1");
    }

    #[test]
    fn invalid_version_parsing() {
        assert!("1.2".parse::<Version>().is_err());
        assert!("abc".parse::<Version>().is_err());
        assert!("1.2.3.4".parse::<Version>().is_err());
        assert!("1.2.-1".parse::<Version>().is_err());
    }

    #[test]
    fn satisfies_minimum() {
        let v1 = Version::new(1, 0, 0);
        let v2 = Version::new(1, 5, 0);
        let v3 = Version::new(2, 0, 0);

        assert!(v2.satisfies_minimum(&v1));
        assert!(!v1.satisfies_minimum(&v2));
        assert!(v3.satisfies_minimum(&v1));
        assert!(v2.satisfies_minimum(&Version::new(1, 5, 0)));
        assert!(v2.satisfies_minimum(&Version::new(1, 4, 0)));
        assert!(!v2.satisfies_minimum(&Version::new(1, 6, 0)));
    }

    #[test]
    fn version_req_any() {
        let req = VersionReq::Any;
        assert!(req.matches(&Version::new(0, 0, 0)));
        assert!(req.matches(&Version::new(999, 999, 999)));
    }

    #[test]
    fn version_req_exact() {
        let req = VersionReq::exact(Version::new(1, 2, 3));
        assert!(req.matches(&Version::new(1, 2, 3)));
        assert!(!req.matches(&Version::new(1, 2, 4)));
        assert!(!req.matches(&Version::new(2, 0, 0)));
    }

    #[test]
    fn version_req_compatible() {
        let req = VersionReq::compatible(Version::new(1, 2, 3));
        assert!(req.matches(&Version::new(1, 2, 3)));
        assert!(req.matches(&Version::new(1, 5, 0)));
        assert!(!req.matches(&Version::new(2, 0, 0)));
    }

    #[test]
    fn version_req_patch_compatible() {
        let req = VersionReq::patch_compatible(Version::new(1, 2, 3));
        assert!(req.matches(&Version::new(1, 2, 3)));
        assert!(req.matches(&Version::new(1, 2, 10)));
        assert!(!req.matches(&Version::new(1, 3, 0)));
    }

    #[test]
    fn version_req_minimum() {
        let req = VersionReq::minimum(Version::new(1, 5, 0));
        assert!(req.matches(&Version::new(1, 5, 0)));
        assert!(req.matches(&Version::new(2, 0, 0)));
        assert!(!req.matches(&Version::new(1, 4, 0)));
    }

    #[test]
    fn version_req_parsing() {
        assert_eq!("*".parse::<VersionReq>().unwrap(), VersionReq::Any);
        assert_eq!("=1.2.3".parse::<VersionReq>().unwrap(), VersionReq::Exact(Version::new(1, 2, 3)));
        assert_eq!("^1.2.3".parse::<VersionReq>().unwrap(), VersionReq::Compatible(Version::new(1, 2, 3)));
        assert_eq!("~1.2.3".parse::<VersionReq>().unwrap(), VersionReq::PatchCompatible(Version::new(1, 2, 3)));
        assert_eq!(">=1.0.0".parse::<VersionReq>().unwrap(), VersionReq::Minimum(Version::new(1, 0, 0)));
    }

    #[test]
    fn is_compatible_with() {
        let v = Version::new(1, 5, 0);
        assert!(v.is_compatible_with(&VersionReq::compatible(Version::new(1, 2, 0))));
        assert!(!v.is_compatible_with(&VersionReq::compatible(Version::new(2, 0, 0))));
    }
}
