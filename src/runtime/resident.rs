use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResidentStatus {
    Running,
    Paused,
    Stopped,
}

impl ResidentStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Paused => "paused",
            Self::Stopped => "stopped",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResidentHive {
    pub resident_id: String,
    pub task_id: String,
    pub worker_node_id: String,
    pub purpose: String,
    pub status: ResidentStatus,
    pub started_at: String,
    pub last_seen_at: String,
}

impl ResidentHive {
    pub fn running(
        resident_id: String,
        task_id: String,
        worker_node_id: String,
        purpose: String,
        started_at: String,
    ) -> Self {
        Self {
            resident_id,
            task_id,
            worker_node_id,
            purpose,
            status: ResidentStatus::Running,
            last_seen_at: started_at.clone(),
            started_at,
        }
    }

    pub fn refresh(&mut self, timestamp: String) {
        self.status = ResidentStatus::Running;
        self.last_seen_at = timestamp;
    }

    pub fn pause(&mut self, timestamp: String) {
        self.status = ResidentStatus::Paused;
        self.last_seen_at = timestamp;
    }

    pub fn stop(&mut self, timestamp: String) {
        self.status = ResidentStatus::Stopped;
        self.last_seen_at = timestamp;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refresh_sets_running_and_updates_last_seen() {
        let mut resident = ResidentHive::running(
            "resident-a".to_owned(),
            "task-a".to_owned(),
            "worker-a".to_owned(),
            "session_watch".to_owned(),
            "unix_ms:100".to_owned(),
        );
        resident.status = ResidentStatus::Paused;

        resident.refresh("unix_ms:200".to_owned());

        assert_eq!(resident.status, ResidentStatus::Running);
        assert_eq!(resident.started_at, "unix_ms:100");
        assert_eq!(resident.last_seen_at, "unix_ms:200");
    }

    #[test]
    fn stop_sets_stopped_and_updates_last_seen() {
        let mut resident = ResidentHive::running(
            "resident-b".to_owned(),
            "task-b".to_owned(),
            "worker-b".to_owned(),
            "session_watch".to_owned(),
            "unix_ms:100".to_owned(),
        );

        resident.stop("unix_ms:300".to_owned());

        assert_eq!(resident.status, ResidentStatus::Stopped);
        assert_eq!(resident.started_at, "unix_ms:100");
        assert_eq!(resident.last_seen_at, "unix_ms:300");
    }

    #[test]
    fn pause_sets_paused_and_updates_last_seen() {
        let mut resident = ResidentHive::running(
            "resident-c".to_owned(),
            "task-c".to_owned(),
            "worker-c".to_owned(),
            "session_watch".to_owned(),
            "unix_ms:100".to_owned(),
        );

        resident.pause("unix_ms:250".to_owned());

        assert_eq!(resident.status, ResidentStatus::Paused);
        assert_eq!(resident.started_at, "unix_ms:100");
        assert_eq!(resident.last_seen_at, "unix_ms:250");
    }
}
