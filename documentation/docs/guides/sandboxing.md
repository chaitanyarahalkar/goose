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

:::info Cross-Platform Support
**Platform Support**: Sandboxing is available on all platforms with multiple methods:

- ✅ **macOS**: Seatbelt (native) + Docker/Podman (containers)
- ✅ **Linux**: Docker/Podman (containers)  
- ✅ **Windows**: Docker/Podman (containers)

The system automatically chooses the best method for your platform when using `--sandbox` without specifying a method.
:::

## Overview of Sandboxing

The benefits of sandboxing include:

- **Security**: Prevent accidental system damage or data loss
- **Isolation**: Limit file system access to project directory and approved paths
- **Consistency**: Ensure reproducible environments across different systems
- **Safety**: Reduce risk when working with untrusted code or experimental commands

### Sandboxing Methods

Goose supports multiple sandboxing approaches:

#### macOS Seatbelt
Uses macOS's built-in `sandbox-exec` command (Seatbelt) to create a secure execution environment. This is lightweight and requires no additional software installation.

#### Docker/Podman Containers  
Cross-platform container-based sandboxing that provides complete process isolation. Automatically detects your project type and uses optimized images with relevant development tools.

## Quickstart

### Enable Sandboxing with Command Flags

```bash
# Enable sandboxing with platform default (Seatbelt on macOS, Docker elsewhere)
goose run --sandbox -t "analyze the code structure"

# Specify method explicitly  
goose run --sandbox=seatbelt -t "run the test suite"    # macOS only
goose run --sandbox=docker -t "npm test"               # All platforms
goose run --sandbox=podman -t "python main.py"         # All platforms

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

### Docker/Podman Configuration

:::warning Profile Behavior in Docker/Podman
**Important**: The four Seatbelt profiles have **limited effect** in Docker/Podman mode:

| Profile | Docker Behavior | Network | File System | Other Restrictions |
|---------|-----------------|---------|-------------|-------------------|
| `permissive-open` | `--network bridge` | ✅ Allowed | Same for all profiles | Same for all profiles |
| `permissive-closed` | `--network none` | ❌ Blocked | Same for all profiles | Same for all profiles |
| `restrictive-open` | `--network bridge` | ✅ Allowed | Same for all profiles | Same for all profiles |
| `restrictive-closed` | `--network none` | ❌ Blocked | Same for all profiles | Same for all profiles |

**Key Points:**
- **Network isolation**: Only `open` vs `closed` matters (`permissive-open` = `restrictive-open`)
- **File system**: All profiles mount the same volumes (project + home directory)
- **Security**: All profiles use identical container security settings

**Effectively, you have 2 Docker profiles:**
- **Network allowed**: `permissive-open` or `restrictive-open` (identical)
- **Network blocked**: `permissive-closed` or `restrictive-closed` (identical)

The full granular control from Seatbelt profiles is **only available on macOS** with `--sandbox=seatbelt`.
:::

#### Automatic Image Detection

Goose automatically detects your project type and uses optimized container images:

| Project Type | Detection | Container Image |
|--------------|-----------|-----------------|
| **Rust** | `Cargo.toml` present | `goose/rust-sandbox:latest` |
| **Node.js** | `package.json` present | `goose/node-sandbox:latest` |
| **Python** | `requirements.txt`, `pyproject.toml`, or `setup.py` | `goose/python-sandbox:latest` |
| **Go** | `go.mod` present | `golang:1.21-bullseye` |
| **Java** | `pom.xml` or `build.gradle` | `openjdk:11-jdk-slim` |
| **Default** | No specific files detected | `ubuntu:22.04` |

#### Environment Variables

Control Docker/Podman behavior with environment variables:

```bash
# Override container image
export GOOSE_SANDBOX_IMAGE=node:18-alpine

# Control user mapping (Linux/macOS)
export SANDBOX_UID=1000
export SANDBOX_GID=1000
```

#### User Mapping and Security

**Important**: Goose automatically runs Docker containers as your host user (not root) for security and file permission consistency.

**Why containers don't run as root:**
- **Security**: Prevents privilege escalation attacks
- **File permissions**: Files created in containers have correct ownership on the host
- **Consistency**: Mounted project files remain accessible with proper permissions

**Comparison:**
```bash
# Manual Docker (runs as root by default)
docker run -it ubuntu:22.04
# → id shows: uid=0(root) gid=0(root)
# → Files created are owned by root, causing permission issues

# Goose sandboxing (runs as your user)
goose run --sandbox=docker -t "id"
# → id shows: uid=501 gid=20 (your host user)
# → Files created have correct ownership
```

**When you need root access:**
```bash
# For package installation or system operations
SANDBOX_UID=0 SANDBOX_GID=0 goose run --sandbox=docker -t "apt-get update"
```

#### Profile Examples

**Seatbelt (macOS only) - Full granular control:**
```bash
# Default profile (permissive-open) - allows network, restricts writes
goose run --sandbox=seatbelt -t "curl -I google.com && touch myfile.txt"

