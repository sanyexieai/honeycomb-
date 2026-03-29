use serde::{Deserialize, Serialize};

use crate::core::{EVOLUTION_SCHEMA_VERSION, current_timestamp};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArchitectureReflectionDecision {
    NoMajorDrift,
    DriftDetected,
}

impl ArchitectureReflectionDecision {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoMajorDrift => "no_major_drift",
            Self::DriftDetected => "drift_detected",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GuardrailSnapshotCount {
    pub label: String,
    pub count: usize,
}

impl GuardrailSnapshotCount {
    pub fn new(label: String, count: usize) -> Self {
        Self { label, count }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchitectureGuardrailSnapshot {
    pub window_label: String,
    pub total_count: usize,
    #[serde(default)]
    pub action_counts: Vec<GuardrailSnapshotCount>,
    #[serde(default)]
    pub reason_counts: Vec<GuardrailSnapshotCount>,
    #[serde(default)]
    pub target_type_counts: Vec<GuardrailSnapshotCount>,
    #[serde(default)]
    pub target_id_counts: Vec<GuardrailSnapshotCount>,
    #[serde(default)]
    pub skill_counts: Vec<GuardrailSnapshotCount>,
}

impl ArchitectureGuardrailSnapshot {
    pub fn new(
        window_label: String,
        total_count: usize,
        action_counts: Vec<GuardrailSnapshotCount>,
        reason_counts: Vec<GuardrailSnapshotCount>,
        target_type_counts: Vec<GuardrailSnapshotCount>,
        target_id_counts: Vec<GuardrailSnapshotCount>,
        skill_counts: Vec<GuardrailSnapshotCount>,
    ) -> Self {
        Self {
            window_label,
            total_count,
            action_counts,
            reason_counts,
            target_type_counts,
            target_id_counts,
            skill_counts,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchitectureReflectionRecord {
    pub schema_version: String,
    pub reflection_id: String,
    pub title: String,
    pub period_label: String,
    pub recorded_by: String,
    pub decision: ArchitectureReflectionDecision,
    pub summary: String,
    #[serde(default)]
    pub detected_drifts: Vec<String>,
    #[serde(default)]
    pub freeze_actions: Vec<String>,
    #[serde(default)]
    pub next_actions: Vec<String>,
    #[serde(default)]
    pub review_refs: Vec<String>,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
    #[serde(default)]
    pub guardrail_snapshot: Option<ArchitectureGuardrailSnapshot>,
    pub created_at: String,
    pub updated_at: String,
}

impl ArchitectureReflectionRecord {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        reflection_id: String,
        title: String,
        period_label: String,
        recorded_by: String,
        decision: ArchitectureReflectionDecision,
        summary: String,
        detected_drifts: Vec<String>,
        freeze_actions: Vec<String>,
        next_actions: Vec<String>,
        review_refs: Vec<String>,
        evidence_refs: Vec<String>,
        guardrail_snapshot: Option<ArchitectureGuardrailSnapshot>,
    ) -> Self {
        let timestamp = current_timestamp();
        Self {
            schema_version: EVOLUTION_SCHEMA_VERSION.to_owned(),
            reflection_id,
            title,
            period_label,
            recorded_by,
            decision,
            summary,
            detected_drifts,
            freeze_actions,
            next_actions,
            review_refs,
            evidence_refs,
            guardrail_snapshot,
            created_at: timestamp.clone(),
            updated_at: timestamp,
        }
    }
}
