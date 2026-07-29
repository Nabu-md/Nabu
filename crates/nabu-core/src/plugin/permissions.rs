//! Plugin Permission Model — defines what resources a plugin may access.
//!
//! This is a **foundation only**. No runtime enforcement is implemented yet.
//! The model is designed to be the basis for future sandboxing and capability
//! enforcement.
//!
//! Permissions follow a "least privilege" principle by default.
//! Plugins must explicitly declare every permission they require.

use std::collections::HashSet;

/// A permission that a plugin requests.
///
/// Permissions describe what resources or operations a plugin may access.
/// Permissions are currently defined but NOT enforced at runtime.
/// Enforcement will be added with plugin sandboxing in a future phase.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Permission {
    /// The permission name (e.g., "filesystem.read", "vault.access").
    pub name: String,
    /// Human-readable description of what this permission allows.
    pub description: String,
    /// Risk level associated with this permission.
    pub risk_level: RiskLevel,
    /// Whether this permission is required for the plugin to function.
    pub required: bool,
}

/// Risk level for a permission — helps users understand the implications.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RiskLevel {
    /// No risk — purely informational.
    None,
    /// Low risk — read-only access to non-sensitive data.
    Low,
    /// Medium risk — read/write access to specific resources.
    Medium,
    /// High risk — potentially destructive operations.
    High,
    /// Critical — full system access or data exfiltration potential.
    Critical,
}

impl RiskLevel {
    pub fn label(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }
}

// ---------------------------------------------------------------------------
// Standard Permission Constants
// ---------------------------------------------------------------------------

/// Returns the set of standard permission definitions for the plugin system.
pub fn standard_permissions() -> Vec<Permission> {
    vec![
        // Vault access
        Permission {
            name: "vault.access".into(),
            description: "Read and write access to the user's vault content".into(),
            risk_level: RiskLevel::High,
            required: false,
        },
        Permission {
            name: "vault.read".into(),
            description: "Read-only access to vault content".into(),
            risk_level: RiskLevel::Medium,
            required: false,
        },
        // Capture
        Permission {
            name: "capture.access".into(),
            description: "Access the capture pipeline to inject or intercept content".into(),
            risk_level: RiskLevel::Medium,
            required: false,
        },
        // Filesystem
        Permission {
            name: "filesystem.read".into(),
            description: "Read files on the local filesystem".into(),
            risk_level: RiskLevel::Medium,
            required: false,
        },
        Permission {
            name: "filesystem.write".into(),
            description: "Write files to the local filesystem".into(),
            risk_level: RiskLevel::High,
            required: false,
        },
        // AI
        Permission {
            name: "ai.providers".into(),
            description: "Access configured AI providers".into(),
            risk_level: RiskLevel::High,
            required: false,
        },
        // Network
        Permission {
            name: "network.http".into(),
            description: "Make HTTP requests to external services".into(),
            risk_level: RiskLevel::Critical,
            required: false,
        },
        Permission {
            name: "network.websocket".into(),
            description: "Open WebSocket connections".into(),
            risk_level: RiskLevel::Critical,
            required: false,
        },
        // Export
        Permission {
            name: "export.access".into(),
            description: "Access the export engine".into(),
            risk_level: RiskLevel::Low,
            required: false,
        },
        // System
        Permission {
            name: "system.process".into(),
            description: "Spawn subprocesses".into(),
            risk_level: RiskLevel::Critical,
            required: false,
        },
        Permission {
            name: "system.env".into(),
            description: "Read environment variables".into(),
            risk_level: RiskLevel::Medium,
            required: false,
        },
        // Event Bus
        Permission {
            name: "event_bus.subscribe".into(),
            description: "Subscribe to event bus topics".into(),
            risk_level: RiskLevel::Low,
            required: false,
        },
        Permission {
            name: "event_bus.publish".into(),
            description: "Publish to the event bus".into(),
            risk_level: RiskLevel::Medium,
            required: false,
        },
        // Storage
        Permission {
            name: "storage.access".into(),
            description: "Access the storage layer".into(),
            risk_level: RiskLevel::High,
            required: false,
        },
    ]
}

// ---------------------------------------------------------------------------
// Permission Set
// ---------------------------------------------------------------------------

/// A set of permissions granted to a specific plugin.
///
/// This is currently a foundation data structure.
/// Runtime enforcement will be added in a future phase.
#[derive(Debug, Clone, Default)]
pub struct PermissionSet {
    granted: HashSet<String>,
    denied: HashSet<String>,
}

impl PermissionSet {
    /// Create an empty permission set (no permissions granted).
    pub fn new() -> Self {
        Self {
            granted: HashSet::new(),
            denied: HashSet::new(),
        }
    }

    /// Grant a permission to a plugin.
    pub fn grant(&mut self, permission_name: &str) {
        self.granted.insert(permission_name.to_string());
        self.denied.remove(permission_name);
    }

    /// Deny a permission to a plugin (explicitly revoke).
    pub fn deny(&mut self, permission_name: &str) {
        self.denied.insert(permission_name.to_string());
        self.granted.remove(permission_name);
    }

