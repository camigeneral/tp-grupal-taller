use std::env;
use std::path::Path;
use rusty_docs::vars::DOCKER;

pub fn get_resource_path<P: AsRef<Path>>(relative_path: P) -> String {
    let cwd = env::current_dir().expect("Failed to get directory");
    let full_path = cwd.join(relative_path);

    full_path
        .to_str()
        .expect("Failed to convert path to string")
        .to_string()
}

pub fn get_node_address(port: usize) -> String {
    let last_digit = port % 10;
    if DOCKER {
        format!("node{}:{}", last_digit, port)
    } else {
        format!("127.0.0.1:{}", port)
    }
}