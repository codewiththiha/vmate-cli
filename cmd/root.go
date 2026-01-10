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
			ensureRootPrivileges(expandedPath, verbose, maxworkers, limit, timeout, modify, connect)
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

				///

				// should format to path and unknown country vpn.VPN type so if we used from recent there's already country and we can skip

				c := network.GetLocation(currentConfig)

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
						//// Not necessary
						// exec.Command("killall", "openvpn", "-9").Run()
						vpns, err := fileUtil.OpenText()
						if err != nil {
							return
						}
						if len(vpns) == 1 {
							fmt.Println("There's no saved config in your recent")
							return
						}
						failFiltered := slices.DeleteFunc(vpns, func(s vpn.VPN) bool {
							return s.Path == strings.TrimSpace(currentConfig)
						})
						status, err := fileUtil.SaveAsText(failFiltered)
						if err != nil {
							fmt.Println("Save failed")
							return
						}
						// this status from the saveastext function is unnecessary will remove later
						if !status {
							fmt.Println("Save failed")
							return
						}

if len(failFiltered) == 0 {
							fmt.Println("There's no saved config in your recent")
							return
						}
						if len(failFiltered) > 0 {
							// Fix index out of range by checking length before accessing index 1
							if len(failFiltered) > 1 {
								newConfig := failFiltered[1].Path
								*pCurrentConfig = newConfig
								fmt.Println("New Config inserted", filepath.Base(currentConfig))
								continue
							} else {
								// If there's only one config, keep using the first one
								newConfig := failFiltered[0].Path
								*pCurrentConfig = newConfig
								fmt.Println("New Config inserted", filepath.Base(currentConfig))
								continue
							}
						}
					}

					// just need to add smart function that will automatically use a config from the recent if the retry on same config failed
					// sometimes stuck at here  initial untrusted session promoted to trusted
					// continue

					return
				}
			}

		}

		expandedPath, _ := expandPath(dir)
		alreadyRoot := ensureRootPrivileges(expandedPath, verbose, maxworkers, limit, timeout, modify, connect)
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
		// We create a context that gets canceled when Ctrl+C is pressed
		ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
		defer stop()

		// Safety net: Force kill openvpn on exit
		defer exec.Command("killall", "openvpn", "-9").Run()

		// 2. Setup Progress Bar Channel
		// This channel receives a '1' every time a single test finishes
		progressChan := make(chan int, len(paths))

		if !verbose {
			fmt.Printf("Testing %d configs with %d workers (Limit: %d success)\n", len(paths), maxworkers, limit)
			// Start the visual progress bar in a separate goroutine
			go runProgressBar(len(paths), progressChan)
		}

		// 3. Run the Tests
		// We pass the signal context 'ctx'. RunTest handles the 'limit' logic internally.
		succeedConfigs := vpn.RunTest(ctx, paths, verbose, maxworkers, limit, timeout, progressChan)

		// Close channel to ensure progress bar stops if it hasn't finished
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
			fmt.Println("Saved to your history access via --recent or -r flag")
		}
	},
}

// Real Progress Bar based on actual completed tasks
func runProgressBar(total int, updates <-chan int) {
	current := 0
	for range updates {
		current++
		percent := float64(current) / float64(total) * 100

		// Create a bar like [#####     ]
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
	rootCmd.Version = "beta-0.0.2a"
	rootCmd.PersistentFlags().StringVarP(&dir, "dir", "d", "~/", "The ovpn files' dir")
	rootCmd.PersistentFlags().IntVarP(&limit, "limit", "l", 100, "Limit the amount of succeed ovpn to find")
	rootCmd.PersistentFlags().BoolVarP(&verbose, "verbose", "", false, "To get more output")
	rootCmd.PersistentFlags().IntVarP(&timeout, "timeout", "t", 15, "The time given to each test process")
	rootCmd.PersistentFlags().IntVarP(&maxworkers, "max", "m", 200, "The max processes allowed per session")
	rootCmd.PersistentFlags().BoolVarP(&recent, "recent", "r", false, "To access the recent")
	rootCmd.PersistentFlags().BoolVarP(&modify, "modify", "", false, "To modify wrong cipher of the configs")
	rootCmd.PersistentFlags().StringVarP(&connect, "connect", "c", "", "To connect to a config")
}

func ensureRootPrivileges(expandedDir string, verbose bool, maxworkers int, limit int, timeout int, modify bool, connect string) bool {
	if os.Getuid() == 0 {
		return true
	}
	exe, err := os.Executable()
	if err != nil {
		fmt.Println("Error getting executable path")
		os.Exit(1)
	}
	args := []string{exe}
	if expandedDir != "" {
		args = append(args, "--dir", expandedDir)
	}
	if verbose {
		args = append(args, "--verbose", "true")
	}
	// Pass flags back to sudo call
	if maxworkers != 200 {
		args = append(args, "--max", strconv.Itoa(maxworkers))
	}
	// Also pass limit and timeout if they are not default
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
	os.Exit(0)
	return false
}

func expandPath(path string) (string, error) {
	if strings.HasPrefix(path, "~/") {
		homeDir, err := os.UserHomeDir()
		if err != nil {
			return "", fmt.Errorf("could not get home directory: %w", err)
		}
		return strings.Replace(path, "~", homeDir, 1), nil
	}
	return path, nil
}

func checkIncompatibleFlags(current string, verboseAllow bool) bool {
	//// I'm obesed with minimal shorter codes so in this case I have to spam if states multiple times so i tried to shorten it
	// totalFlags := 0
	// if dir != "~/" {
	// 	totalFlags++
	// }
	// if verbose {
	// 	totalFlags++
	// }
	// // Pass flags back to sudo call
	// if maxworkers != 200 {
	// 	totalFlags++
	// }
	// // Also pass limit and timeout if they are not default
	// if limit != 100 {
	// 	totalFlags++
	// }
	// if timeout != 15 {
	// 	totalFlags++
	// }

	// if recent {
	// 	totalFlags++
	// }
	// if totalFlags > 1 {
	// 	fmt.Println("You can only use", current, "flag as a single flag")
	// 	return true
	// }
	//// If the values ain't the default then we can know user pass other flags
	//  if the user passed the default values then i don't know ;))
	conditions := []bool{
		dir != "~/",
		verbose,
		maxworkers != 200,
		limit != 100,
		timeout != 15,
		recent,
	}
	var totalFlags int
	if verboseAllow && verbose {
		// to able to pass the check
		totalFlags = -1
	} else {
		totalFlags = 0
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
