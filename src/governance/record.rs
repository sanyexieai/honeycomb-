use serde::{Deserialize, Serialize};

use super::{EvolutionPlan, FitnessReport};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FitnessRecord {
    pub schema_version: String,
    pub fitness_report: FitnessReport,
    pub evolution_plan: EvolutionPlan,
}
