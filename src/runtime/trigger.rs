use serde::{Deserialize, Serialize};

pub const TRIGGER_STATUS_FIRE_ERR: &str = "trigger status transition not allowed";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerStatus {
    Active,
    Paused,
}

impl TriggerStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Paused => "paused",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trigger {
    pub trigger_id: String,
    pub task_id: String,
    pub trigger_type: String,
    pub schedule: String,
    pub status: TriggerStatus,
    pub fire_count: u64,
    #[serde(default)]
    pub consumed_fire_count: u64,
    pub last_fired_at: Option<String>,
}

impl Trigger {
    pub fn active(
        trigger_id: String,
        task_id: String,
        trigger_type: String,
        schedule: String,
    ) -> Self {
        Self {
            trigger_id,
            task_id,
            trigger_type,
            schedule,
            status: TriggerStatus::Active,
            fire_count: 0,
            consumed_fire_count: 0,
            last_fired_at: None,
        }
    }

    pub fn record_fire(&mut self, timestamp: String) {
        assert!(
            self.status == TriggerStatus::Active,
            "{TRIGGER_STATUS_FIRE_ERR}: fire requires active trigger"
        );
        self.fire_count += 1;
        self.last_fired_at = Some(timestamp);
    }

    pub fn pause(&mut self) {
        self.status = TriggerStatus::Paused;
    }

    pub fn resume(&mut self) {
        self.status = TriggerStatus::Active;
    }

    pub fn try_record_fire(&mut self, timestamp: String) -> Result<(), &'static str> {
        if self.status != TriggerStatus::Active {
            return Err(TRIGGER_STATUS_FIRE_ERR);
        }

        self.fire_count += 1;
        self.last_fired_at = Some(timestamp);
        Ok(())
    }

    pub fn has_unconsumed_fire(&self) -> bool {
        self.fire_count > self.consumed_fire_count
    }

    pub fn is_one_shot(&self) -> bool {
        self.trigger_type == "oneshot" || self.schedule == "once"
    }

    pub fn consume_fire(&mut self) {
        self.consumed_fire_count = self.fire_count;
        if self.is_one_shot() {
            self.status = TriggerStatus::Paused;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_trigger_can_record_fire() {
        let mut trigger = Trigger::active(
            "trigger-a".to_owned(),
            "task-a".to_owned(),
            "manual".to_owned(),
            "on_demand".to_owned(),
        );

        trigger
            .try_record_fire("unix_ms:123".to_owned())
            .expect("active trigger should fire");

        assert_eq!(trigger.fire_count, 1);
        assert_eq!(trigger.consumed_fire_count, 0);
        assert_eq!(trigger.last_fired_at.as_deref(), Some("unix_ms:123"));
    }

    #[test]
    fn paused_trigger_cannot_record_fire() {
        let mut trigger = Trigger::active(
            "trigger-b".to_owned(),
            "task-b".to_owned(),
            "schedule".to_owned(),
            "hourly".to_owned(),
        );
        trigger.pause();

        let result = trigger.try_record_fire("unix_ms:456".to_owned());

        assert_eq!(result, Err(TRIGGER_STATUS_FIRE_ERR));
        assert_eq!(trigger.fire_count, 0);
        assert_eq!(trigger.consumed_fire_count, 0);
        assert_eq!(trigger.last_fired_at, None);
    }

    #[test]
    fn trigger_can_consume_fired_state() {
        let mut trigger = Trigger::active(
            "trigger-c".to_owned(),
            "task-c".to_owned(),
            "schedule".to_owned(),
            "hourly".to_owned(),
        );

        trigger
            .try_record_fire("unix_ms:789".to_owned())
            .expect("active trigger should fire");
        assert!(trigger.has_unconsumed_fire());

        trigger.consume_fire();

        assert!(!trigger.has_unconsumed_fire());
        assert_eq!(trigger.consumed_fire_count, 1);
    }

    #[test]
    fn oneshot_trigger_pauses_after_consume() {
        let mut trigger = Trigger::active(
            "trigger-d".to_owned(),
            "task-d".to_owned(),
            "oneshot".to_owned(),
            "once".to_owned(),
        );

        trigger
            .try_record_fire("unix_ms:999".to_owned())
            .expect("oneshot trigger should fire");
        trigger.consume_fire();

        assert_eq!(trigger.status, TriggerStatus::Paused);
        assert_eq!(trigger.consumed_fire_count, 1);
    }
}
