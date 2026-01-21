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
	exportPath string // New flag
)

var rootCmd = &cobra.Command{
	Use:   "vmate",
	Short: "VPN config tester",
	Long:  `Test OpenVPN configurations from a directory with timeout and verbose options.`,
	Run: func(cmd *cobra.Command, args []string) {
		// If you confused why recent flag isn't in the ensureRootPrivileges then we just run this function
		//  above the ensureRootPrivileges so the double call doesn't happens and we don't need this
		//  to pass recent as parameter
		if recent {
			if checkIncompatibleFlags("recent", false) {
				return
			}
			vpns, err := fileUtil.OpenText()
			if err != nil {
				return
			}
			fmt.Println("Here're your previously succeed configs")
			fmt.Println("--------------------------------------")
			for _, vpn := range vpns {
				fmt.Println(vpn.Path + " -- " + vpn.Country)
			}
			return
		}

		if connect != "" {
			expandedPath, _ := expandPath(dir)
			reconnect := false
			Proconnect := &reconnect
			if checkIncompatibleFlags("connect", true) {
				return
			}
			ensureRootPrivileges(expandedPath, verbose, maxworkers, limit, timeout, modify, connect, exportPath)
			ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
			defer stop()
			defer exec.Command("killall", "openvpn", "-9").Run()
			loopcount := 0
			currentConfig := connect
			pCurrentConfig := &currentConfig
			for {
				// debug purpose
				loopcount++
				// fmt.Println("connecting:", loopcount, filepath.Base(currentConfig))
				fmt.Println("connecting to:", filepath.Base(currentConfig))
				// fmt.Println(reconnect)

				// Check recent.txt for location before calling API
				var c string
				history, _ := fileUtil.OpenText()
				for _, h := range history {
					if h.Path == currentConfig {
						c = h.Country
						break
					}
				}
				if c == "" {
					c = network.GetLocation(currentConfig)
				}

				err := vpn.ConnectAndMonitor(ctx, currentConfig, c, Proconnect, verbose)
				// fmt.Println(reconnect, "after getting back from func")
				if ctx.Err() != nil {
					fmt.Println("User Exited!")
					return
				}
				if err != nil {
					fmt.Println("Reconnecting")
					if !reconnect {

						reconnect = true
						exec.Command("killall", "openvpn", "-9").Run()
						continue
					}
					if reconnect {
						// fmt.Println("in the reconnect attempt")
						reconnect = false
						vpns, err := fileUtil.OpenText()
						if err != nil {
							return
						}
						if len(vpns) == 0 {
							fmt.Println("There's no saved config in your recent")
							return
						}
						failFiltered := slices.DeleteFunc(vpns, func(s vpn.VPN) bool {
							return s.Path == strings.TrimSpace(currentConfig)
						})
						_, _ = fileUtil.SaveAsText(failFiltered)

						if len(failFiltered) == 0 {
							fmt.Println("There's no saved config in your recent")
							return
						}
						if len(failFiltered) > 0 {
							if len(failFiltered) > 1 {
								newConfig := failFiltered[1].Path
								*pCurrentConfig = newConfig
								fmt.Println("New Config inserted", filepath.Base(currentConfig))
								continue
							} else {
								newConfig := failFiltered[0].Path
								*pCurrentConfig = newConfig
								fmt.Println("New Config inserted", filepath.Base(currentConfig))
								continue
							}
						}
					}
					return
				}
			}
		}

		expandedPath, _ := expandPath(dir)
		alreadyRoot := ensureRootPrivileges(expandedPath, verbose, maxworkers, limit, timeout, modify, connect, exportPath)
		if alreadyRoot {
			if expandedPath == "/root/" {
				fmt.Println("Avoid using vmate as root/sudo")
				return
			}
		}

		paths, err := fileUtil.GetConfigs(expandedPath)

		if modify {
			fmt.Println("Modifying!!")
			fileUtil.ModifyConfigs(paths)
		}

		if err != nil {
			fmt.Println("Error reading configs:", err)
			return
		}

		if maxworkers > len(paths) {
			maxworkers = len(paths)
		}

		// 1. Setup Signal Handling (Ctrl+C)
		ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
		defer stop()

		// Safety net: Force kill openvpn on exit
		defer exec.Command("killall", "openvpn", "-9").Run()

		// 2. Setup Progress Bar Channel
		progressChan := make(chan int, len(paths))

		if !verbose {
			fmt.Printf("Testing %d configs with %d workers (Limit: %d success)\n", len(paths), maxworkers, limit)
			go runProgressBar(len(paths), progressChan)
		}

		// 3. Run the Tests
		succeedConfigs := vpn.RunTest(ctx, paths, verbose, maxworkers, limit, timeout, progressChan)

		// Close channel
		close(progressChan)

		// 4. Output Results
		fmt.Println("\n\n--- Final Result ---")
		for _, config := range succeedConfigs {
			fmt.Printf("%s -- %s\n", config.Path, config.Country)
		}
		fmt.Printf("Found: %d / Scanned: %d\n", len(succeedConfigs), len(paths))
		status, err := fileUtil.SaveAsText(succeedConfigs)
		if err != nil {
			fmt.Println("Can't create the file")
		}
		if status {
			fmt.Println("Saved to your history (~/.config/vmate-cli/recent.txt)")
		}

		// New Export Logic
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
		if current == total {
			break
		}
	}
	fmt.Print("\n")
}

