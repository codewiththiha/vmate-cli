package fileUtil

import (
	"io/fs"
	"path/filepath"
	"strings"
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
