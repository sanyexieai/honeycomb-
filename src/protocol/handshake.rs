use crate::core::{current_timestamp, PROTOCOL_VERSION};

use super::message::{
    HeartbeatPayload, HelloAckPayload, HelloPayload, MessageKind, ProtocolEnvelope,
    ShutdownPayload,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueenEndpoint {
    pub queen_node_id: String,
    pub task_id: String,
    pub tenant_id: String,
    pub namespace: String,
    pub queen_token: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandshakeTranscript {
    pub hello: ProtocolEnvelope,
    pub hello_payload: HelloPayload,
    pub ack: ProtocolEnvelope,
    pub ack_payload: HelloAckPayload,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeartbeatTranscript {
    pub heartbeat: ProtocolEnvelope,
    pub payload: HeartbeatPayload,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShutdownTranscript {
    pub shutdown: ProtocolEnvelope,
    pub payload: ShutdownPayload,
}

impl QueenEndpoint {
    pub fn new(
        queen_node_id: String,
        task_id: String,
        tenant_id: String,
        namespace: String,
        queen_token: String,
    ) -> Self {
        Self {
            queen_node_id,
            task_id,
            tenant_id,
            namespace,
            queen_token,
        }
    }
}

pub fn simulate_handshake(
    endpoint: &QueenEndpoint,
    worker_node_id: &str,
    tenant_id: &str,
    namespace: &str,
    task_id: &str,
    queen_node_id: &str,
    queen_token: &str,
) -> HandshakeTranscript {
    let hello = ProtocolEnvelope {
        msg_id: format!("msg-hello-{worker_node_id}"),
        kind: MessageKind::Hello,
        protocol_version: PROTOCOL_VERSION.to_owned(),
        from: worker_node_id.to_owned(),
        to: endpoint.queen_node_id.clone(),
        task_id: task_id.to_owned(),
        tenant_id: tenant_id.to_owned(),
        namespace: namespace.to_owned(),
        timestamp: current_timestamp(),
    };
    let hello_payload = HelloPayload {
        worker_node_id: worker_node_id.to_owned(),
        queen_node_id: queen_node_id.to_owned(),
        queen_token: queen_token.to_owned(),
    };

    let accepted = hello.protocol_version == PROTOCOL_VERSION
        && hello.task_id == endpoint.task_id
        && hello.tenant_id == endpoint.tenant_id
        && hello.namespace == endpoint.namespace
        && hello_payload.queen_node_id == endpoint.queen_node_id
        && hello_payload.queen_token == endpoint.queen_token;
    let reason = if accepted {
        "accepted".to_owned()
    } else {
        first_failure_reason(endpoint, &hello, &hello_payload)
    };

    let ack = ProtocolEnvelope {
        msg_id: format!("msg-hello-ack-{worker_node_id}"),
        kind: MessageKind::HelloAck,
        protocol_version: PROTOCOL_VERSION.to_owned(),
        from: endpoint.queen_node_id.clone(),
        to: worker_node_id.to_owned(),
        task_id: endpoint.task_id.clone(),
        tenant_id: endpoint.tenant_id.clone(),
        namespace: endpoint.namespace.clone(),
        timestamp: current_timestamp(),
    };
    let ack_payload = HelloAckPayload { accepted, reason };

    HandshakeTranscript {
        hello,
        hello_payload,
        ack,
        ack_payload,
    }
}

fn first_failure_reason(
    endpoint: &QueenEndpoint,
    hello: &ProtocolEnvelope,
    hello_payload: &HelloPayload,
) -> String {
    if hello.protocol_version != PROTOCOL_VERSION {
        return "protocol_version_mismatch".to_owned();
    }
    if hello.task_id != endpoint.task_id {
        return "task_id_mismatch".to_owned();
    }
    if hello.tenant_id != endpoint.tenant_id {
        return "tenant_id_mismatch".to_owned();
    }
    if hello.namespace != endpoint.namespace {
        return "namespace_mismatch".to_owned();
    }
    if hello_payload.queen_node_id != endpoint.queen_node_id {
        return "queen_node_id_mismatch".to_owned();
    }
    if hello_payload.queen_token != endpoint.queen_token {
        return "queen_token_invalid".to_owned();
    }
    "unknown".to_owned()
}

pub fn simulate_heartbeat(
    endpoint: &QueenEndpoint,
    worker_node_id: &str,
    state: &str,
) -> HeartbeatTranscript {
    let heartbeat = ProtocolEnvelope {
        msg_id: format!("msg-heartbeat-{worker_node_id}"),
        kind: MessageKind::Heartbeat,
        protocol_version: PROTOCOL_VERSION.to_owned(),
        from: worker_node_id.to_owned(),
        to: endpoint.queen_node_id.clone(),
        task_id: endpoint.task_id.clone(),
        tenant_id: endpoint.tenant_id.clone(),
        namespace: endpoint.namespace.clone(),
        timestamp: current_timestamp(),
    };
    let payload = HeartbeatPayload {
        worker_node_id: worker_node_id.to_owned(),
        state: state.to_owned(),
    };

    HeartbeatTranscript { heartbeat, payload }
}

pub fn simulate_shutdown(
    endpoint: &QueenEndpoint,
    worker_node_id: &str,
    reason: &str,
) -> ShutdownTranscript {
    let shutdown = ProtocolEnvelope {
        msg_id: format!("msg-shutdown-{worker_node_id}"),
        kind: MessageKind::Shutdown,
        protocol_version: PROTOCOL_VERSION.to_owned(),
        from: endpoint.queen_node_id.clone(),
        to: worker_node_id.to_owned(),
        task_id: endpoint.task_id.clone(),
        tenant_id: endpoint.tenant_id.clone(),
        namespace: endpoint.namespace.clone(),
        timestamp: current_timestamp(),
    };
    let payload = ShutdownPayload {
        queen_node_id: endpoint.queen_node_id.clone(),
        worker_node_id: worker_node_id.to_owned(),
        reason: reason.to_owned(),
    };

    ShutdownTranscript { shutdown, payload }
}
