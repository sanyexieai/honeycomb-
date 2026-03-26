use serde::{Deserialize, Serialize};

use crate::core::current_timestamp;

use super::{TaskRuntime, TaskSpec};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskRecord {
    pub schema_version: String,
    pub task_spec: TaskSpec,
    pub task_runtime: TaskRuntime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventRecord {
    pub event_id: String,
    pub event_type: String,
    pub task_id: String,
    pub timestamp: String,
    pub payload: String,
}

impl EventRecord {
    pub fn new(
        event_id: String,
        event_type: String,
        task_id: String,
        timestamp: String,
        payload: String,
    ) -> Self {
        Self {
            event_id,
            event_type,
            task_id,
            timestamp,
            payload,
        }
    }

    pub fn now(event_id: String, event_type: String, task_id: String, payload: String) -> Self {
        Self::new(event_id, event_type, task_id, current_timestamp(), payload)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditRecord {
    pub audit_id: String,
    pub timestamp: String,
    pub actor_type: String,
    pub actor_id: String,
    pub action: String,
    pub target_type: String,
    pub target_id: String,
    pub task_id: String,
    pub result: String,
    pub payload: String,
}

impl AuditRecord {
    pub fn new(
        audit_id: String,
        timestamp: String,
        actor_type: String,
        actor_id: String,
        action: String,
        target_type: String,
        target_id: String,
        task_id: String,
        result: String,
        payload: String,
    ) -> Self {
        Self {
            audit_id,
            timestamp,
            actor_type,
            actor_id,
            action,
            target_type,
            target_id,
            task_id,
            result,
            payload,
        }
    }

    pub fn now(
        audit_id: String,
        actor_type: String,
        actor_id: String,
        action: String,
        target_type: String,
        target_id: String,
        task_id: String,
        result: String,
        payload: String,
    ) -> Self {
        Self::new(
            audit_id,
            current_timestamp(),
            actor_type,
            actor_id,
            action,
            target_type,
            target_id,
            task_id,
            result,
            payload,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceRecord {
    pub trace_id: String,
    pub span_id: String,
    pub parent_span_id: Option<String>,
    pub timestamp: String,
    pub event_type: String,
    pub task_id: String,
    pub status: String,
    pub payload: String,
}

impl TraceRecord {
    pub fn new(
        trace_id: String,
        span_id: String,
        parent_span_id: Option<String>,
        timestamp: String,
        event_type: String,
        task_id: String,
        status: String,
        payload: String,
    ) -> Self {
        Self {
            trace_id,
            span_id,
            parent_span_id,
            timestamp,
            event_type,
            task_id,
            status,
            payload,
        }
    }

    pub fn now(
        trace_id: String,
        span_id: String,
        parent_span_id: Option<String>,
        event_type: String,
        task_id: String,
        status: String,
        payload: String,
    ) -> Self {
        Self::new(
            trace_id,
            span_id,
            parent_span_id,
            current_timestamp(),
            event_type,
            task_id,
            status,
            payload,
        )
    }
}
