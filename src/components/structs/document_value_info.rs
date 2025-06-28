extern crate chrono;
use self::chrono::Local;

pub fn get_timestamp_millis() -> i64 {
    let now = Local::now();
    now.timestamp_millis()
}

#[derive(Debug)]
pub struct DocumentValueInfo {
    pub file: String,
    pub value: String,
    pub index: i32,
    pub timestamp: i64,
}

impl DocumentValueInfo {
    pub fn new(value: String, index: i32) -> Self {
        DocumentValueInfo {
            file: String::new(),
            value,
            index,
            timestamp: get_timestamp_millis(),
        }
    }

    pub fn parse_text(&mut self) {
        self.value = if self.value.trim_end_matches('\n').is_empty() {
            "<delete>".to_string()
        } else {
            self.value.replace('\n', "<enter>")
        };
        self.value = self.value.replace(' ', "<space>");
    }

    pub fn decode_text(&mut self) {
        self.value = self
            .value
            .replace("<space>", " ")
            .replace("<enter>", "\n")
            .replace("<delete>", "");
    }
}
