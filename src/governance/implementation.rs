use serde::{Deserialize, Serialize};

use crate::registry::ImplementationRecord;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernedImplementation {
    pub implementation_id: String,
    pub skill_id: String,
    pub executor: String,
    pub entry_kind: String,
    pub entry_path: String,
    pub component_count: usize,
    pub strategy_count: usize,
    pub constraint_count: usize,
    #[serde(default)]
    pub prompt_component: Option<String>,
    #[serde(default)]
    pub config_component: Option<String>,
    #[serde(default)]
    pub strategy_mode: Option<String>,
    #[serde(default)]
    pub max_cost: Option<String>,
    #[serde(default)]
    pub max_latency_ms: Option<String>,
    #[serde(default)]
    pub origin_source: Option<String>,
}

impl GovernedImplementation {
    pub fn from_record(record: &ImplementationRecord) -> Self {
        Self {
            implementation_id: record.implementation_id.clone(),
            skill_id: record.skill_id.clone(),
            executor: record.executor.clone(),
            entry_kind: record.entry.kind.clone(),
            entry_path: record.entry.path.clone(),
            component_count: record.components.len(),
            strategy_count: record.strategy.len(),
            constraint_count: record.constraints.len(),
            prompt_component: record.components.get("prompt").cloned(),
            config_component: record.components.get("config").cloned(),
            strategy_mode: record.strategy.get("mode").cloned(),
            max_cost: record.constraints.get("max_cost").cloned(),
            max_latency_ms: record.constraints.get("max_latency_ms").cloned(),
            origin_source: record.origin.as_ref().map(|origin| origin.source.clone()),
        }
    }
}
