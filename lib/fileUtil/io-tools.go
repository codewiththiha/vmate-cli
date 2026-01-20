package fileUtil

import (
	"bufio"
	"bytes"
	"fmt"
	"io"
	"io/fs"
	"os"
	"path/filepath"
	"strings"

	"github.com/codewiththiha/vmate-cli/lib/vpn"
)

const (
	OldCipher = "cipher AES-128-CBC"
	NewCipher = "data-ciphers AES-256-GCM:AES-128-GCM:CHACHA20-POLY1305:AES-128-CBC"
)

// Standardizes storage to ~/.config/vmate-cli/
func getStoragePath() (string, error) {
	home, err := os.UserHomeDir()
	if err != nil {
		return "", err
	}
	configDir := filepath.Join(home, ".config", "vmate-cli")
	if _, err := os.Stat(configDir); os.IsNotExist(err) {
		if err := os.MkdirAll(configDir, 0755); err != nil {
			return "", err
		}
	}
	return filepath.Join(configDir, "recent.txt"), nil
}

func GetConfigs(dir string) ([]string, error) {
	configs := []string{}
	err := filepath.WalkDir(dir, func(path string, d fs.DirEntry, err error) error {
		if err != nil {
			return filepath.SkipDir
		}
		if !d.IsDir() && strings.HasSuffix(d.Name(), ".ovpn") {
			configs = append(configs, path)
		}
		return nil
	})
	return configs, err
}

func SaveAsText(lines []vpn.VPN) (bool, error) {
	path, err := getStoragePath()
	if err != nil {
		return false, err
	}
	file, err := os.Create(path)
	if err != nil {
		return false, err
	}
	defer file.Close()
	for _, line := range lines {
		_, err := file.WriteString(line.Country + ";;" + line.Path + "\n")
		if err != nil {
			return false, err
		}
	}
	return true, nil
}

func OpenText() ([]vpn.VPN, error) {
	path, err := getStoragePath()
	if err != nil {
		return nil, err
	}
	file, err := os.Open(path)
	if err != nil {
		return []vpn.VPN{}, nil
	}
	defer file.Close()

	vpns := []vpn.VPN{}
	scanner := bufio.NewScanner(file)
	for scanner.Scan() {
		parts := strings.Split(scanner.Text(), ";;")
		if len(parts) >= 2 {
			vpns = append(vpns, vpn.VPN{Country: parts[0], Path: parts[1]})
		}
	}
	return vpns, nil
}

func ExportConfigs(vpns []vpn.VPN, dest string) {
	if dest == "" {
		dest = "."
	}
	absDest, _ := filepath.Abs(dest)
	_ = os.MkdirAll(absDest, 0755)

	fmt.Printf("Exporting %d configs to: %s\n", len(vpns), absDest)
	for _, v := range vpns {
		// Sanitize name: Replace spaces with _ and prepend Country
		originalName := filepath.Base(v.Path)
		sanitizedName := strings.ReplaceAll(originalName, " ", "_")
		newName := fmt.Sprintf("%s_%s", v.Country, sanitizedName)

		sourceFile, err := os.Open(v.Path)
		if err != nil {
			continue
		}

		destFile, err := os.Create(filepath.Join(absDest, newName))
		if err != nil {
			sourceFile.Close()
			continue
		}

		_, err = io.Copy(destFile, sourceFile)
		sourceFile.Close()
		destFile.Close()

		if err == nil {
			fmt.Printf("Exported: %s\n", newName)
		}
	}
}

func ModifyConfigs(paths []string) {
	for _, dir := range paths {
		content, err := os.ReadFile(dir)
		if err != nil || bytes.HasPrefix(content, []byte("#MODIFIED\n")) {
			continue
		}
		lines := strings.Split(string(content), "\n")
		var newLines []string
		modified := false
		for _, line := range lines {
			if strings.Contains(strings.TrimSpace(line), OldCipher) {
				newLines = append(newLines, NewCipher)
				modified = true
			} else {
				newLines = append(newLines, line)
			}
		}
		if modified {
			_ = os.WriteFile(dir, []byte("#MODIFIED\n"+strings.Join(newLines, "\n")), 0644)
		}
	}
}
