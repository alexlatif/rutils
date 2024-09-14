use crate::prelude::*;
use std::fs;

pub fn assert_files_exist(files: Vec<&str>) {
    for file in files {
        if fs::metadata(file).is_err() {
            error!(
                "Error: Required file '{}' not found in the current directory",
                file
            );
            std::process::exit(1);
        }
    }
}
