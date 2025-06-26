use anyhow::{Context, Result};
use std::env;
use std::process::Command;

/// Check CLI flag or environment variables and, if sandboxing is requested,
/// re-exec the current Goose binary inside the selected sandbox runtime.
///
/// Currently supported runtimes:
///  * macOS seatbelt via `sandbox-exec` (methods: `seatbelt`, `sandbox-exec`, or empty/default)
///
/// The function exits the current process after spawning the sandboxed
/// instance. When already running inside a sandbox (detected through the
/// `GOOSE_SANDBOX_ACTIVE` environment variable) it returns immediately.
pub fn maybe_enter_sandbox(method_flag: Option<String>) -> Result<()> {
    // Do nothing if we're already in a sandboxed context.
    if env::var("GOOSE_SANDBOX_ACTIVE").is_ok() {
        return Ok(());
    }

    // Determine sandbox method from CLI flag (highest precedence) or env var.
    let method = method_flag
        .or_else(|| env::var("GOOSE_SANDBOX").ok())
        .unwrap_or_default();

    // If the user did not ask for sandboxing, simply continue.
    if method.is_empty() {
        return Ok(());
    }

    match method.as_str() {
        // Seatbelt (macOS sandbox-exec) implementation
        "seatbelt" | "sandbox-exec" | "true" | "1" => enter_seatbelt_sandbox(),
        // Stub for Docker sandboxing – not yet implemented.
        "docker" | "podman" => {
            eprintln!("[goose] Docker/Podman sandboxing is not yet implemented – proceeding without sandbox");
            Ok(())
        }
        other => {
            eprintln!("[goose] Unsupported sandbox method '{other}'. Proceeding without sandbox.");
            Ok(())
        }
    }
}

#[cfg(target_os = "macos")]
fn enter_seatbelt_sandbox() -> Result<()> {
    let profile_env = env::var("SEATBELT_PROFILE").unwrap_or_else(|_| "permissive-open".to_string());
    let exe = env::current_exe().context("failed to determine current executable path")?;

    // Build argument list, removing the sandbox flag/value so we don't recurse forever.
    let mut args: Vec<String> = std::env::args().skip(1).collect();

    let mut i = 0;
    while i < args.len() {
        let is_flag = args[i] == "-s" || args[i] == "--sandbox";
        if is_flag {
            args.remove(i);
            if i < args.len() && !args[i].starts_with('-') { args.remove(i); }
            continue;
        }
        i += 1;
    }

    // Decide whether to use a builtin (-n) or inline (-p) profile.
    let (flag, profile_arg, extra_params): (&str, String, Vec<(String, String)>) = match profile_env.as_str() {
        "permissive-open" => {
            let inline = format!(
                "(version 1)\n(allow default)\n(deny file-write*)\n(allow file-write* (subpath \"{}\"))\n(allow file-write* (subpath \"{}\"))\n(allow file-write* (subpath \"{}\"))\n",
                env::current_dir()?.display(),
                format!("{}/.local/state/goose", env::var("HOME").unwrap_or_default()),
                format!("{}/.local/share/goose", env::var("HOME").unwrap_or_default())
            );
            ("-p", inline, vec![])
        }
        "permissive-closed" => {
            let inline = format!(
                "(version 1)\n(allow default (network*))\n(deny default (with message \"denied\"))\n(allow file-read*)\n(allow file-write* (subpath \"{}\"))\n(allow file-write* (subpath \"{}\"))\n(allow file-write* (subpath \"{}\"))\n(allow process*)\n",
                env::current_dir()?.display(),
                format!("{}/.local/state/goose", env::var("HOME").unwrap_or_default()),
                format!("{}/.local/share/goose", env::var("HOME").unwrap_or_default())
            );
            ("-p", inline, vec![])
        }
        other => ("-n", other.to_string(), vec![]),
    };

    let mut cmd = Command::new("sandbox-exec");
    cmd.arg(flag).arg(&profile_arg);

    for (k, v) in extra_params {
        cmd.arg("-D").arg(format!("{}={}", k, v));
    }

    let status = cmd
        .arg(exe)
        .args(&args)
        .env("GOOSE_SANDBOX_ACTIVE", "1")
        .status()
        .context("failed to launch sandbox-exec")?;

    std::process::exit(status.code().unwrap_or(1));
}

#[cfg(not(target_os = "macos"))]
fn enter_seatbelt_sandbox() -> Result<()> {
    eprintln!("[goose] Seatbelt sandboxing is only supported on macOS – proceeding without sandbox");
    Ok(())
} 