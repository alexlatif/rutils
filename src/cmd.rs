use crate::err;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use tokio::io::AsyncBufReadExt;
use tokio::process::Command as TokioCommand;
pub use tracing::{error, info};

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
                Ok(line) => info!("{}", line),
                Err(e) => error!("Error reading stdout line: {}", e),
            }
        }
    });

    let stderr_handle = std::thread::spawn(move || {
        for line in stderr_reader.lines() {
            match line {
                Ok(line) => info!("{}", line),
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
