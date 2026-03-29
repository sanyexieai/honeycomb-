use serde::{Deserialize, Serialize};

use super::{GovernanceDecision, GovernedImplementation};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvolutionPlan {
    pub implementation: GovernedImplementation,
    pub decision: GovernanceDecision,
    pub rationale: String,
}

impl EvolutionPlan {
    pub fn observe(implementation: GovernedImplementation, rationale: String) -> Self {
        Self {
            implementation,
            decision: GovernanceDecision::Observe,
            rationale,
        }
    }

    pub fn implementation_id(&self) -> &str {
        &self.implementation.implementation_id
    }
}
