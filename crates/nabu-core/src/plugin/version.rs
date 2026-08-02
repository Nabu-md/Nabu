//! Version negotiation for plugins.
//!
//! Supports semantic versioning, minimum supported versions, API compatibility
//! checks, and future migration planning.

use std::cmp::Ordering;
use std::fmt;

/// A semantic version (major.minor.patch).
///
/// Versions are comparable and support compatibility checks according to
/// semantic versioning rules:
/// - Major version 0: unstable API, breaking changes allowed
/// - Same major version with same minor: compatible
/// - Same major version: compatible with MINOR-version checks
/// - Different major version: incompatible (unless explicitly declared)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    /// Optional pre-release tag (e.g., "alpha", "beta.1").
    pub pre: Option<String>,
    /// Optional build metadata (e.g., "20260729").
    pub build: Option<String>,
}

impl Version {
    /// Create a new version.
    pub const fn new(major: u64, minor: u64, patch: u64) -> Self {
        Self {
            major,
            minor,
            patch,
            pre: None,
            build: None,
        }
    }

    /// Create a version with pre-release tag.
    pub fn with_pre(major: u64, minor: u64, patch: u64, pre: &str) -> Self {
        Self {
            major,
            minor,
            patch,
            pre: Some(pre.to_string()),
            build: None,
        }
    }

    /// Parse a version string ("1.2.3" or "1.2.3-alpha.1+build123").
    pub fn parse(s: &str) -> Result<Self, VersionError> {
        let s = s.trim();

        // Split off build metadata
        let (base, build) = match s.find('+') {
            Some(pos) => (&s[..pos], Some(s[pos + 1..].to_string())),
            None => (s, None),
        };

        // Split off pre-release tag
        let (version_str, pre) = match base.find('-') {
            Some(pos) => (&base[..pos], Some(base[pos + 1..].to_string())),
            None => (base, None),
        };

        let parts: Vec<&str> = version_str.split('.').collect();
        if parts.len() != 3 {
            return Err(VersionError::ParseError(s.to_string()));
        }

        let major = parts[0]
            .parse()
            .map_err(|_| VersionError::ParseError(s.to_string()))?;
        let minor = parts[1]
            .parse()
            .map_err(|_| VersionError::ParseError(s.to_string()))?;
        let patch = parts[2]
            .parse()
            .map_err(|_| VersionError::ParseError(s.to_string()))?;

        Ok(Self {
            major,
            minor,
            patch,
            pre,
            build,
        })
    }

    /// Check if this version is compatible with a requirement.
    ///
    /// Compatibility rules:
    /// - Major 0.x: only exact minor+patch match (unstable API)
    /// - Same major + same minor: compatible (same API surface)
    /// - Same major + higher minor: compatible (added functionality)
    /// - Different major: incompatible
    pub fn is_compatible_with(&self, requirement: &VersionRequirement) -> bool {
        match requirement {
            VersionRequirement::Exact(v) => self == v,
            VersionRequirement::Compatible(v) => {
                if self.major == 0 || v.major == 0 {
                    // Unstable: require exact match
                    self.major == v.major && self.minor == v.minor && self.patch == v.patch
                } else {
                    // Stable: same major version required
                    self.major == v.major && self.minor >= v.minor
                }
            }
            VersionRequirement::Range(min, max) => {
                let min_ok = self.major > min.major
                    || (self.major == min.major && self.minor > min.minor)
                    || (self.major == min.major
                        && self.minor == min.minor
                        && self.patch >= min.patch);
                let max_ok = self.major < max.major
                    || (self.major == max.major && self.minor < max.minor)
                    || (self.major == max.major
                        && self.minor == max.minor
                        && self.patch <= max.patch);
                min_ok && max_ok
            }
            VersionRequirement::GreaterThan(min) => self >= min,
        }
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.major.cmp(&other.major) {
            Ordering::Equal => match self.minor.cmp(&other.minor) {
                Ordering::Equal => match self.patch.cmp(&other.patch) {
                    Ordering::Equal => {
                        // Pre-release versions are less than release versions
                        match (&self.pre, &other.pre) {
                            (None, Some(_)) => Ordering::Greater,
                            (Some(_), None) => Ordering::Less,
                            (None, None) => Ordering::Equal,
                            (Some(a), Some(b)) => a.cmp(b),
                        }
                    }
                    other => other,
                },
                other => other,
            },
            other => other,
        }
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        if let Some(pre) = &self.pre {
            write!(f, "-{}", pre)?;
        }
        if let Some(build) = &self.build {
            write!(f, "+{}", build)?;
        }
        Ok(())
    }
}

