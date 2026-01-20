package cmd

import (
	"context"
	"fmt"
	"os"
	"os/exec"
	"os/signal"
	"path/filepath"
	"slices"
	"strconv"
	"strings"
	"syscall"

	"github.com/codewiththiha/vmate-cli/lib/fileUtil"
	"github.com/codewiththiha/vmate-cli/lib/network"
	"github.com/codewiththiha/vmate-cli/lib/vpn"

	"github.com/spf13/cobra"
)

var (
	dir        string
	limit      int
	timeout    int
	maxworkers int
	verbose    bool
	recent     bool
	modify     bool
	connect    string
	exportPath string
)

var rootCmd = &cobra.Command{
	Use:   "vmate",
	Short: "VPN config tester",
	Long:  `Test OpenVPN configurations from a directory with timeout and verbose options.`,
	Run: func(cmd *cobra.Command, args []string) {
		// 1. Handle "Recent" Flag
		if recent {
			vpns, err := fileUtil.OpenText()
			if err != nil {
				return
			}
			fmt.Println("Here're your previously succeed configs")
			fmt.Println("--------------------------------------")
			for _, v := range vpns {
				fmt.Println(v.Path + " -- " + v.Country)
			}
			return
		}

		// 2. Setup Context for Signal Handling (Ctrl+C)
		ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
		defer stop()

		// Global safety net: Force kill openvpn on exit
		defer exec.Command("killall", "openvpn", "-9").Run()

		// 3. Handle "Connect" Flag
		if connect != "" {
			expandedPath, _ := expandPath(dir)
			ensureRootPrivileges(expandedPath, verbose, maxworkers, limit, timeout, modify, connect, exportPath)

			reconnect := false
			currentConfig := connect
			pCurrentConfig := &currentConfig

			for {
				// Check if user signaled to stop before starting a new connection attempt
				select {
				case <-ctx.Done():
					fmt.Println("\nUser Exited!")
					return
				default:
				}

				fmt.Println("connecting to:", filepath.Base(*pCurrentConfig))

				// Optimization: Check history for location to avoid API calls [cite: 2]
				var country string
				history, _ := fileUtil.OpenText()
				for _, h := range history {
					if h.Path == *pCurrentConfig {
						country = h.Country
						break
					}
				}
				if country == "" {
					country = network.GetLocation(*pCurrentConfig)
				}

				err := vpn.ConnectAndMonitor(ctx, *pCurrentConfig, country, &reconnect, verbose)

				if ctx.Err() != nil {
					fmt.Println("User Exited!")
					return
				}

				if err != nil {
					fmt.Println("Connection failed. Attempting recovery...")
					if !reconnect {
						reconnect = true
						exec.Command("killall", "openvpn", "-9").Run()
						continue
					}

					// Reconnection with the same file failed, try fallback from history
					reconnect = false
					vpns, err := fileUtil.OpenText()
					if err != nil || len(vpns) == 0 {
						fmt.Println("No alternative configs available in history.")
						return
					}

					// Remove the failing config from history
					failFiltered := slices.DeleteFunc(vpns, func(s vpn.VPN) bool {
						return s.Path == strings.TrimSpace(*pCurrentConfig)
					})
					fileUtil.SaveAsText(failFiltered)

					if len(failFiltered) > 0 {
						*pCurrentConfig = failFiltered[0].Path
						fmt.Printf("Switched to alternative config: %s\n", filepath.Base(*pCurrentConfig))
						continue
					} else {
						fmt.Println("No more valid configs left in history.")
						return
					}
				}
			}
		}

		// 4. Default Mode: Test Configs
		expandedPath, _ := expandPath(dir)
		ensureRootPrivileges(expandedPath, verbose, maxworkers, limit, timeout, modify, connect, exportPath)

		if expandedPath == "/root/" {
			fmt.Println("Avoid using vmate as root/sudo directly")
			return
		}

		paths, err := fileUtil.GetConfigs(expandedPath)
		if err != nil {
			fmt.Println("Error reading configs:", err)
			return
		}

		if modify {
			fmt.Println("Modifying configs (fixing ciphers)...")
			fileUtil.ModifyConfigs(paths)
		}

		if maxworkers > len(paths) {
			maxworkers = len(paths)
		}

		progressChan := make(chan int, len(paths))
		if !verbose {
			fmt.Printf("Testing %d configs with %d workers (Limit: %d success)\n", len(paths), maxworkers, limit)
			go runProgressBar(len(paths), progressChan)
		}

		succeedConfigs := vpn.RunTest(ctx, paths, verbose, maxworkers, limit, timeout, progressChan)
		close(progressChan)

		fmt.Println("\n\n--- Final Result ---")
		for _, config := range succeedConfigs {
			fmt.Printf("%s -- %s\n", config.Path, config.Country)
		}
		fmt.Printf("Found: %d / Scanned: %d\n", len(succeedConfigs), len(paths))

		status, err := fileUtil.SaveAsText(succeedConfigs)
		if err == nil && status {
			fmt.Println("Saved to history (~/.config/vmate-cli/recent.txt)")
		}

		// Export feature
		if cmd.Flags().Changed("export") {
			fileUtil.ExportConfigs(succeedConfigs, exportPath)
		}
	},
}

