use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageKind {
    Hello,
    HelloAck,
    Heartbeat,
    TaskAssign,
    TaskProgress,
    TaskResult,
    Shutdown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolEnvelope {
    pub msg_id: String,
    pub kind: MessageKind,
    pub protocol_version: String,
    pub from: String,
    pub to: String,
    pub task_id: String,
    pub tenant_id: String,
    pub namespace: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskResultPayload {
    pub assignment_id: String,
    pub attempt_id: String,
    pub worker_node_id: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HelloPayload {
    pub worker_node_id: String,
    pub queen_node_id: String,
    pub queen_token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HelloAckPayload {
    pub accepted: bool,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeartbeatPayload {
    pub worker_node_id: String,
    pub state: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShutdownPayload {
    pub queen_node_id: String,
    pub worker_node_id: String,
    pub reason: String,
}
