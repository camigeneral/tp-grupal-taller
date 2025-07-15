pub fn extract_document_name(resp: &str) -> Option<String> {
    let parts: Vec<&str> = resp.split("\r\n").collect();

    for part in parts.iter().rev() {
        if !part.is_empty() && (part.ends_with(".txt") || part.ends_with(".xlsx")) {
            return Some(part.to_string());
        }
    }

    None
}

