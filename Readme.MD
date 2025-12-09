# Vmate ⚡

## Note: Vmate only optimized well on configs from vpn gate

Fast, concurrent OpenVPN configuration validator that automatically finds working .ovpn files from your specified directory.

> **What it does**: Instead of manually testing each OpenVPN config file with `sudo openvpn --config`, Vmate automates the entire process, saving you hours of tedious work.

## 🚀 Performance

This is a complete rewrite of my [original Java version](https://github.com/codewiththiha/vpn_automation_project) in Go, leveraging goroutines for massive concurrency improvements:

| Version | Processing Time (250 configs) | Speed Improvement |
| ------- | ----------------------------- | ----------------- |
| Java    | ~40 minutes                   | Baseline          |
| Go      | **~10 seconds**               | **240x faster**   |

The more system resources you have (especially RAM), the better the performance scales.

## ✨ Features

- 🔍 **Automatic Discovery**: Scans specified directories recursively for .ovpn files
- ⚡ **Concurrent Testing**: Uses goroutines to test multiple configs simultaneously
- ⏱️ **Configurable Timeout**: Set custom timeouts for each connection test
- 🎯 **Result Limiting**: Control how many working configs to find
- 🔧 **Verbose Mode**: Get detailed output for debugging
- 📊 **Worker Pool Management**: Control concurrency levels to match your system
- 🔄 **Smart Connect**: Connect to a specific config with auto-reconnection and failover
- 📜 **Recent History**: View and reuse previously successful configs
- ✏️ **Config Modifier**: Automatically fix outdated cipher settings in .ovpn files
- 📈 **Progress Visualization**: Real-time progress bar showing test status

## 📦 Installation

### Prerequisites

- Go 1.19+ (for building from source)
- OpenVPN installed on your system
- Root/sudo access (required for OpenVPN testing - Vmate will prompt automatically)

### Build from Source

```bash
git clone https://github.com/codewiththiha/vmate-cli.git
cd vmate-cli
go build -o vmate-cli main.go
sudo mv vmate-cli /usr/local/bin/
```

### Install with Go

```bash
go install github.com/codewiththiha/vmate-cli@latest
```

**Note**: Ensure your `GOPATH/bin` (usually `~/go/bin`) is in your system's PATH:

```bash
export PATH=$PATH:$(go env GOPATH)/bin
```

#### Troubleshooting `go install` Issues

If you encounter version conflicts or caching issues:

**1. Bypass the Go Module Proxy:**

```bash
GOPROXY=direct go install github.com/codewiththiha/vmate-cli@latest
```

**2. Install a Specific Commit:**

```bash
# Get the latest commit hash
git ls-remote https://github.com/codewiththiha/vmate-cli.git HEAD

# Then install using that commit
go install github.com/codewiththiha/vmate-cli@<commit-hash>
```

**3. Install from Local Clone:**

```bash
git clone https://github.com/codewiththiha/vmate-cli.git
cd vmate-cli
go install .
sudo mv $(go env GOPATH)/bin/vmate-cli /usr/local/bin/
```

**Why these issues happen**: The Go module proxy caches versions for performance, and it can take 30-60 minutes to recognize new commits.

## 🎯 Usage

⚠️ **IMPORTANT**: Do NOT run Vmate with `sudo` manually. The tool automatically requests root privileges when needed.

```bash
# Basic usage - scan default home directory
vmate-cli

# Scan specific directory with custom settings
vmate-cli --dir=/path/to/ovpn/files --limit=50 --timeout=10 --max=20

# Get detailed output
vmate-cli --verbose --dir=~/vpn-configs

# View previously successful configs
vmate-cli --recent

# Connect to a specific config with smart failover
vmate-cli --connect=/path/to/config.ovpn

# Fix outdated cipher settings in all configs
vmate-cli --dir=/path/to/ovpn/files --modify
```

### Command Line Flags

```bash
Flags:
  -d, --dir string      Directory containing .ovpn files (default "~/")
  -h, --help            Help for vmate
  -l, --limit int       Maximum number of working configs to find (default 100)
  -m, --max int         Maximum concurrent worker processes (default 200)
  -t, --timeout int     Timeout in seconds for each connection test (default 15)
      --verbose         Enable detailed output for debugging
  -r, --recent          Display recently successful configs from history
      --modify          Fix outdated cipher settings in .ovpn files
  -c, --connect string  Connect to a specific config with smart reconnection
```

### Flag Compatibility Notes

- `--recent` must be used alone (incompatible with other operational flags)
- `--connect` can be combined with `--verbose` but not other flags
- The tool automatically re-executes with sudo if not run as root

## 🛠️ Configuration

Vmate automatically respects your system's OpenVPN capabilities. Configuration history is stored in `recent.txt` in the current directory.

### Important Notes

- **⚠️ DO NOT run with `sudo` manually** - Vmate automatically requests root privileges when needed
- **Adjust `--max` based on your system's RAM** (higher = faster but more memory usage)
- **Use `--timeout` to balance speed vs. thoroughness** (lower = faster, but might miss slow servers)
- **Set `--limit` to 0 for unlimited results**
- **Recent history** is saved after each successful scan and persists across sessions

### Config Modifier

The `--modify` flag automatically updates outdated cipher configurations:

- Replaces `cipher AES-128-CBC` with modern `data-ciphers AES-256-GCM:AES-128-GCM:CHACHA20-POLY1305:AES-128-CBC`
- Marks modified files with `#MODIFIED` header to prevent duplicate changes
- Only processes files that haven't been modified already

## 🗺️ Roadmap

- [x] Core CLI functionality
- [x] Concurrent processing with worker pools
- [x] Smart connection management with auto-reconnection
- [x] Recent history tracking
- [x] Config modifier for outdated ciphers
- [ ] **GUI version** - Currently in development using [Wails](https://wails.io/)

## 🤝 Contributing

This project is in beta (v0.0.2a). Basic functions are stable and usable. Contributions are welcome! Please ensure all CLI flags are documented when adding new features.

## 💭 Feedback

Found a working config that Vmate missed? Open an issue with the details. Performance not meeting expectations? Let me know your setup!

## 🙏 Acknowledgments

- The Go team for making concurrency approachable
- [Cobra](https://github.com/spf13/cobra) for the excellent CLI framework
- OpenVPN community for maintaining the protocol

---

**⚠️ Beta Notice**: This tool is currently in beta (v0.0.2a). While core functionality is stable, please report any issues you encounter.

[![Watch the app showcase video on YouTube](img.youtube.com)](https://youtu.be/77tMUuJhO_8)