# Block network access but allow system file reading
goose run --sandbox=seatbelt --sandbox-profile=permissive-closed -t "ls /usr/bin | head -5"

# Maximum restrictions - no network, minimal file access
goose run --sandbox=seatbelt --sandbox-profile=restrictive-closed -t "echo 'Hello World'"
```

**Docker/Podman - Network control only:**
```bash
# These are IDENTICAL (both allow network):
goose run --sandbox=docker --sandbox-profile=permissive-open -t "curl -I google.com"
goose run --sandbox=docker --sandbox-profile=restrictive-open -t "curl -I google.com"

# These are IDENTICAL (both block network):
goose run --sandbox=docker --sandbox-profile=permissive-closed -t "echo 'Network blocked'"
goose run --sandbox=docker --sandbox-profile=restrictive-closed -t "echo 'Network blocked'"

# Simplified approach - just use open/closed:
goose run --sandbox=docker --sandbox-profile=permissive-open -t "apt-get update"     # Network allowed
goose run --sandbox=docker --sandbox-profile=permissive-closed -t "echo 'Offline'"  # Network blocked
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
- **For Seatbelt**: Verify you're on macOS and `sandbox-exec` is available:
  ```bash
  which sandbox-exec
  ```
- **For Docker/Podman**: Verify Docker or Podman is installed and running:
  ```bash
  docker --version && docker info
  # OR
  podman --version && podman info
  ```

**Docker/Container Issues**

**"Image not found" errors**
- The required container image doesn't exist locally
- Pull the image manually or use a different one:
  ```bash
  # Pull missing image
  docker pull ubuntu:22.04
  
  # Or override with available image
  GOOSE_SANDBOX_IMAGE=ubuntu:22.04 goose run --sandbox=docker -t "your command"
  ```

**"Docker daemon not running" errors**
- Start Docker Desktop or your system's Docker service
- For Podman, ensure the Podman service is running:
  ```bash
  # Linux
  sudo systemctl start podman
  
  # macOS
  podman machine start
  ```

**Permission errors in containers**
- Container runs as wrong user (especially on Linux)
- **Root cause**: Goose runs containers as your host user, but some operations require root
- **Solutions**:
  ```bash
  # Option 1: Run as root for system operations
  SANDBOX_UID=0 SANDBOX_GID=0 goose run --sandbox=docker -t "apt-get install curl"
  
  # Option 2: Set custom user mapping  
  export SANDBOX_UID=$(id -u)
  export SANDBOX_GID=$(id -g)
  goose run --sandbox=docker -t "your command"
  
  # Option 3: Use pre-built images with tools already installed
  GOOSE_SANDBOX_IMAGE=goose/python-sandbox:latest goose run --sandbox=docker -t "pip install requests"
  ```

### Debug Mode

Enable verbose output to troubleshoot sandbox issues:

```bash
RUST_LOG=debug goose run --sandbox -t "test command" --no-session
```

### Verify Sandbox is Working

Test that the sandbox is actually restricting access:

```bash
# Test 1: Verify containerization (Docker/Podman)
goose run --sandbox=docker -t "cat /etc/os-release | head -2" --no-session
# Should show Linux distribution info, not your host OS

# Test 2: Network restrictions  
# This should work (network allowed)
goose run --sandbox --sandbox-profile=permissive-open -t "curl -I google.com" --no-session

# This should fail (network blocked)  
goose run --sandbox --sandbox-profile=restrictive-closed -t "curl -I google.com || echo 'Network blocked'" --no-session

# Test 3: File system isolation
goose run --sandbox=docker -t "ls /workspace && echo 'Project files accessible'" --no-session
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

When sandboxing is enabled, Goose wraps shell commands with the appropriate sandbox method:

#### Seatbelt (macOS)
```bash
# Original command
bash -c "your command"

# Sandboxed command  
sandbox-exec -f profile.sb -D project_dir=/path/to/project -D home_dir=/Users/you bash -c "your command"
```

#### Docker/Podman  
```bash  
# Original command
bash -c "your command"

# Sandboxed command
docker run --rm --name goose-sandbox-12345 \
  --workdir /workspace \
  --volume /path/to/project:/workspace \
  --volume /home/user:/home/goose \
  --network none \
  --user 1000:1000 \
  --security-opt no-new-privileges \
  ubuntu:22.04 bash -c "your command"
```

### Configuration Files

- **Seatbelt profiles**: Embedded in the Goose binary with comprehensive system access rules
- **Docker images**: Auto-selected based on project type or customizable via environment variables
- **Container configurations**: Automatically configured for security and project access

## Related Documentation

- [CLI Commands](/docs/guides/goose-cli-commands): Complete CLI reference
- [Tool Permissions](/docs/guides/tool-permissions): Fine-grained permission control
- [Running Tasks](/docs/guides/running-tasks): Task execution patterns
- [Environment Variables](/docs/guides/environment-variables): Configuration options