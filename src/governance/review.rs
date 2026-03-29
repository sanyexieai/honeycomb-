use serde::{Deserialize, Serialize};

use crate::core::{EVOLUTION_SCHEMA_VERSION, current_timestamp};

use super::ArchitectureGuardrailSnapshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewTargetPlane {
    Execution,
    Evolution,
    CrossPlane,
}

impl ReviewTargetPlane {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Execution => "execution",
            Self::Evolution => "evolution",
            Self::CrossPlane => "cross_plane",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArchitectureReviewStatus {
    Open,
    Completed,
}

impl ArchitectureReviewStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Completed => "completed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArchitectureReviewDecision {
    Pass,
    PassWithFollowup,
    NeedsRedesign,
    Blocked,
}

impl ArchitectureReviewDecision {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::PassWithFollowup => "pass_with_followup",
            Self::NeedsRedesign => "needs_redesign",
            Self::Blocked => "blocked",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchitectureReviewRecord {
    pub schema_version: String,
    pub review_id: String,
    pub title: String,
    pub change_scope: String,
    pub requested_by: String,
    pub target_plane: ReviewTargetPlane,
    #[serde(default)]
    pub target_modules: Vec<String>,
    #[serde(default)]
    pub writes_runtime: bool,
    #[serde(default)]
    pub writes_long_term: bool,
    #[serde(default)]
    pub mutates_historical_facts: bool,
    #[serde(default)]
    pub touches_registry: bool,
    #[serde(default)]
    pub touches_approval_or_policy: bool,
    pub status: ArchitectureReviewStatus,
    pub decision: ArchitectureReviewDecision,
    pub rationale: String,
    #[serde(default)]
    pub required_followups: Vec<String>,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
    #[serde(default)]
    pub guardrail_snapshot: Option<ArchitectureGuardrailSnapshot>,
    pub created_at: String,
    pub updated_at: String,
}

impl ArchitectureReviewRecord {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        review_id: String,
        title: String,
        change_scope: String,
        requested_by: String,
        target_plane: ReviewTargetPlane,
        target_modules: Vec<String>,
        writes_runtime: bool,
        writes_long_term: bool,
        mutates_historical_facts: bool,
        touches_registry: bool,
        touches_approval_or_policy: bool,
        status: ArchitectureReviewStatus,
        decision: ArchitectureReviewDecision,
        rationale: String,
        required_followups: Vec<String>,
        evidence_refs: Vec<String>,
        guardrail_snapshot: Option<ArchitectureGuardrailSnapshot>,
    ) -> Self {
        let timestamp = current_timestamp();
        Self {
            schema_version: EVOLUTION_SCHEMA_VERSION.to_owned(),
            review_id,
            title,
            change_scope,
            requested_by,
            target_plane,
            target_modules,
            writes_runtime,
            writes_long_term,
            mutates_historical_facts,
            touches_registry,
            touches_approval_or_policy,
            status,
            decision,
            rationale,
            required_followups,
            evidence_refs,
            guardrail_snapshot,
            created_at: timestamp.clone(),
            updated_at: timestamp,
        }
    }
}
