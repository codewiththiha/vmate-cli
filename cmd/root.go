/*
Copyright © 2025 NAME HERE <EMAIL ADDRESS>
*/
package cmd

import (
	"context"
	"fmt"
	"os"
	"os/exec"
	"strings"
	"time"
	"vmate/lib/fileUtil"
	"vmate/lib/vpn"

	"github.com/spf13/cobra"
)

var (
	dir     string
	limit   int
	verbose bool
)

// rootCmd represents the base command when called without any subcommands
var rootCmd = &cobra.Command{
	Use:   "vmate",
	Short: "VPN config tester",
	Long:  `Test OpenVPN configurations from a directory with timeout and verbose options.`,
	Run: func(cmd *cobra.Command, args []string) {

		// Your original logic goes here, now using flags
		expandedPath, _ := expandPath(dir)
		ensureRootPrivileges(expandedPath)
		ctx, cancel := context.WithTimeout(context.Background(), 16*time.Second)
		fmt.Println(expandedPath)
		paths, err := fileUtil.GetConfigs(expandedPath)
		fmt.Println(len(paths))
		if err != nil {
			fmt.Println("can't get the paths")
		}
		succeedConfigs := vpn.RunTest(paths, ctx, cancel)
		for _, config := range succeedConfigs {
			fmt.Println(config.Path, "--", config.Country)
		}
		fmt.Println(len(succeedConfigs), "/", len(paths))
		defer cancel()

	},
}

// Execute adds all child commands to the root command and sets flags appropriately.
// This is called by main.main(). It only needs to happen once to the rootCmd.
func Execute() {
	err := rootCmd.Execute()
	if err != nil {
		os.Exit(1)
	}
}

func init() {
	// Here you will define your flags and configuration settings.
	// Cobra supports persistent flags, which, if defined here,
	// will be global for your application.

	// rootCmd.PersistentFlags().StringVar(&cfgFile, "config", "", "config file (default is $HOME/.vmate.yaml)")

	// Cobra also supports local flags, which will only run
	// when this action is called directly.
	rootCmd.Flags().BoolP("toggle", "t", false, "Help message for toggle")
	rootCmd.PersistentFlags().StringVarP(&dir, "dir", "d", "~/", "The ovpn files' dir")
	rootCmd.PersistentFlags().IntVarP(&limit, "limit", "l", 100, "Limit the amount of succeed ovpn to find")
	rootCmd.PersistentFlags().BoolVarP(&verbose, "verbose", "", false, "To get more output")

}

// to get the root permission for our app
func ensureRootPrivileges(expandedDir string) {
	if os.Getuid() == 0 {
		return // Already root, nothing to do
	}

	exe, err := os.Executable()
	if err != nil {
		fmt.Println("Error getting executable path")
		os.Exit(1)
	}

	// So basically we extract actual user dir before passing it to this funtion so this expandedDir will be actual user home dir (;D sorry i'm using feyman method here)
	args := []string{exe}
	if expandedDir != "" {
		args = append(args, "--dir", expandedDir)
	}

	cmd := exec.Command("sudo", args...)
	cmd.Stdin = os.Stdin
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr

	err = cmd.Run()
	if err != nil {
		if exitError, ok := err.(*exec.ExitError); ok {
			os.Exit(exitError.ExitCode())
		}
		os.Exit(1)
	}

	// If it get the permission it will exit and will restart back with the permission
	// So bassically Exit(0) mean restructing :D
	os.Exit(0)
}

// this happened before the root permissions so we got actual home , without this we will get root/ which is not what we wanted
func expandPath(path string) (string, error) {
	if strings.HasPrefix(path, "~/") {
		homeDir, err := os.UserHomeDir()
		if err != nil {
			return "", fmt.Errorf("could not get home directory: %w", err)
		}

		return string(homeDir), nil
	}
	return path, nil
}
