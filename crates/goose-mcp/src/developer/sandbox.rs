use serde::{Deserialize, Serialize};
use std::env;
use std::path::PathBuf;
use std::str::FromStr;
use tokio::process::Command;

use super::shell::ShellConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxConfig {
    pub enabled: bool,
    pub method: SandboxMethod,
    pub profile: SeatbeltProfile,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SandboxMethod {
    None,
    Seatbelt,
    Docker,
    Podman,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SeatbeltProfile {
    PermissiveOpen,
    PermissiveClosed,
    RestrictiveOpen,
    RestrictiveClosed,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            method: SandboxMethod::None,
            profile: SeatbeltProfile::PermissiveOpen,
        }
    }
}

impl FromStr for SandboxMethod {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "none" | "false" | "disabled" => Ok(SandboxMethod::None),
            "seatbelt" | "sandbox-exec" => Ok(SandboxMethod::Seatbelt),
            "docker" => Ok(SandboxMethod::Docker),
            "podman" => Ok(SandboxMethod::Podman),
            _ => Err(format!("Unknown sandbox method: {}", s)),
        }
    }
}

impl FromStr for SeatbeltProfile {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().replace('-', "_").as_str() {
            "permissive_open" | "permissiveopen" => Ok(SeatbeltProfile::PermissiveOpen),
            "permissive_closed" | "permissiveclosed" => Ok(SeatbeltProfile::PermissiveClosed),
            "restrictive_open" | "restrictiveopen" => Ok(SeatbeltProfile::RestrictiveOpen),
            "restrictive_closed" | "restrictiveclosed" => Ok(SeatbeltProfile::RestrictiveClosed),
            _ => Err(format!("Unknown seatbelt profile: {}", s)),
        }
    }
}

impl SeatbeltProfile {
    fn profile_filename(&self) -> &'static str {
        match self {
            SeatbeltProfile::PermissiveOpen => "permissive-open.sb",
            SeatbeltProfile::PermissiveClosed => "permissive-closed.sb",
            SeatbeltProfile::RestrictiveOpen => "restrictive-open.sb",
            SeatbeltProfile::RestrictiveClosed => "restrictive-closed.sb",
        }
    }
}

pub struct SandboxWrapper {
    config: SandboxConfig,
    project_dir: PathBuf,
    home_dir: PathBuf,
}

impl SandboxWrapper {
    pub fn new(config: SandboxConfig) -> Result<Self, String> {
        // Check for unsupported platform combinations early
        if config.enabled {
            match config.method {
                SandboxMethod::Seatbelt if !cfg!(target_os = "macos") => {
                    return Err(format!(
                        "Seatbelt sandboxing is only available on macOS. Current platform: {}.\n\
                         Available options:\n\
                         - Use --sandbox=docker or --sandbox=podman for container-based sandboxing\n\
                         - Remove --sandbox flag to disable sandboxing",
                        std::env::consts::OS
                    ));
                }
                _ => {} // Docker and Podman work on all platforms
            }
        }

        let project_dir =
            env::current_dir().map_err(|e| format!("Failed to get current directory: {}", e))?;

        let home_dir =
            dirs::home_dir().ok_or_else(|| "Failed to get home directory".to_string())?;

        Ok(Self {
            config,
            project_dir,
            home_dir,
        })
    }

    pub fn wrap_command(
        &self,
        shell_config: &ShellConfig,
        command: &str,
    ) -> Result<Command, String> {
        if !self.config.enabled || self.config.method == SandboxMethod::None {
            return Ok(self.create_direct_command(shell_config, command));
        }

        match self.config.method {
            SandboxMethod::Seatbelt => self.create_seatbelt_command(shell_config, command),
            SandboxMethod::None => Ok(self.create_direct_command(shell_config, command)),
            SandboxMethod::Docker => self.create_docker_command(shell_config, command),
            SandboxMethod::Podman => self.create_podman_command(shell_config, command),
        }
    }

    fn create_direct_command(&self, shell_config: &ShellConfig, command: &str) -> Command {
        let mut cmd = Command::new(&shell_config.executable);
        cmd.args(&shell_config.args);
        cmd.arg(command);
        cmd
    }

