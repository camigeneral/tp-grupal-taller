use super::redis_response::RedisResponse;
use commands::redis_parser::CommandResponse;
use std::collections::HashSet;
use std::fs;
use redis_types::RedisDocumentsMap;


pub fn get_files(_docs: &RedisDocumentsMap) -> RedisResponse {
    let mut doc_names = HashSet::new();

    if let Ok(entries) = fs::read_dir(".") {
        for entry in entries.map_while(Result::ok) {
            let path = entry.path();
            let fname = path
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or("")
                .to_string();
            if fname.starts_with("redis_node_") && fname.ends_with(".rdb") {
                if let Ok(file) = fs::File::open(&path) {
                    use std::io::{BufRead, BufReader};
                    let reader = BufReader::new(file);
                    for line in reader.lines().flatten() {
                        if let Some((doc_name, _)) = line.split_once("/++/") {
                            if !doc_name.trim().is_empty() {
                                doc_names.insert(doc_name.trim().to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    let mut doc_names_vec: Vec<String> = doc_names.into_iter().collect();
    doc_names_vec.sort();
    let mut vector_doc: Vec<CommandResponse> = vec![CommandResponse::String("FILES".to_string())];
    for doc in doc_names_vec {
        vector_doc.push(CommandResponse::String(doc.clone()));
    }

    RedisResponse::new(
        CommandResponse::Array(vector_doc),
        true,
        "".to_string(),
        "".to_string(),
    )
}
