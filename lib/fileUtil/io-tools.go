package fileUtil

import (
	"bufio"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"strings"
	"vmate/lib/vpn"
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

func OpenText() []vpn.VPN {
	vpns := []vpn.VPN{}
	file, err := os.Open("recent.txt")
	if err != nil {
		fmt.Println("You don't have any previous saved configs!")
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
	return vpns
}
