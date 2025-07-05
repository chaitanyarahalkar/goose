---
sidebar_position: 9
title: Sandboxing in Goose
sidebar_label: Sandboxing
---

Sandboxing isolates potentially dangerous operations (such as shell commands or file modifications) from your host system, providing a security barrier between AI operations and your environment.

## Prerequisites

Before using sandboxing, ensure you have the latest Goose CLI installed:

```bash
# Install goose-cli – macOS & Linux
curl -fsSL https://github.com/block/goose/releases/download/stable/download_cli.sh | bash

# Verify installation
goose --version
```

:::info macOS Only
**Platform Support**: Sandboxing is currently only available on macOS using the built-in Seatbelt system. 

- ✅ **macOS**: Full support via Seatbelt profiles
- ❌ **Linux/Windows**: Not yet supported (Docker/Podman planned for future releases)

If you try to enable sandboxing on Linux or Windows, you'll receive a clear error message with instructions to disable sandboxing.
:::

## Overview of Sandboxing

The benefits of sandboxing include:

- **Security**: Prevent accidental system damage or data loss
- **Isolation**: Limit file system access to project directory and approved paths
- **Consistency**: Ensure reproducible environments across different systems
- **Safety**: Reduce risk when working with untrusted code or experimental commands

### macOS Seatbelt

Goose uses macOS's built-in `sandbox-exec` command (Seatbelt) to create a secure execution environment. This is lightweight and requires no additional software installation.

## Quickstart

### Enable Sandboxing with Command Flags

```bash
# Enable sandboxing with default profile
goose run --sandbox -t "analyze the code structure"

# Specify seatbelt method explicitly  
goose run --sandbox=seatbelt -t "run the test suite"

# Use a specific security profile
goose run --sandbox --sandbox-profile=restrictive-closed -t "cargo build"
```

### Enable Sandboxing with Environment Variables

```bash
# Set environment variables
export GOOSE_SANDBOX=seatbelt
export SEATBELT_PROFILE=permissive-closed

# Run normally - sandboxing will be applied automatically
goose run -t "npm test"
```

## Configuration

### Configuration Hierarchy

Settings are applied in order of precedence (highest → lowest):

1. **Command flag**: `--sandbox[=METHOD]` and `--sandbox-profile=PROFILE`
2. **Environment variable**: `GOOSE_SANDBOX` and `SEATBELT_PROFILE`
3. **Default**: No sandboxing

### macOS Seatbelt Profiles

Goose includes four built-in Seatbelt profiles with different security levels:

| Profile | Network Access | File System Access | Use Case |
|---------|----------------|-------------------|----------|
| `permissive-open` | ✅ Allowed | Read anywhere, write to project only | **Default** - Development work with network access |
| `permissive-closed` | ❌ Blocked | Read anywhere, write to project only | Development work without network access |
| `restrictive-open` | ✅ Allowed | Minimal system access, write to project only | Stricter security with network |
| `restrictive-closed` | ❌ Blocked | Minimal system access, write to project only | **Maximum security** - Isolated execution |

#### Profile Examples

```bash
# Default profile (permissive-open) - allows network, restricts writes
goose run --sandbox -t "curl -I google.com && touch myfile.txt"

# Block network access but allow system file reading
goose run --sandbox --sandbox-profile=permissive-closed -t "ls /usr/bin | head -5"

# Maximum restrictions - no network, minimal file access
goose run --sandbox --sandbox-profile=restrictive-closed -t "echo 'Hello World'"
```

## Usage Examples

### Basic Development Workflow

```bash
# Enable sandboxing for a development session
goose session --sandbox --name secure-dev

# Run sandboxed commands
goose run --sandbox -t "make build && make test"

# Network-isolated testing
goose run --sandbox --sandbox-profile=permissive-closed -t "run offline tests"
```

### Environment Variable Configuration

```bash
# Configure in your shell profile (~/.zshrc, ~/.bashrc)
export GOOSE_SANDBOX=seatbelt
export SEATBELT_PROFILE=permissive-open

# Now all goose commands will be sandboxed
goose run -t "analyze this codebase"
goose session --name my-session
```

### Comparing Sandboxed vs Non-Sandboxed Execution

```bash
# Without sandbox - full system access
goose run -t "ping -c 1 google.com" --no-session

# With network-blocking sandbox - should fail
goose run --sandbox --sandbox-profile=restrictive-closed -t "ping -c 1 google.com" --no-session
```

## Troubleshooting

### Common Issues

**"Operation not permitted" errors**
- The operation requires access blocked by the current profile
- Try switching to a more permissive profile:
  ```bash
  goose run --sandbox --sandbox-profile=permissive-open -t "your command"
  ```

**Network commands fail unexpectedly**
- Check if you're using a `-closed` profile that blocks network access
- Switch to an `-open` profile for network access:
  ```bash
  goose run --sandbox --sandbox-profile=restrictive-open -t "curl example.com"
  ```

**Sandbox not activating**
- Verify you're on macOS (sandboxing currently requires macOS)
- Check that `sandbox-exec` is available:
  ```bash
  which sandbox-exec
  ```

**"Sandboxing is not yet supported on linux/windows" errors**
- Sandboxing is currently macOS-only
- To disable sandboxing and continue:
  ```bash
  # Remove CLI flag
  goose run -t "your command" --no-session
  
  # Or unset environment variable
  unset GOOSE_SANDBOX
  goose run -t "your command" --no-session
  ```

### Debug Mode

Enable verbose output to troubleshoot sandbox issues:

```bash
RUST_LOG=debug goose run --sandbox -t "test command" --no-session
```

### Verify Sandbox is Working

Test that the sandbox is actually restricting access:

```bash
# This should work (network allowed)
goose run --sandbox --sandbox-profile=permissive-open -t "ping -c 1 google.com" --no-session

# This should fail with DNS resolution error (network blocked)
goose run --sandbox --sandbox-profile=restrictive-closed -t "ping -c 1 google.com" --no-session
```

## Security Notes

:::warning Important Security Information
- Sandboxing reduces but doesn't eliminate all risks
- Use the most restrictive profile that allows your work
- Never run untrusted commands on production data without additional safeguards
- The sandbox profiles allow reading from most system locations - they primarily restrict writes and network access
:::

## Technical Details

### How It Works

When sandboxing is enabled, Goose wraps shell commands with macOS's `sandbox-exec`:

```bash
# Original command
bash -c "your command"

# Sandboxed command  
sandbox-exec -f profile.sb -D project_dir=/path/to/project -D home_dir=/Users/you bash -c "your command"
```

### Profile Locations

Seatbelt profiles are embedded in the Goose binary and include comprehensive system access rules based on proven configurations from other CLI tools.

## Related Documentation

- [CLI Commands](/docs/guides/goose-cli-commands): Complete CLI reference
- [Tool Permissions](/docs/guides/tool-permissions): Fine-grained permission control
- [Running Tasks](/docs/guides/running-tasks): Task execution patterns
- [Environment Variables](/docs/guides/environment-variables): Configuration options