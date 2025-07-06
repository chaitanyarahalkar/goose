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
    // Traditional Seatbelt profiles (macOS only)
    PermissiveOpen,
    PermissiveClosed,
    RestrictiveOpen,
    RestrictiveClosed,
    Custom(PathBuf),
    // Simplified container profiles (Docker/Podman only)
    Permissive,   // Network allowed
    Restrictive,  // Network blocked
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
            // Traditional Seatbelt profiles
            "permissive_open" | "permissiveopen" => Ok(SeatbeltProfile::PermissiveOpen),
            "permissive_closed" | "permissiveclosed" => Ok(SeatbeltProfile::PermissiveClosed),
            "restrictive_open" | "restrictiveopen" => Ok(SeatbeltProfile::RestrictiveOpen),
            "restrictive_closed" | "restrictiveclosed" => Ok(SeatbeltProfile::RestrictiveClosed),
            // Simplified container profiles
            "permissive" => Ok(SeatbeltProfile::Permissive),
            "restrictive" => Ok(SeatbeltProfile::Restrictive),
            _ => {
                // Check if it's a file path
                let path = PathBuf::from(s);
                if path.exists() {
                    // Validate it's a .sb file
                    if let Some(extension) = path.extension() {
                        if extension == "sb" {
                            return Ok(SeatbeltProfile::Custom(path));
                        } else {
                            return Err(format!(
                                "Custom profile file must have .sb extension, got: {}", 
                                extension.to_string_lossy()
                            ));
                        }
                    } else {
                        return Err("Custom profile file must have .sb extension".to_string());
                    }
                } else {
                    return Err(format!(
                        "Unknown sandbox profile '{}'. Available profiles:\n  Seatbelt (macOS): permissive-open, permissive-closed, restrictive-open, restrictive-closed\n  Container (Docker/Podman): permissive, restrictive\n  Custom: path to existing .sb file", 
                        s
                    ));
                }
            }
        }
    }
}

impl SeatbeltProfile {
    fn profile_filename(&self) -> Option<&'static str> {
        match self {
            SeatbeltProfile::PermissiveOpen => Some("permissive-open.sb"),
            SeatbeltProfile::PermissiveClosed => Some("permissive-closed.sb"),
            SeatbeltProfile::RestrictiveOpen => Some("restrictive-open.sb"),
            SeatbeltProfile::RestrictiveClosed => Some("restrictive-closed.sb"),
            SeatbeltProfile::Custom(_) => None, // Custom profiles use their own paths
            SeatbeltProfile::Permissive | SeatbeltProfile::Restrictive => None, // Container profiles don't use .sb files
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
                SandboxMethod::Docker | SandboxMethod::Podman => {
                    // Check if invalid profiles are being used with Docker/Podman
                    match config.profile {
                        SeatbeltProfile::Custom(_) => {
                            return Err(format!(
                                "Custom .sb profile files are only supported with Seatbelt sandboxing on macOS.\n\
                                 Docker and Podman sandboxing use container profiles only.\n\
                                 Available options:\n\
                                 - Use --sandbox=seatbelt with your custom profile (macOS only)\n\
                                 - Use container profiles: permissive, restrictive"
                            ));
                        }
                        SeatbeltProfile::PermissiveOpen | SeatbeltProfile::PermissiveClosed | 
                        SeatbeltProfile::RestrictiveOpen | SeatbeltProfile::RestrictiveClosed => {
                            return Err(format!(
                                "Traditional Seatbelt profiles are only supported with Seatbelt sandboxing on macOS.\n\
                                 Docker and Podman sandboxing use simplified container profiles.\n\
                                 Available options:\n\
                                 - Use --sandbox=seatbelt with Seatbelt profiles: permissive-open, permissive-closed, restrictive-open, restrictive-closed\n\
                                 - Use container profiles: permissive (network allowed), restrictive (network blocked)"
                            ));
                        }
                        SeatbeltProfile::Permissive | SeatbeltProfile::Restrictive => {
                            // These are the correct profiles for Docker/Podman
                        }
                    }
                }
                SandboxMethod::Seatbelt => {
                    // Check if container-specific profiles are being used with Seatbelt
                    if matches!(config.profile, SeatbeltProfile::Permissive | SeatbeltProfile::Restrictive) {
                        return Err(format!(
                            "Container profiles 'permissive' and 'restrictive' are only supported with Docker/Podman sandboxing.\n\
                             Seatbelt sandboxing uses different profiles.\n\
                             Available options:\n\
                             - Use --sandbox=docker or --sandbox=podman with container profiles: permissive, restrictive\n\
                             - Use Seatbelt profiles: permissive-open, permissive-closed, restrictive-open, restrictive-closed"
                        ));
                    }
                }
                _ => {} // Other methods are fine
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
        match &self.config.profile {
            SeatbeltProfile::Custom(custom_path) => {
                // Validate the custom profile file
                self.validate_custom_profile(custom_path)?;
                Ok(custom_path.clone())
            }
            _ => {
                // Get the path to the built-in profile file
                let profile_filename = self.config.profile.profile_filename()
                    .ok_or("Internal error: built-in profile should have filename")?;

                // For now, use profiles from the source directory
                // In a production build, these would be embedded as resources
                let mut profile_path = env::current_dir()
                    .map_err(|e| format!("Failed to get current directory: {}", e))?;

                profile_path.push("crates");
                profile_path.push("goose-mcp");
                profile_path.push("src");
                profile_path.push("developer");
                profile_path.push("profiles");
                profile_path.push(profile_filename);

                if !profile_path.exists() {
                    return Err(format!(
                        "Built-in seatbelt profile not found: {}",
                        profile_path.display()
                    ));
                }

                Ok(profile_path)
            }
        }
    }

