use crate::prelude::*;
use std::{
    io::{BufRead, BufReader},
    process::{Command, Stdio},
};

pub fn run_python_script_with_args(file: &str, args: Option<&[&str]>) {
    let dummy = vec![""];
    let args = args.unwrap_or_else(|| &dummy);

    let mut cmd = Command::new("pdm")
        .arg("run")
        .arg(file)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start pdm run script");

    let stdout = cmd.stdout.take().expect("Failed to capture stdout");
    let stderr = cmd.stderr.take().expect("Failed to capture stderr");

    let stdout_reader = BufReader::new(stdout);
    let stderr_reader = BufReader::new(stderr);

    // Process both stdout and stderr
    for line in stdout_reader.lines().chain(stderr_reader.lines()) {
        match line {
            Ok(line) => {
                info!("{}", line);
            }
            Err(e) => error!("Error reading line: {}", e),
        }
    }

    let status = cmd.wait().expect("Failed to wait on child process");

    if !status.success() {
        info!("Python script failed with status: {}", status);
    }
}
