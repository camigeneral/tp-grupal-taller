use std::fs::OpenOptions;
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn log_event(log_path: &str, message: &str) {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    let log_line = format!("[{}] {}\n", now.as_secs(), message);

    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(log_path) {
        let _ = file.write_all(log_line.as_bytes());
    }
}

pub fn get_log_path_from_config(config_path: &str) -> String {
    let config = std::fs::read_to_string(config_path).unwrap_or_default();
    for line in config.lines() {
        if let Some(path) = line.strip_prefix("log_path=") {
            return path.trim().to_string();
        }
    }
    "server.log".to_string()
}