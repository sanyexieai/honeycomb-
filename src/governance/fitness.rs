use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FitnessReport {
    pub implementation_id: String,
    pub score: String,
    pub summary: String,
}

impl FitnessReport {
    pub fn new(implementation_id: String, score: String, summary: String) -> Self {
        Self {
            implementation_id,
            score,
            summary,
        }
    }
}
