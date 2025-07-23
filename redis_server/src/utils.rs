use std::env;
use std::path::Path;

pub fn get_resource_path<P: AsRef<Path>>(relative_path: P) -> String {
    let cwd = env::current_dir().expect("Failed to get directory");
    let full_path = cwd.join(relative_path);

    full_path
        .to_str()
        .expect("Failed to convert path to string")
        .to_string()
}