    fn create_seatbelt_command(
        &self,
        shell_config: &ShellConfig,
        command: &str,
    ) -> Result<Command, String> {
        // Check if we're on macOS
        if !cfg!(target_os = "macos") {
            return Err(format!(
                "Seatbelt sandboxing is only available on macOS. Current platform: {}. \
                 To disable sandboxing, remove the --sandbox flag or unset GOOSE_SANDBOX environment variable.",
                std::env::consts::OS
            ));
        }

        // Check if sandbox-exec is available
        if which::which("sandbox-exec").is_err() {
            return Err(
                "sandbox-exec command not found. Seatbelt sandboxing requires macOS with sandbox-exec available. \
                 This usually indicates an incomplete macOS installation.".to_string()
            );
        }

        let profile_path = self.get_seatbelt_profile_path()?;

        let mut cmd = Command::new("sandbox-exec");
        cmd.arg("-f");
        cmd.arg(&profile_path);
        cmd.arg("-D");
        cmd.arg(&format!("project_dir={}", self.project_dir.display()));
        cmd.arg("-D");
        cmd.arg(&format!("home_dir={}", self.home_dir.display()));
        cmd.arg(&shell_config.executable);
        cmd.args(&shell_config.args);
        cmd.arg(command);

        Ok(cmd)
    }

    fn get_seatbelt_profile_path(&self) -> Result<PathBuf, String> {
        // Get the path to the profile file embedded in the binary
        let profile_filename = self.config.profile.profile_filename();

        // For now, use profiles from the source directory
        // In a production build, these would be embedded as resources
        let mut profile_path =
            env::current_dir().map_err(|e| format!("Failed to get current directory: {}", e))?;

        profile_path.push("crates");
        profile_path.push("goose-mcp");
        profile_path.push("src");
        profile_path.push("developer");
        profile_path.push("profiles");
        profile_path.push(profile_filename);

        if !profile_path.exists() {
            return Err(format!(
                "Seatbelt profile not found: {}",
                profile_path.display()
            ));
        }

        Ok(profile_path)
    }