    /// Check if a specific permission is granted.
    pub fn is_granted(&self, permission_name: &str) -> bool {
        if self.denied.contains(permission_name) {
            return false;
        }
        self.granted.contains(permission_name)
    }

    /// List all granted permissions.
    pub fn granted(&self) -> Vec<&str> {
        let mut perms: Vec<&str> = self.granted.iter().map(|s| s.as_str()).collect();
        perms.sort();
        perms
    }

    /// List all explicitly denied permissions.
    pub fn denied(&self) -> Vec<&str> {
        let mut perms: Vec<&str> = self.denied.iter().map(|s| s.as_str()).collect();
        perms.sort();
        perms
    }

    /// Number of granted permissions.
    pub fn granted_count(&self) -> usize {
        self.granted.len()
    }

    /// Merge another permission set into this one.
    pub fn merge(&mut self, other: &PermissionSet) {
        for perm in other.granted.iter() {
            if !self.denied.contains(perm) {
                self.granted.insert(perm.clone());
            }
        }
        for perm in other.denied.iter() {
            self.denied.insert(perm.clone());
            self.granted.remove(perm);
        }
    }
}

// ---------------------------------------------------------------------------
// Permission Evaluator (Foundation)
// ---------------------------------------------------------------------------

/// Evaluates whether a plugin has the required permissions.
///
/// This is a **foundation** implementation. Runtime enforcement
/// will be added in a future phase alongside plugin sandboxing.
#[derive(Debug, Clone)]
pub struct PermissionEvaluator {
    /// Registry of all known permission definitions.
    known_permissions: Vec<Permission>,
}

impl PermissionEvaluator {
    pub fn new() -> Self {
        Self {
            known_permissions: standard_permissions(),
        }
    }

    pub fn with_permissions(permissions: Vec<Permission>) -> Self {
        Self {
            known_permissions: permissions,
        }
    }

    /// Check if a plugin has all required permissions granted.
    pub fn check_required_permissions(
        &self,
        requested: &[String],
        granted: &PermissionSet,
    ) -> Vec<PermissionCheck> {
        let mut checks = Vec::new();
        for perm_name in requested {
            let known = self.known_permissions.iter()
                .find(|p| p.name == *perm_name);
            let is_granted = granted.is_granted(perm_name);
            checks.push(PermissionCheck {
                permission: perm_name.clone(),
                is_granted,
                risk_level: known.map(|p| p.risk_level).unwrap_or(RiskLevel::High),
                known: known.is_some(),
            });
        }
        checks
    }

    /// Validate that requested permissions are all known and valid.
    pub fn validate_requested(
        &self,
        requested: &[String],
    ) -> Vec<PermissionValidation> {
        let mut results = Vec::new();
        for perm_name in requested {
            let known = self.known_permissions.iter()
                .find(|p| p.name == *perm_name);
            results.push(PermissionValidation {
                permission: perm_name.clone(),
                valid: known.is_some(),
                description: known.map(|p| p.description.clone()),
                risk_level: known.map(|p| p.risk_level),
            });
        }
        results
    }
}

impl Default for PermissionEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of checking a specific permission.
#[derive(Debug, Clone, PartialEq)]
pub struct PermissionCheck {
    pub permission: String,
    pub is_granted: bool,
    pub risk_level: RiskLevel,
    pub known: bool,
}

/// Result of validating a requested permission.
#[derive(Debug, Clone, PartialEq)]
pub struct PermissionValidation {
    pub permission: String,
    pub valid: bool,
    pub description: Option<String>,
    pub risk_level: Option<RiskLevel>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_set_grants_nothing() {
        let ps = PermissionSet::new();
        assert!(!ps.is_granted("vault.access"));
    }

    #[test]
    fn grant_and_check() {
        let mut ps = PermissionSet::new();
        ps.grant("vault.read");
        assert!(ps.is_granted("vault.read"));
    }

    #[test]
    fn deny_overrides_grant() {
        let mut ps = PermissionSet::new();
        ps.grant("network.http");
        ps.deny("network.http");
        assert!(!ps.is_granted("network.http"));
    }

    #[test]
    fn standard_permissions_count() {
        let perms = standard_permissions();
        assert!(perms.len() >= 10);
    }

    #[test]
    fn evaluator_checks_known_permissions() {
        let evaluator = PermissionEvaluator::new();
        let mut ps = PermissionSet::new();
        ps.grant("vault.read");
        let checks = evaluator.check_required_permissions(
            &["vault.read".into(), "network.http".into()],
            &ps,
        );
        assert_eq!(checks.len(), 2);
        assert!(checks[0].is_granted);
        assert!(!checks[1].is_granted);
    }

    #[test]
    fn evaluator_validates_requested() {
        let evaluator = PermissionEvaluator::new();
        let results = evaluator.validate_requested(
            &["vault.read".into(), "unknown.permission".into()],
        );
        assert!(results[0].valid);
        assert!(!results[1].valid);
    }

    #[test]
    fn merge_combines_permissions() {
        let mut ps1 = PermissionSet::new();
        ps1.grant("vault.read");
        let mut ps2 = PermissionSet::new();
        ps2.grant("network.http");
        ps1.merge(&ps2);
        assert!(ps1.is_granted("vault.read"));
        assert!(ps1.is_granted("network.http"));
    }
}
