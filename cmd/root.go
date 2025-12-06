package cmd

import (
	"context"
	"fmt"
	"os"
	"os/exec"
	"os/signal"
	"strconv"
	"strings"
	"syscall"
	"vmate/lib/fileUtil"
	"vmate/lib/vpn"

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
			if checkIncompatibleFlags("recent") {
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

		expandedPath, _ := expandPath(dir)
		ensureRootPrivileges(expandedPath, verbose, maxworkers, limit, timeout, modify)

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
		barLen := 20
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
	rootCmd.Version = "beta-0.0.1b"
	rootCmd.PersistentFlags().StringVarP(&dir, "dir", "d", "~/", "The ovpn files' dir")
	rootCmd.PersistentFlags().IntVarP(&limit, "limit", "l", 100, "Limit the amount of succeed ovpn to find")
	rootCmd.PersistentFlags().BoolVarP(&verbose, "verbose", "", false, "To get more output")
	rootCmd.PersistentFlags().IntVarP(&timeout, "timeout", "t", 15, "The time given to each test process")
	rootCmd.PersistentFlags().IntVarP(&maxworkers, "max", "m", 200, "The max processes allowed per session")
	rootCmd.PersistentFlags().BoolVarP(&recent, "recent", "r", false, "To access the recent")
	rootCmd.PersistentFlags().BoolVarP(&modify, "modify", "", false, "To modify wrong cipher of the configs")
}

func ensureRootPrivileges(expandedDir string, verbose bool, maxworkers int, limit int, timeout int, modify bool) {
	if os.Getuid() == 0 {
		return
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

func checkIncompatibleFlags(current string) bool {
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

	totalFlags := 0
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
