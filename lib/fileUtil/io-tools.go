package fileUtil

import (
	"bufio"
	"bytes"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"strings"
	"vmate/lib/vpn"
)

const (
	OldCipher = "cipher AES-128-CBC"
	NewCipher = "data-ciphers AES-256-GCM:AES-128-GCM:CHACHA20-POLY1305:AES-128-CBC"
)

// will walk through every sub dir and detect the ovpn configs
func GetConfigs(dir string) ([]string, error) {
	configs := []string{}
	err := filepath.WalkDir(dir, func(path string, d fs.DirEntry, err error) error {
		if err != nil {
			if strings.Contains(err.Error(), "permission denied") {
				// you can return nil too but i want to be explict here
				return filepath.SkipDir
			}
			// will return with error if the error is serious
			return err
		}
		if !d.IsDir() && strings.HasSuffix(d.Name(), ".ovpn") {
			configs = append(configs, path)
			return nil
		}

		return nil
	})

	if err != nil {
		return configs, err
	}
	return configs, nil

}

func SaveAsText(lines []vpn.VPN) (bool, error) {
	fileName := "recent.txt"
	file, err := os.Create(fileName)
	if err != nil {
		fmt.Println("Can't create the file")
		return false, err
	}
	defer file.Close()
	for _, line := range lines {
		_, err := file.WriteString(line.Country + ";;" + line.Path + "\n")
		if err != nil {
			fmt.Println("Can't write to the file")
			return false, err
		}
	}
	return true, err
}

func OpenText() ([]vpn.VPN, error) {
	vpns := []vpn.VPN{}
	file, err := os.Open("recent.txt")
	if err != nil {
		fmt.Println("You don't have any previous saved configs!")
		return []vpn.VPN{}, err
	}
	scanner := bufio.NewScanner(file)
	for scanner.Scan() {
		line := strings.TrimSpace(scanner.Text())
		parts := strings.Split(line, ";;")
		vpns = append(vpns, vpn.VPN{
			Country: parts[0],
			Path:    parts[1],
		})

	}
	return vpns, nil
}

// in this function i tried to used scanner but it's make the function a lot more complex than it should be
// so this ReadFile function is much make sense here
func ModifyConfigs(paths []string) {

	for _, dir := range paths {
		content, err := os.ReadFile(dir)
		newLines := []string{}
		modified := false
		if err != nil {
			fmt.Println("Can't read the file")
		}
		if bytes.HasPrefix(content, []byte("#MODIFIED\n")) {
			fmt.Println("Your file", filepath.Base(dir), "is already modified")
			return
		}

		lines := strings.Split(string(content), "\n")

		//// IN MY SYSTEM SOMEHOW THERE'RE BUNCH OF TEMP OVPN FILES WITH NO BYTES IN THERE SO I USED THIS TO REMOVE THEM (use with caution)
		// if len(lines) < 10 {
		// 	os.Remove(dir)
		// }

		for _, line := range lines {
			if strings.Contains(strings.TrimSpace(line), OldCipher) {
				newLines = append(newLines, NewCipher)
				modified = true
			} else {
				newLines = append(newLines, line)
			}
		}

		if modified {
			markMod := "#MODIFIED\n" + strings.Join(newLines, "\n")
			os.WriteFile(dir, []byte(markMod), 0644)
			fmt.Println("Your file", filepath.Base(dir), "is modified")
		}

	}

}
