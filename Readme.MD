# Vmate ⚡

> **Note:** Vmate is currently optimized for configurations sourced from [VPN Gate](https://www.vpngate.net/).

[![zread](https://img.shields.io/badge/Ask_Zread-_.svg?style=flat&color=00b0aa&labelColor=000000&logo=data%3Aimage%2Fsvg%2Bxml%3Bbase64%2CPHN2ZyB3aWR0aD0iMTYiIGhlaWdodD0iMTYiIHZpZXdCb3g9IjAgMCAxNiAxNiIgZmlsbD0ibm9uZSIgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIj4KPHBhdGggZD0iTTQuOTYxNTYgMS42MDAxSDIuMjQxNTZDMS44ODgxIDEuNjAwMSAxLjYwMTU2IDEuODg2NjQgMS42MDE1NiAyLjI0MDFWNC45NjAxQzEuNjAxNTYgNS4zMTM1NiAxLjg4ODEgNS42MDAxIDIuMjQxNTYgNS42MDAxSDQuOTYxNTZDNS4zMTUwMiA1LjYwMDEgNS42MDE1NiA1LjMxMzU2IDUuNjAxNTYgNC45NjAxVjIuMjQwMUM1LjYwMTU2IDEuODg2NjQgNS4zMTUwMiAxLjYwMDEgNC45NjE1NiAxLjYwMDFaIiBmaWxsPSIjZmZmIi8%2BCjxwYXRoIGQ9Ik00Ljk2MTU2IDEwLjM5OTlIMi4yNDE1NkMxLjg4ODEgMTAuMzk5OSAxLjYwMTU2IDEwLjY4NjQgMS42MDE1NiAxMS4wMzk5VjEzLjc1OTlDMS42MDE1NiAxNC4xMTM0IDEuODg4MSAxNC4zOTk5IDIuMjQxNTYgMTQuMzk5OUg0Ljk2MTU2QzUuMzE1MDIgMTQuMzk5OSA1LjYwMTU2IDE0LjExMzQgNS42MDE1NiAxMy43NTk5VjExLjAzOTlDNS42MDE1NiAxMC42ODY0IDUuMzE1MDIgMTAuMzk5OSA0Ljk2MTU2IDEwLjM5OTlaIiBmaWxsPSIjZmZmIi8%2BCjxwYXRoIGQ9Ik0xMy43NTg0IDEuNjAwMUgxMS4wMzg0QzEwLjY4NSAxLjYwMDEgMTAuMzk4NCAxLjg4NjY0IDEwLjM5ODQgMi4yNDAxVjQuOTYwMUMxMC4zOTg0IDUuMzEzNTYgMTAuNjg1IDUuNjAwMSAxMS4wMzg0IDUuNjAwMUgxMy43NTg0QzE0LjExMTkgNS42MDAxIDE0LjM5ODQgNS4zMTM1NiAxNC4zOTg0IDQuOTYwMVYyLjI0MDFDMTQuMzk4NCAxLjg4NjY0IDE0LjExMTkgMS42MDAxIDEzLjc1ODQgMS42MDAxWiIgZmlsbD0iI2ZmZiIvPgo8cGF0aCBkPSJNNCAxMkwxMiA0TDQgMTJaIiBmaWxsPSIjZmZmIi8%2BCjxwYXRoIGQ9Ik00IDEyTDEyIDQiIHN0cm9rZT0iI2ZmZiIgc3Ryb2tlLXdpZHRoPSIxLjUiIHN0cm9rZS1saW5lY2FwPSJyb3VuZCIvPgo8L3N2Zz4K&logoColor=ffffff)](https://zread.ai/codewiththiha/vmate-cli)


**Vmate** is a fast, concurrent OpenVPN configuration validator that automatically identifies working `.ovpn` files from your specified directories. Instead of manually testing each file with `sudo openvpn --config`, Vmate automates the process, saving you hours of tedious work.

## 🚀 Performance

This is a complete rewrite of the [original Java version](https://github.com/codewiththiha/vpn_automation_project) in Go, leveraging goroutines for massive concurrency improvements:

| Version | Processing Time (250 configs) | Speed Improvement |
| --- | --- | --- |
| Java | ~40 minutes | Baseline |
| Go | **~10 seconds** | **240x faster** |

*Note: Performance scales with system resources, particularly RAM.*

---

## ✨ Features

* 🔍 **Automatic Discovery**: Scans specified directories recursively for `.ovpn` files.
* ⚡ **Concurrent Testing**: Uses goroutines to test multiple configs simultaneously.
* ⏱️ **Configurable Timeout**: Set custom timeouts for each connection test.
* 🎯 **Result Limiting**: Control the number of working configs to find.
* 🔧 **Verbose Mode**: Detailed output for debugging purposes.
* 📊 **Worker Pool Management**: Adjust concurrency levels to match your system.
* 🔄 **Smart Connect**: Connect to a specific config with auto-reconnection and failover.
* 📜 **Recent History**: View and reuse successful configs (stored at `~/.config/vmate-cli/recent.txt`).
* ✏️ **Config Modifier**: Automatically fixes outdated cipher settings in `.ovpn` files.
* 📈 **Progress Visualization**: Real-time progress bar showing test status.
* 📦 **Export Successful Configs**: Copy working configs to a directory with country-prefixed filenames.

---

## 📦 Installation

### Prerequisites

* Go 1.19+ (for building from source)
* OpenVPN installed on your system
* Root/sudo access (Vmate will prompt automatically)

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

*Ensure your `GOPATH/bin` is in your system's PATH:*

```bash
export PATH=$PATH:$(go env GOPATH)/bin

```

#### Troubleshooting `go install`

If you encounter caching issues:

1. **Bypass Proxy**: `GOPROXY=direct go install github.com/codewiththiha/vmate-cli@latest`
2. **Install Specific Commit**: `go install github.com/codewiththiha/vmate-cli@<commit-hash>`
3. **Local Install**:
```bash
git clone https://github.com/codewiththiha/vmate-cli.git && cd vmate-cli
go install .

```



---

## 🎯 Usage

> [!CAUTION]
> **IMPORTANT**: Do NOT run Vmate with `sudo` manually. The tool automatically requests root privileges when needed.

```bash
# Basic scan in home directory
vmate-cli

# Scan specific directory with custom limits and concurrency
vmate-cli --dir=/path/to/ovpn/files --limit=50 --timeout=10 --max=20

# Fix outdated cipher settings in all configs
vmate-cli --dir=/path/to/ovpn/files --modify

# Export successful configs to a folder
vmate-cli --dir=~/vpn-configs --limit=20 --export=~/working-configs

# View history
vmate-cli --recent

# Connect to a specific file
vmate-cli --connect=/path/to/config.ovpn

```

### Command Line Flags

| Flag | Description | Default |
| --- | --- | --- |
| `-d, --dir` | Directory containing `.ovpn` files | `~/` |
| `-l, --limit` | Max number of working configs to find | `100` |
| `-m, --max` | Max concurrent worker processes | `200` |
| `-t, --timeout` | Timeout in seconds for each test | `15` |
| `--verbose` | Enable detailed debugging output | - |
| `-r, --recent` | Display recently successful configs | - |
| `--modify` | Fix outdated cipher settings in files | - |
| `-c, --connect` | Connect to a specific config | - |
| `-e, --export` | Path to export successful configs | - |

---

## 🛠️ Configuration & Notes

* **Sudo**: Vmate handles privilege escalation. Avoid running the binary itself with `sudo`.
* **Resources**: Higher `--max` values increase speed but consume more RAM.
* **Config Modifier**: Replaces `cipher AES-128-CBC` with modern `data-ciphers` (AES-256-GCM, etc.) and adds a `#MODIFIED` header to prevent redundant processing.
* **Exporting**: Working configs are renamed with country prefixes (e.g., `JP_vpn-gate.ovpn`) for better organization.

## 🗺️ Roadmap

* [x] Concurrent worker pools
* [x] Smart reconnection management
* [x] Config modifier for outdated ciphers
* [x] Export functionality
* [ ] **GUI version** (Currently in development using [Wails](https://wails.io/))

## 🤝 Contributing

This project is in **beta (v0.0.3)**. Contributions are welcome! Please ensure all CLI flags are documented when adding features.

---

**Showcase**: [Watch the app showcase on YouTube](https://youtu.be/77tMUuJhO_8)
