use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::governance::GovernanceDecision;

fn default_requested_by() -> String {
    "unknown".to_owned()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillRecord {
    pub skill_id: String,
    pub display_name: String,
    pub description: String,
    pub implementation_ref: String,
    pub owner: String,
    pub version: String,
    #[serde(default)]
    pub default_tool_refs: Vec<String>,
    #[serde(default)]
    pub goal_template: Option<String>,
    #[serde(default)]
    pub recommended_implementation_id: Option<String>,
    #[serde(default)]
    pub governance_decision: Option<GovernanceDecision>,
    #[serde(default)]
    pub last_synced_at: Option<String>,
    #[serde(default)]
    pub governance_policy: BTreeMap<String, String>,
}

impl SkillRecord {
    pub fn new(
        skill_id: String,
        display_name: String,
        description: String,
        implementation_ref: String,
        owner: String,
        version: String,
        default_tool_refs: Vec<String>,
        goal_template: Option<String>,
    ) -> Self {
        Self {
            skill_id,
            display_name,
            description,
            implementation_ref,
            owner,
            version,
            default_tool_refs,
            goal_template,
            recommended_implementation_id: None,
            governance_decision: None,
            last_synced_at: None,
            governance_policy: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceDefaultsRecord {
    #[serde(default)]
    pub governance_policy: BTreeMap<String, String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

impl GovernanceDefaultsRecord {
    pub fn new() -> Self {
        Self {
            governance_policy: BTreeMap::new(),
            updated_at: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImplementationEntry {
    pub kind: String,
    pub path: String,
}

impl ImplementationEntry {
    pub fn new(kind: String, path: String) -> Self {
        Self { kind, path }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImplementationCompatibility {
    pub capability: String,
    pub input_schema_version: String,
    pub output_schema_version: String,
}

impl ImplementationCompatibility {
    pub fn new(
        capability: String,
        input_schema_version: String,
        output_schema_version: String,
    ) -> Self {
        Self {
            capability,
            input_schema_version,
            output_schema_version,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImplementationOrigin {
    pub source: String,
    #[serde(default)]
    pub parent_impl: Option<String>,
}

impl ImplementationOrigin {
    pub fn new(source: String, parent_impl: Option<String>) -> Self {
        Self {
            source,
            parent_impl,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImplementationRecord {
    pub implementation_id: String,
    pub skill_id: String,
    pub executor: String,
    pub entry: ImplementationEntry,
    #[serde(default)]
    pub components: BTreeMap<String, String>,
    #[serde(default)]
    pub strategy: BTreeMap<String, String>,
    pub compatibility: ImplementationCompatibility,
    #[serde(default)]
    pub constraints: BTreeMap<String, String>,
    #[serde(default)]
    pub origin: Option<ImplementationOrigin>,
}

impl ImplementationRecord {
    pub fn new(
        implementation_id: String,
        skill_id: String,
        executor: String,
        entry: ImplementationEntry,
        compatibility: ImplementationCompatibility,
    ) -> Self {
        Self {
            implementation_id,
            skill_id,
            executor,
            entry,
            components: BTreeMap::new(),
            strategy: BTreeMap::new(),
            compatibility,
            constraints: BTreeMap::new(),
            origin: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolRecord {
    pub tool_id: String,
    pub display_name: String,
    pub description: String,
    pub entrypoint: String,
    pub owner: String,
    pub version: String,
    #[serde(default)]
    pub allow_shell: bool,
    #[serde(default)]
    pub shell_approval_pending: bool,
    #[serde(default)]
    pub shell_approval_request_id: Option<String>,
}

impl ToolRecord {
    pub fn new(
        tool_id: String,
        display_name: String,
        description: String,
        entrypoint: String,
        owner: String,
        version: String,
        allow_shell: bool,
        shell_approval_pending: bool,
        shell_approval_request_id: Option<String>,
    ) -> Self {
        Self {
            tool_id,
            display_name,
            description,
            entrypoint,
            owner,
            version,
            allow_shell,
            shell_approval_pending,
            shell_approval_request_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalRequestStatus {
    Pending,
    Approved,
    Rejected,
}

impl ApprovalRequestStatus {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Rejected => "rejected",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShellApprovalRequest {
    pub request_id: String,
    pub tool_id: String,
    pub owner: String,
    pub entrypoint: String,
    #[serde(default = "default_requested_by")]
    pub requested_by: String,
    pub requested_at: String,
    pub status: ApprovalRequestStatus,
    #[serde(default)]
    pub resolved_at: Option<String>,
    #[serde(default)]
    pub resolved_by: Option<String>,
    #[serde(default)]
    pub resolution_note: Option<String>,
}

impl ShellApprovalRequest {
    pub fn pending(
        request_id: String,
        tool_id: String,
        owner: String,
        entrypoint: String,
        requested_by: String,
        requested_at: String,
    ) -> Self {
        Self {
            request_id,
            tool_id,
            owner,
            entrypoint,
            requested_by,
            requested_at,
            status: ApprovalRequestStatus::Pending,
            resolved_at: None,
            resolved_by: None,
            resolution_note: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertKind {
    BlockedTool,
    OverdueRequest,
}

impl AlertKind {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::BlockedTool => "blocked_tool",
            Self::OverdueRequest => "overdue_request",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyAlertAck {
    pub alert_id: String,
    pub kind: AlertKind,
    pub target_id: String,
    pub acked_by: String,
    pub acked_at: String,
    #[serde(default)]
    pub note: Option<String>,
}

impl PolicyAlertAck {
    pub fn new(
        alert_id: String,
        kind: AlertKind,
        target_id: String,
        acked_by: String,
        acked_at: String,
        note: Option<String>,
    ) -> Self {
        Self {
            alert_id,
            kind,
            target_id,
            acked_by,
            acked_at,
            note,
        }
    }
}
