use std::fs::OpenOptions;
use std::io::Write;

extern crate chrono;
use self::chrono::Local;

pub fn get_readable_datetime() -> String {
    let now = Local::now();
    format!("[{}]", now.format("%Y-%m-%d %H:%M:%S"))
}

pub fn log_event(log_path: &str, message: &str) {
    let datetime = get_readable_datetime();
    let log_line = format!("{} {}\n", datetime, message);

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