use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FitnessReport {
    pub implementation_id: String,
    pub score: String,
    pub summary: String,
    #[serde(default)]
    pub skill_refs: Vec<String>,
    #[serde(default)]
    pub tool_refs: Vec<String>,
}

impl FitnessReport {
    pub fn new(
        implementation_id: String,
        score: String,
        summary: String,
        skill_refs: Vec<String>,
        tool_refs: Vec<String>,
    ) -> Self {
        Self {
            implementation_id,
            score,
            summary,
            skill_refs,
            tool_refs,
        }
    }
}
