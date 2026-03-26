use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GovernanceDecision {
    Promote,
    Hold,
    Deprecate,
    Observe,
}

impl GovernanceDecision {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Promote => "promote",
            Self::Hold => "hold",
            Self::Deprecate => "deprecate",
            Self::Observe => "observe",
        }
    }
}