func runProgressBar(total int, updates <-chan int) {
	current := 0
	for range updates {
		current++
		percent := float64(current) / float64(total) * 100
		barLen := 40
		filled := int((float64(current) / float64(total)) * float64(barLen))
		bar := strings.Repeat("#", filled) + strings.Repeat("-", barLen-filled)
		fmt.Printf("\r[%s] %.1f%% (%d/%d)", bar, percent, current, total)
	}
	fmt.Print("\n")
}

func Execute() {
	if err := rootCmd.Execute(); err != nil {
		os.Exit(1)
	}
}

func init() {
	rootCmd.Version = "beta-0.0.2a"
	rootCmd.PersistentFlags().StringVarP(&dir, "dir", "d", "~/", "The ovpn files' dir")
	rootCmd.PersistentFlags().IntVarP(&limit, "limit", "l", 100, "Limit the amount of succeed ovpn to find")
	rootCmd.PersistentFlags().BoolVarP(&verbose, "verbose", "", false, "To get more output")
	rootCmd.PersistentFlags().IntVarP(&timeout, "timeout", "t", 15, "The time given to each test process")
	rootCmd.PersistentFlags().IntVarP(&maxworkers, "max", "m", 200, "The max processes allowed per session")
	rootCmd.PersistentFlags().BoolVarP(&recent, "recent", "r", false, "To access the recent history")
	rootCmd.PersistentFlags().BoolVarP(&modify, "modify", "", false, "To modify wrong cipher of the configs")
	rootCmd.PersistentFlags().StringVarP(&connect, "connect", "c", "", "To connect to a specific config")
	rootCmd.PersistentFlags().StringVarP(&exportPath, "export", "e", "", "Export succeed configs to a folder")

	// REPLACE checkIncompatibleFlags with Cobra's built-in feature
	rootCmd.MarkFlagsMutuallyExclusive("recent", "connect", "dir")
}

func ensureRootPrivileges(expandedDir string, verbose bool, maxworkers int, limit int, timeout int, modify bool, connect string, export string) bool {
	if os.Getuid() == 0 {
		return true
	}
	exe, _ := os.Executable()
	args := []string{} // sudo needs the executable as the first argument in the slice usually

	// Reconstruct arguments for the sudo call [cite: 9, 10]
	if expandedDir != "" {
		args = append(args, "--dir", expandedDir)
	}
	if verbose {
		args = append(args, "--verbose")
	}
	if maxworkers != 200 {
		args = append(args, "--max", strconv.Itoa(maxworkers))
	}
	if limit != 100 {
		args = append(args, "--limit", strconv.Itoa(limit))
	}
	if timeout != 15 {
		args = append(args, "--timeout", strconv.Itoa(timeout))
	}
	if modify {
		args = append(args, "--modify")
	}
	if connect != "" {
		args = append(args, "--connect", connect)
	}
	if export != "" {
		args = append(args, "--export", export)
	}

	// Insert executable at the start for sudo
	args = append([]string{exe}, args...)

	cmd := exec.Command("sudo", args...)
	cmd.Stdin, cmd.Stdout, cmd.Stderr = os.Stdin, os.Stdout, os.Stderr

	err := cmd.Run()
	if err != nil {
		if exitError, ok := err.(*exec.ExitError); ok {
			os.Exit(exitError.ExitCode())
		}
		os.Exit(1)
	}
	os.Exit(0) // Parent process exits after sudo child finishes
	return false
}

func expandPath(path string) (string, error) {
	if strings.HasPrefix(path, "~/") {
		homeDir, _ := os.UserHomeDir()
		return strings.Replace(path, "~", homeDir, 1), nil
	}
	return path, nil
}
