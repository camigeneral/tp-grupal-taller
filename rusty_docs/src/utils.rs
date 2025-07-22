use std::env;
use std::path::Path;

pub fn get_resource_path<P: AsRef<Path>>(relative_path: P) -> String {
    let exe_path = env::current_exe().expect("Failed to get path");
    let exe_dir = exe_path.parent().expect("Failed to get directory");
    let full_path = exe_dir.join(relative_path);

    full_path
        .to_str()
        .expect("Failed to convert path to string")
        .to_string()
}