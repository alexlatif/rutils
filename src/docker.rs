use crate::utils::cmd::run_command;
use std::env;
use std::process::Command;
pub use tracing::debug;

pub fn ensure_docker_running() -> Result<(), String> {
    // Check if Docker is installed and running
    if let Err(_) = Command::new("docker").arg("info").output() {
        debug!("Docker is not installed or not running. Attempting to install and start...");

        let os = env::consts::OS;
        match os {
            "macos" => install_and_start_docker_macos(),
            "linux" => install_and_start_docker_linux(),
            _ => return Err(format!("Unsupported operating system: {}", os)),
        }?;
    } else {
        debug!("Docker is installed and running.");
    }

    Ok(())
}

fn install_and_start_docker_macos() -> Result<(), String> {
    // Check if Homebrew is installed
    if let Err(_) = Command::new("brew").arg("--version").output() {
        return Err("Homebrew is not installed. Please install Homebrew first.".to_string());
    }

    // Install Docker using Homebrew
    run_command("brew", &["install", "docker"])
        .map_err(|e| format!("Failed to install Docker: {}", e))?;

    // Start Docker
    run_command("open", &["-a", "Docker"]).map_err(|e| format!("Failed to start Docker: {}", e))?;

    debug!("Docker has been installed and started on macOS.");
    Ok(())
}

fn install_and_start_docker_linux() -> Result<(), String> {
    // Detect the Linux distribution
    let output = Command::new("cat")
        .arg("/etc/os-release")
        .output()
        .map_err(|e| format!("Failed to detect Linux distribution: {}", e))?;
    let os_release = String::from_utf8_lossy(&output.stdout);

    if os_release.contains("Ubuntu") || os_release.contains("Debian") {
        // Update package list
        run_command("sudo", &["apt-get", "update"])
            .map_err(|e| format!("Failed to update package list: {}", e))?;

        // Install Docker on Ubuntu/Debian
        run_command("sudo", &["apt-get", "install", "-y", "docker"])
            .map_err(|e| format!("Failed to install Docker: {}", e))?;
    } else if os_release.contains("CentOS") || os_release.contains("Fedora") {
        // Install Docker on CentOS/Fedora
        run_command("sudo", &["yum", "install", "-y", "docker"])
            .map_err(|e| format!("Failed to install Docker: {}", e))?;
    } else {
        return Err("Unsupported Linux distribution".to_string());
    }

    // Start Docker service
    run_command("sudo", &["systemctl", "start", "docker"])
        .map_err(|e| format!("Failed to start Docker service: {}", e))?;

    // Enable Docker service to start on boot
    run_command("sudo", &["systemctl", "enable", "docker"])
        .map_err(|e| format!("Failed to enable Docker service: {}", e))?;

    debug!("Docker has been installed and started on Linux.");
    Ok(())
}
