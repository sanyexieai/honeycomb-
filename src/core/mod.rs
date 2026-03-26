pub const EXECUTION_SCHEMA_VERSION: &str = "1.0.0";
pub const EVOLUTION_SCHEMA_VERSION: &str = "1.0.0";
pub const PROTOCOL_VERSION: &str = "1.0.0";

pub fn current_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => format!("unix_ms:{}", duration.as_millis()),
        Err(_) => "unix_ms:0".to_owned(),
    }
}