func Execute() {
	err := rootCmd.Execute()
	if err != nil {
		os.Exit(1)
	}
}

func init() {
	rootCmd.Version = "beta-0.0.3"
	rootCmd.PersistentFlags().StringVarP(&dir, "dir", "d", "~/", "The ovpn files' dir")
	rootCmd.PersistentFlags().IntVarP(&limit, "limit", "l", 100, "Limit the amount of succeed ovpn to find")
	rootCmd.PersistentFlags().BoolVarP(&verbose, "verbose", "", false, "To get more output")
	rootCmd.PersistentFlags().IntVarP(&timeout, "timeout", "t", 15, "The time given to each test process")
	rootCmd.PersistentFlags().IntVarP(&maxworkers, "max", "m", 200, "The max processes allowed per session")
	rootCmd.PersistentFlags().BoolVarP(&recent, "recent", "r", false, "To access the recent")
	rootCmd.PersistentFlags().BoolVarP(&modify, "modify", "", false, "To modify wrong cipher of the configs")
	rootCmd.PersistentFlags().StringVarP(&connect, "connect", "c", "", "To connect to a config")
	rootCmd.PersistentFlags().StringVarP(&exportPath, "export", "e", "", "Export folder path")
}

func ensureRootPrivileges(expandedDir string, verbose bool, maxworkers int, limit int, timeout int, modify bool, connect string, export string) bool {
	if os.Getuid() == 0 {
		return true
	}
	exe, _ := os.Executable()
	args := []string{exe}
	if expandedDir != "" {
		args = append(args, "--dir", expandedDir)
	}
	if verbose {
		args = append(args, "--verbose", "true")
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
		args = append(args, "--modify", "true")
	}
	if connect != "" {
		args = append(args, "--connect", connect)
	}
	if export != "" {
		args = append(args, "--export", export)
	}

	cmd := exec.Command("sudo", args...)
	cmd.Stdin, cmd.Stdout, cmd.Stderr = os.Stdin, os.Stdout, os.Stderr
	_ = cmd.Run()
	os.Exit(0)
	return false
}

func expandPath(path string) (string, error) {
	if strings.HasPrefix(path, "~/") {
		homeDir, _ := os.UserHomeDir()
		return strings.Replace(path, "~", homeDir, 1), nil
	}
	return path, nil
}

func checkIncompatibleFlags(current string, verboseAllow bool) bool {
	// If connect is being used, only verbose is allowed.
	// We ignore defaults by checking if the values were changed or if they differ from default.
	conditions := []bool{
		dir != "~/",
		maxworkers != 200,
		limit != 100,
		timeout != 15,
		recent,
		modify,
		exportPath != "",
	}

	var totalFlags int
	if verboseAllow && verbose {
		totalFlags = -1
	} else {
		totalFlags = 0
		if verbose {
			totalFlags++
		}
	}

	for _, active := range conditions {
		if active {
			totalFlags++
		}
	}

	if totalFlags > 1 {
		fmt.Println("You can only use", current, "flag as a single flag")
		return true
	}
	return false
}