    fn create_docker_command(
        &self,
        shell_config: &ShellConfig,
        command: &str,
    ) -> Result<Command, String> {
        // Check if Docker is available
        if which::which("docker").is_err() {
            return Err(format!(
                "Docker not found. Docker sandboxing requires Docker to be installed and available in PATH.\n\
                 Install Docker from https://docker.com and ensure 'docker' command is available.\n\
                 Current platform: {}",
                std::env::consts::OS
            ));
        }

        // Check if Docker daemon is running
        let docker_check = std::process::Command::new("docker")
            .args(&["info"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();

        if docker_check.is_err() || !docker_check.unwrap().success() {
            return Err(
                "Docker daemon is not running. Please start Docker and try again.\n\
                 You can start Docker through Docker Desktop or your system's service manager."
                    .to_string(),
            );
        }

        let image_name = self.get_docker_image_name();
        let container_name = format!("goose-sandbox-{}", std::process::id());

        let mut cmd = Command::new("docker");
        cmd.arg("run")
            .arg("--rm") // Remove container when done
            .arg("--name")
            .arg(&container_name)
            .arg("--workdir")
            .arg("/workspace") // Set working directory inside container
            .arg("--volume")
            .arg(format!("{}:/workspace", self.project_dir.display())) // Mount project directory
            .arg("--volume")
            .arg(format!("{}:/home/goose", self.home_dir.display())) // Mount home directory (read-only for configs)
            .arg("--network")
            .arg(self.get_docker_network_mode()) // Control network access
            .arg("--user")
            .arg(self.get_docker_user()) // Run as appropriate user
            .arg("--security-opt")
            .arg("no-new-privileges") // Security hardening
            .arg(&image_name)
            .arg(&shell_config.executable)
            .args(&shell_config.args)
            .arg(command);

        Ok(cmd)
    }

    fn create_podman_command(
        &self,
        shell_config: &ShellConfig,
        command: &str,
    ) -> Result<Command, String> {
        // Check if Podman is available
        if which::which("podman").is_err() {
            return Err(format!(
                "Podman not found. Podman sandboxing requires Podman to be installed and available in PATH.\n\
                 Install Podman from https://podman.io and ensure 'podman' command is available.\n\
                 Current platform: {}",
                std::env::consts::OS
            ));
        }

        let image_name = self.get_docker_image_name(); // Reuse Docker image
        let container_name = format!("goose-sandbox-{}", std::process::id());

        let mut cmd = Command::new("podman");
        cmd.arg("run")
            .arg("--rm") // Remove container when done
            .arg("--name")
            .arg(&container_name)
            .arg("--workdir")
            .arg("/workspace") // Set working directory inside container
            .arg("--volume")
            .arg(format!("{}:/workspace:Z", self.project_dir.display())) // Mount project directory with SELinux label
            .arg("--volume")
            .arg(format!("{}:/home/goose:ro,Z", self.home_dir.display())) // Mount home directory read-only
            .arg("--network")
            .arg(self.get_docker_network_mode()) // Control network access
            .arg("--user")
            .arg(self.get_docker_user()) // Run as appropriate user
            .arg("--security-opt")
            .arg("no-new-privileges") // Security hardening
            .arg(&image_name)
            .arg(&shell_config.executable)
            .args(&shell_config.args)
            .arg(command);

        Ok(cmd)
    }

    fn get_docker_image_name(&self) -> String {
        // Check environment variable first, then auto-detect or use default
        env::var("GOOSE_SANDBOX_IMAGE").unwrap_or_else(|_| {
            // Try to detect project type and use appropriate image
            self.detect_project_image().unwrap_or_else(|| {
                // Default to Ubuntu with common development tools
                "ubuntu:22.04".to_string()
            })
        })
    }
    
    fn detect_project_image(&self) -> Option<String> {
        // Check for common project files to determine the best image
        
        // Rust project
        if self.project_dir.join("Cargo.toml").exists() {
            return Some("goose/rust-sandbox:latest".to_string());
        }
        
        // Node.js project
        if self.project_dir.join("package.json").exists() {
            return Some("goose/node-sandbox:latest".to_string());
        }
        
        // Python project
        if self.project_dir.join("requirements.txt").exists() 
            || self.project_dir.join("pyproject.toml").exists()
            || self.project_dir.join("setup.py").exists() {
            return Some("goose/python-sandbox:latest".to_string());
        }
        
        // Go project
        if self.project_dir.join("go.mod").exists() {
            return Some("golang:1.21-bullseye".to_string());
        }
        
        // Java project
        if self.project_dir.join("pom.xml").exists() 
            || self.project_dir.join("build.gradle").exists() {
            return Some("openjdk:11-jdk-slim".to_string());
        }
        
        None
    }

    fn get_docker_network_mode(&self) -> String {
        match &self.config.profile {
            SeatbeltProfile::PermissiveOpen | SeatbeltProfile::RestrictiveOpen => "bridge".to_string(),
            SeatbeltProfile::PermissiveClosed | SeatbeltProfile::RestrictiveClosed => "none".to_string(),
        }
    }

    fn get_docker_user(&self) -> String {
        // Use host UID:GID if available, otherwise run as container user
        if let Ok(uid) = env::var("SANDBOX_UID") {
            if let Ok(gid) = env::var("SANDBOX_GID") {
                return format!("{}:{}", uid, gid);
            }
        }

        // Try to get current user ID on Unix systems
        #[cfg(unix)]
        {
            let uid = unsafe { libc::getuid() };
            let gid = unsafe { libc::getgid() };
            format!("{}:{}", uid, gid)
        }

        #[cfg(not(unix))]
        {
            // On Windows, run as container user (typically root)
            "1000:1000".to_string() // Default user in most Linux containers
        }
    }

    pub fn is_sandboxing_available(&self) -> bool {
        match self.config.method {
            SandboxMethod::None => true,
            SandboxMethod::Seatbelt => {
                cfg!(target_os = "macos") && which::which("sandbox-exec").is_ok()
            }
            SandboxMethod::Docker => which::which("docker").is_ok(),
            SandboxMethod::Podman => which::which("podman").is_ok(),
        }
    }

    pub fn get_status_info(&self) -> String {
        if !self.config.enabled {
            return "Sandboxing disabled".to_string();
        }

        match self.config.method {
            SandboxMethod::None => "Sandboxing disabled".to_string(),
            SandboxMethod::Seatbelt => {
                if self.is_sandboxing_available() {
                    format!(
                        "Seatbelt sandboxing enabled (profile: {:?})",
                        self.config.profile
                    )
                } else {
                    "Seatbelt sandboxing not available on this system".to_string()
                }
            }
            SandboxMethod::Docker => {
                if self.is_sandboxing_available() {
                    "Docker sandboxing enabled (not implemented)".to_string()
                } else {
                    "Docker not available".to_string()
                }
            }
            SandboxMethod::Podman => {
                if self.is_sandboxing_available() {
                    "Podman sandboxing enabled (not implemented)".to_string()
                } else {
                    "Podman not available".to_string()
                }
            }
        }
    }
}

/// Parse sandbox configuration from environment variables and CLI arguments
pub fn parse_sandbox_config_from_env() -> SandboxConfig {
    parse_sandbox_config(None, None)
}

/// Parse sandbox configuration from CLI arguments and environment variables
pub fn parse_sandbox_config(
    sandbox_arg: Option<Option<String>>,
    profile_arg: Option<String>,
) -> SandboxConfig {
    let mut config = SandboxConfig::default();

    // Check environment variables first
    if let Ok(sandbox_enabled) = env::var("GOOSE_SANDBOX") {
        if let Ok(method) = SandboxMethod::from_str(&sandbox_enabled) {
            config.enabled = method != SandboxMethod::None;
            config.method = method;
        } else if sandbox_enabled.to_lowercase() == "true" {
            config.enabled = true;
            // Use platform-appropriate default
            config.method = if cfg!(target_os = "macos") {
                SandboxMethod::Seatbelt
            } else {
                SandboxMethod::Docker // Use Docker as default on Linux/Windows
            };
        }
    }

    // Check for seatbelt profile override from env
    if let Ok(profile_str) = env::var("SEATBELT_PROFILE") {
        if let Ok(profile) = SeatbeltProfile::from_str(&profile_str) {
            config.profile = profile;
        }
    }

    // CLI arguments override environment variables
    if let Some(sandbox_opt) = sandbox_arg {
        match sandbox_opt {
            Some(method_str) => {
                // Specific method provided
                if let Ok(method) = SandboxMethod::from_str(&method_str) {
                    config.enabled = method != SandboxMethod::None;
                    config.method = method;
                }
            }
            None => {
                // Flag provided without value, enable with platform default
                config.enabled = true;
                config.method = if cfg!(target_os = "macos") {
                    SandboxMethod::Seatbelt
                } else {
                    SandboxMethod::Docker // Use Docker as default on Linux/Windows
                };
            }
        }
    }

    // CLI profile argument overrides environment
    if let Some(profile_str) = profile_arg {
        if let Ok(profile) = SeatbeltProfile::from_str(&profile_str) {
            config.profile = profile;
        }
    }

    config
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sandbox_method_from_str() {
        assert_eq!(
            SandboxMethod::from_str("seatbelt").unwrap(),
            SandboxMethod::Seatbelt
        );
        assert_eq!(
            SandboxMethod::from_str("docker").unwrap(),
            SandboxMethod::Docker
        );
        assert_eq!(
            SandboxMethod::from_str("none").unwrap(),
            SandboxMethod::None
        );
        assert_eq!(
            SandboxMethod::from_str("false").unwrap(),
            SandboxMethod::None
        );
        assert!(SandboxMethod::from_str("invalid").is_err());
    }

    #[test]
    fn test_seatbelt_profile_from_str() {
        assert_eq!(
            SeatbeltProfile::from_str("permissive-open").unwrap(),
            SeatbeltProfile::PermissiveOpen
        );
        assert_eq!(
            SeatbeltProfile::from_str("restrictive_closed").unwrap(),
            SeatbeltProfile::RestrictiveClosed
        );
        assert!(SeatbeltProfile::from_str("invalid").is_err());
    }

    #[test]
    fn test_default_sandbox_config() {
        let config = SandboxConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.method, SandboxMethod::None);
        assert_eq!(config.profile, SeatbeltProfile::PermissiveOpen);
    }
}
