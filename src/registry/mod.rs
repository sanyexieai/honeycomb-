use serde::{Deserialize, Serialize};

use crate::governance::GovernanceDecision;

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
}

impl ToolRecord {
    pub fn new(
        tool_id: String,
        display_name: String,
        description: String,
        entrypoint: String,
        owner: String,
        version: String,
    ) -> Self {
        Self {
            tool_id,
            display_name,
            description,
            entrypoint,
            owner,
            version,
        }
    }
}
