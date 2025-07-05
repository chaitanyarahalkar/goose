use std::env;
use std::path::PathBuf;
use std::str::FromStr;
use serde::{Deserialize, Serialize};
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
        if config.enabled && !cfg!(target_os = "macos") {
            return Err(format!(
                "Sandboxing is not yet supported on {}. Sandboxing is currently only available on macOS using Seatbelt.\n\
                 Support for Docker/Podman on Linux and Windows is planned for future releases.\n\
                 To continue without sandboxing, remove the --sandbox flag or unset GOOSE_SANDBOX environment variable.",
                std::env::consts::OS
            ));
        }

        let project_dir = env::current_dir()
            .map_err(|e| format!("Failed to get current directory: {}", e))?;
        
        let home_dir = dirs::home_dir()
            .ok_or_else(|| "Failed to get home directory".to_string())?;

        Ok(Self {
            config,
            project_dir,
            home_dir,
        })
    }

    pub fn wrap_command(&self, shell_config: &ShellConfig, command: &str) -> Result<Command, String> {
        if !self.config.enabled || self.config.method == SandboxMethod::None {
            return Ok(self.create_direct_command(shell_config, command));
        }

        match self.config.method {
            SandboxMethod::Seatbelt => self.create_seatbelt_command(shell_config, command),
            SandboxMethod::None => Ok(self.create_direct_command(shell_config, command)),
            SandboxMethod::Docker => {
                Err(format!(
                    "Docker sandboxing is not yet implemented. Available options:\n\
                     - On macOS: Use --sandbox=seatbelt\n\
                     - To disable: Remove --sandbox flag or unset GOOSE_SANDBOX\n\
                     Current platform: {}",
                    std::env::consts::OS
                ))
            }
            SandboxMethod::Podman => {
                Err(format!(
                    "Podman sandboxing is not yet implemented. Available options:\n\
                     - On macOS: Use --sandbox=seatbelt\n\
                     - To disable: Remove --sandbox flag or unset GOOSE_SANDBOX\n\
                     Current platform: {}",
                    std::env::consts::OS
                ))
            }
        }
    }

    fn create_direct_command(&self, shell_config: &ShellConfig, command: &str) -> Command {
        let mut cmd = Command::new(&shell_config.executable);
        cmd.args(&shell_config.args);
        cmd.arg(command);
        cmd
    }

    fn create_seatbelt_command(&self, shell_config: &ShellConfig, command: &str) -> Result<Command, String> {
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
        let mut profile_path = env::current_dir()
            .map_err(|e| format!("Failed to get current directory: {}", e))?;
        
        profile_path.push("crates");
        profile_path.push("goose-mcp");
        profile_path.push("src");
        profile_path.push("developer");
        profile_path.push("profiles");
        profile_path.push(profile_filename);

        if !profile_path.exists() {
            return Err(format!("Seatbelt profile not found: {}", profile_path.display()));
        }

        Ok(profile_path)
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
                    format!("Seatbelt sandboxing enabled (profile: {:?})", self.config.profile)
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
            // Use platform-appropriate default (only seatbelt is implemented)
            if cfg!(target_os = "macos") {
                config.method = SandboxMethod::Seatbelt;
            } else {
                // On non-macOS, we don't have a working sandbox method yet
                // This will result in an error when trying to create the sandbox wrapper
                config.method = SandboxMethod::None;
                config.enabled = false;
            }
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
                if cfg!(target_os = "macos") {
                    config.enabled = true;
                    config.method = SandboxMethod::Seatbelt;
                } else {
                    // On non-macOS platforms, sandboxing isn't available yet
                    config.enabled = false;
                    config.method = SandboxMethod::None;
                }
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
        assert_eq!(SandboxMethod::from_str("seatbelt").unwrap(), SandboxMethod::Seatbelt);
        assert_eq!(SandboxMethod::from_str("docker").unwrap(), SandboxMethod::Docker);
        assert_eq!(SandboxMethod::from_str("none").unwrap(), SandboxMethod::None);
        assert_eq!(SandboxMethod::from_str("false").unwrap(), SandboxMethod::None);
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