    fn validate_custom_profile(&self, profile_path: &PathBuf) -> Result<(), String> {
        // Check if file exists
        if !profile_path.exists() {
            return Err(format!(
                "Custom seatbelt profile file not found: {}",
                profile_path.display()
            ));
        }

        // Check if it's a file (not a directory)
        if !profile_path.is_file() {
            return Err(format!(
                "Custom seatbelt profile path is not a file: {}",
                profile_path.display()
            ));
        }

        // Check file extension
        match profile_path.extension() {
            Some(ext) if ext == "sb" => {},
            Some(ext) => {
                return Err(format!(
                    "Custom seatbelt profile must have .sb extension, got: {}",
                    ext.to_string_lossy()
                ));
            }
            None => {
                return Err("Custom seatbelt profile must have .sb extension".to_string());
            }
        }

        // Try to read the file to ensure it's accessible
        match std::fs::read_to_string(profile_path) {
            Ok(content) => {
                // Basic validation: ensure it's not empty and looks like a seatbelt profile
                if content.trim().is_empty() {
                    return Err("Custom seatbelt profile file is empty".to_string());
                }
                
                // Check for basic seatbelt syntax (should contain at least one rule)
                if !content.contains("(") || !content.contains(")") {
                    return Err(format!(
                        "Custom seatbelt profile file '{}' does not appear to contain valid seatbelt syntax",
                        profile_path.display()
                    ));
                }
            }
            Err(e) => {
                return Err(format!(
                    "Failed to read custom seatbelt profile file '{}': {}",
                    profile_path.display(),
                    e
                ));
            }
        }

        Ok(())
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
            // Traditional Seatbelt profiles (shouldn't be used with Docker, but handle gracefully)
            SeatbeltProfile::PermissiveOpen | SeatbeltProfile::RestrictiveOpen => "bridge".to_string(),
            SeatbeltProfile::PermissiveClosed | SeatbeltProfile::RestrictiveClosed => "none".to_string(),
            SeatbeltProfile::Custom(_) => {
                // Custom profiles are only for macOS Seatbelt, not Docker
                // If somehow we get here with Docker, default to no network for security
                "none".to_string()
            }
            // Container-specific profiles (primary profiles for Docker/Podman)
            SeatbeltProfile::Permissive => "bridge".to_string(),   // Network allowed
            SeatbeltProfile::Restrictive => "none".to_string(),    // Network blocked
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
                    match &self.config.profile {
                        SeatbeltProfile::Custom(path) => {
                            format!(
                                "Seatbelt sandboxing enabled (custom profile: {})",
                                path.display()
                            )
                        }
                        _ => {
                            format!(
                                "Seatbelt sandboxing enabled (profile: {:?})",
                                self.config.profile
                            )
                        }
                    }
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
pub fn parse_sandbox_config_from_env() -> Result<SandboxConfig, String> {
    parse_sandbox_config(None, None)
}

/// Parse sandbox configuration from CLI arguments and environment variables
pub fn parse_sandbox_config(
    sandbox_arg: Option<Option<String>>,
    profile_arg: Option<String>,
) -> Result<SandboxConfig, String> {
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
        match SeatbeltProfile::from_str(&profile_str) {
            Ok(profile) => config.profile = profile,
            Err(e) => return Err(format!("Invalid SEATBELT_PROFILE environment variable: {}", e)),
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
        match SeatbeltProfile::from_str(&profile_str) {
            Ok(profile) => config.profile = profile,
            Err(e) => return Err(format!("Invalid --sandbox-profile argument: {}", e)),
        }
    }

    Ok(config)
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
        
        // Test that non-existent file path returns error
        assert!(SeatbeltProfile::from_str("/nonexistent/path.sb").is_err());
        
        // Test that file without .sb extension returns error
        assert!(SeatbeltProfile::from_str("/tmp/test.txt").is_err());
    }

    #[test]
    fn test_default_sandbox_config() {
        let config = SandboxConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.method, SandboxMethod::None);
        assert_eq!(config.profile, SeatbeltProfile::PermissiveOpen);
    }
}
