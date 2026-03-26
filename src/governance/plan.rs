use serde::{Deserialize, Serialize};

use super::GovernanceDecision;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvolutionPlan {
    pub implementation_id: String,
    pub decision: GovernanceDecision,
    pub rationale: String,
}

impl EvolutionPlan {
    pub fn observe(implementation_id: String, rationale: String) -> Self {
        Self {
            implementation_id,
            decision: GovernanceDecision::Observe,
            rationale,
        }
    }
}
