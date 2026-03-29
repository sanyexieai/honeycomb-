use serde::{Deserialize, Serialize};

use super::GovernedImplementation;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FitnessReport {
    pub implementation: GovernedImplementation,
    pub score: String,
    pub summary: String,
    #[serde(default)]
    pub skill_refs: Vec<String>,
    #[serde(default)]
    pub tool_refs: Vec<String>,
}

impl FitnessReport {
    pub fn new(
        implementation: GovernedImplementation,
        score: String,
        summary: String,
        skill_refs: Vec<String>,
        tool_refs: Vec<String>,
    ) -> Self {
        Self {
            implementation,
            score,
            summary,
            skill_refs,
            tool_refs,
        }
    }

    pub fn implementation_id(&self) -> &str {
        &self.implementation.implementation_id
    }
}