/// A requirement for a version — used by plugins to declare API compatibility.
#[derive(Debug, Clone, PartialEq)]
pub enum VersionRequirement {
    /// Must match exactly.
    Exact(Version),
    /// Must be compatible (same major, minor >= required).
    Compatible(Version),
    /// Must be within range [min, max].
    Range(Version, Version),
    /// Must be greater than or equal to minimum.
    GreaterThan(Version),
}

impl VersionRequirement {
    /// Check if a version satisfies this requirement.
    pub fn is_satisfied_by(&self, version: &Version) -> bool {
        version.is_compatible_with(self)
    }

    /// Create a ">= major.minor.patch" requirement.
    pub fn at_least(major: u64, minor: u64, patch: u64) -> Self {
        Self::GreaterThan(Version::new(major, minor, patch))
    }
}

impl fmt::Display for VersionRequirement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Exact(v) => write!(f, "=={}", v),
            Self::Compatible(v) => write!(f, "~{}", v),
            Self::Range(min, max) => write!(f, ">={}, <={}", min, max),
            Self::GreaterThan(v) => write!(f, ">={}", v),
        }
    }
}

/// Errors that can occur during version operations.
///
/// The `TooOld`/`TooNew` variants box their [`Version`]s so the error type
/// stays small (returned from `Result`s on hot paths).
#[derive(Debug, Clone, PartialEq)]
pub enum VersionError {
    /// The version string could not be parsed.
    ParseError(String),
    /// An API version is too old.
    TooOld {
        version: Box<Version>,
        minimum: Box<Version>,
    },
    /// An API version is too new (not tested).
    TooNew {
        version: Box<Version>,
        maximum: Box<Version>,
    },
}

impl fmt::Display for VersionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ParseError(s) => write!(f, "Cannot parse version string: '{}'", s),
            Self::TooOld { version, minimum } => {
                write!(
                    f,
                    "Version {} is too old. Minimum supported: {}",
                    version, minimum
                )
            }
            Self::TooNew { version, maximum } => {
                write!(
                    f,
                    "Version {} was not tested. Maximum tested: {}",
                    version, maximum
                )
            }
        }
    }
}

impl std::error::Error for VersionError {}

/// Result of a version compatibility check.
#[derive(Debug, Clone, PartialEq)]
pub enum CompatibilityResult {
    /// Version is fully compatible.
    Compatible,
    /// Version is incompatible for the given reason.
    Incompatible(VersionError),
    /// Version is compatible but may have minor issues.
    Warning(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_parsing() {
        let v = Version::parse("1.2.3").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
    }

    #[test]
    fn version_with_pre_release() {
        let v = Version::parse("1.0.0-alpha.1").unwrap();
        assert_eq!(v.pre, Some("alpha.1".to_string()));
    }

    #[test]
    fn version_with_build() {
        let v = Version::parse("1.0.0+build20260729").unwrap();
        assert_eq!(v.build, Some("build20260729".to_string()));
    }

    #[test]
    fn version_ordering() {
        let v1 = Version::new(1, 0, 0);
        let v2 = Version::new(2, 0, 0);
        assert!(v1 < v2);
    }

    #[test]
    fn compatible_same_major() {
        let v1 = Version::new(1, 5, 0);
        let req = VersionRequirement::Compatible(Version::new(1, 3, 0));
        assert!(v1.is_compatible_with(&req));
    }

    #[test]
    fn incompatible_different_major() {
        let v1 = Version::new(2, 0, 0);
        let req = VersionRequirement::Compatible(Version::new(1, 0, 0));
        assert!(!v1.is_compatible_with(&req));
    }

    #[test]
    fn unstable_requires_exact() {
        let v1 = Version::new(0, 2, 0);
        let req = VersionRequirement::Compatible(Version::new(0, 1, 0));
        assert!(!v1.is_compatible_with(&req));
    }

    #[test]
    fn range_requirement() {
        let v = Version::new(1, 5, 0);
        let req = VersionRequirement::Range(Version::new(1, 0, 0), Version::new(2, 0, 0));
        assert!(v.is_compatible_with(&req));
    }

    #[test]
    fn at_least_requirement() {
        let v = Version::new(2, 0, 0);
        let req = VersionRequirement::at_least(1, 5, 0);
        assert!(v.is_compatible_with(&req));
    }

    #[test]
    fn version_display() {
        let v = Version::new(1, 2, 3);
        assert_eq!(v.to_string(), "1.2.3");
    }

    #[test]
    fn pre_release_less_than_release() {
        let v1 = Version::with_pre(1, 0, 0, "alpha");
        let v2 = Version::new(1, 0, 0);
        assert!(v1 < v2);
    }
}
