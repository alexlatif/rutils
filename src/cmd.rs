use crate::err;
use std::{
    io::{BufRead, BufReader},
    process::{Command, Stdio},
};
use tokio::io::{AsyncBufReadExt, BufReader as AsyncBufReader};
use tokio::process::Command as TokioCommand;
pub use tracing::{debug, error, info};

use super::errors::{AnyErr, RResult};

fn stream_output(child: &mut std::process::Child) -> RResult<(), AnyErr> {
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| err!(AnyErr, "Failed to take stdout"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| err!(AnyErr, "Failed to take stderr"))?;

    let stdout_reader = BufReader::new(stdout);
    let stderr_reader = BufReader::new(stderr);

    let stdout_handle = std::thread::spawn(move || {
        for line in stdout_reader.lines() {
            match line {
                Ok(line) => debug!("{}", line),
                Err(e) => error!("Error reading stdout line: {}", e),
            }
        }
    });

    let stderr_handle = std::thread::spawn(move || {
        for line in stderr_reader.lines() {
            match line {
                Ok(line) => debug!("{}", line),
                Err(e) => error!("Error reading stderr line: {}", e),
            }
        }
    });

    stdout_handle
        .join()
        .map_err(|_| err!(AnyErr, "Failed to join stdout thread"))?;
    stderr_handle
        .join()
        .map_err(|_| err!(AnyErr, "Failed to join stderr thread"))?;

    Ok(())
}

pub fn run_command(command: &str, args: &[&str]) -> RResult<(), AnyErr> {
    let mut child = Command::new(command)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| err!(AnyErr, "Failed to spawn command: {}", e))?;

    stream_output(&mut child)?;

    let status = child
        .wait()
        .map_err(|e| err!(AnyErr, "Failed to wait for command: {}", e))?;

    if !status.success() {
        return Err(err!(
            AnyErr,
            "Command '{}' failed with status: {}",
            command,
            status
        ));
    }

    Ok(())
}

pub async fn run_async_command(command: &str, args: &[&str]) -> RResult<(), AnyErr> {
    let mut child = TokioCommand::new(command)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| err!(AnyErr, "Failed to spawn command: {}", e))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| err!(AnyErr, "Failed to take stdout"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| err!(AnyErr, "Failed to take stderr"))?;

    let mut stdout_reader = tokio::io::BufReader::new(stdout).lines();
    let mut stderr_reader = tokio::io::BufReader::new(stderr).lines();

    let stdout_handle = tokio::spawn(async move {
        while let Some(line) = stdout_reader.next_line().await.unwrap_or(None) {
            println!("{}", line);
        }
    });

    let stderr_handle = tokio::spawn(async move {
        while let Some(line) = stderr_reader.next_line().await.unwrap_or(None) {
            eprintln!("{}", line);
        }
    });

    stdout_handle
        .await
        .map_err(|_| err!(AnyErr, "Failed to join stdout task"))?;
    stderr_handle
        .await
        .map_err(|_| err!(AnyErr, "Failed to join stderr task"))?;

    let status = child
        .wait()
        .await
        .map_err(|e| err!(AnyErr, "Failed to wait for command: {}", e))?;
    if !status.success() {
        return Err(err!(
            AnyErr,
            "Command '{}' failed with status: {}",
            command,
            status
        ));
    }

    Ok(())
}

pub fn run_python_script(file: &str, args: Option<&[&str]>) {
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

pub fn run_background_python_script(file: &str, args: Option<&[&str]>) -> JoinHandle<()> {
    let file = file.to_string(); // Convert to owned `String`
    let args = args
        .unwrap_or(&[])
        .iter()
        .map(|&s| s.to_string())
        .collect::<Vec<String>>(); // Convert `args` to owned `Vec<String>`

    // Spawn the command asynchronously in a new task
    tokio::spawn(async move {
        let mut cmd = TokioCommand::new("pdm")
            .arg("run")
            .arg(file) // `file` is now owned
            .args(&args) // Pass owned `Vec<String>` to the args
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Failed to start pdm run script");

        // Take stdout and stderr streams
        let stdout = cmd.stdout.take().expect("Failed to capture stdout");
        let stderr = cmd.stderr.take().expect("Failed to capture stderr");

        let stdout_reader = AsyncBufReader::new(stdout);
        let stderr_reader = AsyncBufReader::new(stderr);

        // Process stdout and stderr asynchronously in separate tasks
        let stdout_task = tokio::task::spawn(async move {
            let mut lines = stdout_reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                info!("stdout: {}", line);
            }
        });

        let stderr_task = tokio::task::spawn(async move {
            let mut lines = stderr_reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                error!("stderr: {}", line);
            }
        });

        // Wait for the Python script to complete and ensure logs are processed
        let status = cmd.wait().await.expect("Failed to wait on child process");

        // Ensure both stdout and stderr tasks are finished
        let _ = tokio::join!(stdout_task, stderr_task);

        if !status.success() {
            info!("Python script failed with status: {}", status);
        }
    })
}
