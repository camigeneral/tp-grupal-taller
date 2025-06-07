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