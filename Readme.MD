# Vmate ⚡

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

- 🔍 **Automatic Discovery**: Scans specified directories for .ovpn files
- ⚡ **Concurrent Testing**: Uses goroutines to test multiple configs simultaneously
- ⏱️ **Configurable Timeout**: Set custom timeouts for each connection test
- 🎯 **Result Limiting**: Control how many working configs to find
- 🔧 **Verbose Mode**: Get detailed output for debugging
- 📊 **Worker Pool Management**: Control concurrency levels to match your system
- 📊 **smart connect**: watch and manage the connections smartly

## 📦 Installation

### Prerequisites

- Go 1.19+ (for building from source)
- OpenVPN installed on your system
- sudo access (required for OpenVPN testing)

### Build from Source

```bash
git clone https://github.com/codewiththiha/vmate-cli.git
cd vmate-cli
go build -o vmate-cli main.go
sudo mv vmate-cli /usr/local/bin/
```

## 🎯 Usage

```bash
# Basic usage - scan default home directory
sudo vmate-cli

# Scan specific directory with custom settings
sudo vmate-cli --dir=/path/to/ovpn/files --limit=50 --timeout=10 --max=20

# Get detailed output
sudo vmate-cli --verbose --dir=~/vpn-configs
```

### Command Line Flags

```bash
Flags:
  -d, --dir string      Directory containing .ovpn files (default "~/")
  -h, --help            Help for vmate
  -l, --limit int       Maximum number of working configs to find (default 100)
  -m, --max int         Maximum concurrent worker processes (default 15)
  -t, --timeout int     Timeout in seconds for each connection test (default 15)
      --verbose         Enable detailed output for debugging
```

## 🛠️ Configuration

Vmate automatically respects your system's OpenVPN capabilities. No additional configuration files are needed.

### Important Notes

- Always run with `sudo` - OpenVPN requires elevated privileges(or it will ask itself)
- Adjust `--max` based on your system's RAM (higher = faster but more memory usage)
- Use `--timeout` to balance speed vs. thoroughness (lower = faster, but might miss slow servers)
- Set `--limit` to 0 for unlimited results

## 🗺️ Roadmap

- [x] Core CLI functionality
- [x] Concurrent processing with worker pools
- [ ] **GUI version** - Currently in development using [Wails](https://wails.io/)

## 🤝 Contributing

This project is in beta. Basic functions are stable and usable. Contributions are welcome!

## 💭 Feedback

Found a working config that Vmate missed? Open an issue with the details. Performance not meeting expectations? Let me know your setup!

## 🙏 Acknowledgments

- The Go team for making concurrency approachable
- [Cobra](https://github.com/spf13/cobra) for the excellent CLI framework
- OpenVPN community for maintaining the protocol

---

**⚠️ Beta Notice**: This tool is currently in beta. While core functionality is stable, please report any issues you encounter.
