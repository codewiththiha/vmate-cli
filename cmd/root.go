package cmd

import (
	"fmt"
	"math"
	"os"
	"os/exec"
	"os/signal"
	"strconv"
	"strings"
	"syscall"
	"time"
	"vmate/lib/fileUtil"
	"vmate/lib/vpn"

	"github.com/spf13/cobra"
)

// TODO make flags actually usable
// TODO use millisecond var for more fine grain control

var (
	dir string
	// The limit usage should be limited ;) it only make sense to use if user's laptop has low ram and
	// he's limited the max workers lower than 20, otherwise should be neglected
	limit   int
	timeout int
	// TODO if the user specified workers exceed the total amount of configs handle that
	maxworkers int

	verbose bool
)

// rootCmd represents the base command when called without any subcommands
var rootCmd = &cobra.Command{
	Use:   "vmate",
	Short: "VPN config tester",
	Long:  `Test OpenVPN configurations from a directory with timeout and verbose options.`,
	Run: func(cmd *cobra.Command, args []string) {
		//// You can see the ensureRootPrivileges function restart the process and double call
		////  with following print func if you want to

		// fmt.Println(verbose)
		// Your original logic goes here, now using flags

		expandedPath, _ := expandPath(dir)

		ensureRootPrivileges(expandedPath, verbose, maxworkers)

		paths, err := fileUtil.GetConfigs(expandedPath)
		if maxworkers > len(paths) {
			maxworkers = len(paths)
			fmt.Println("Since your specified amount is smaller than the configs to test your maxworkers is set to", maxworkers)
		}
		circle := float64(len(paths)) / float64(maxworkers)
		fmt.Println(len(paths), "/", maxworkers, int(math.Round(circle)))

		// +1 to exceed the progress bar or just make progress bar -1
		// this seem to be wrong in someway
		// ctx, cancel := context.WithTimeout(context.Background(), time.Duration(timeout*int(math.Round(circle)))*time.Second)

		if err != nil {
			fmt.Println("can't get the paths")
		}

		if !verbose {
			go progressBar(timeout * int(math.Round(circle)))
		}

		// Handle Ctrl+C
		sigChan := make(chan os.Signal, 1)
		signal.Notify(sigChan, os.Interrupt, syscall.SIGTERM)

		go func() {
			// This mf will keep waitinig and get stucked
			<-sigChan
			fmt.Println("Force Exit Detected")
			// I have to reconsider this will this even okay??
			exec.Command("killall", "openvpn", "-9").Run()

			// cancel() // Signal all goroutines to stop

		}()

		succeedConfigs := vpn.RunTest(paths, verbose, maxworkers)
		fmt.Println("Final Result")
		for _, config := range succeedConfigs {
			fmt.Println(config.Path, "--", config.Country)
		}

		fmt.Println(len(succeedConfigs), "/", len(paths))
		// to make sure
		defer exec.Command("killall", "openvpn", "-9").Run()
		// defer cancel()

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
	rootCmd.Version = "beta-0.0.1"
	rootCmd.PersistentFlags().StringVarP(&dir, "dir", "d", "~/", "The ovpn files' dir")
	rootCmd.PersistentFlags().IntVarP(&limit, "limit", "l", 100, "Limit the amount of succeed ovpn to find")
	rootCmd.PersistentFlags().BoolVarP(&verbose, "verbose", "", false, "To get more output")
	rootCmd.PersistentFlags().IntVarP(&timeout, "timeout", "t", 15, "The time given to the test processs")
	rootCmd.PersistentFlags().IntVarP(&maxworkers, "max", "m", 15, "The max processes allowed per session")

}

// to get the root permission for our app
func ensureRootPrivileges(expandedDir string, verbose bool, maxworkers int) {
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
	if verbose {
		args = append(args, "--verbose", "true")
	}
	// because default is 15 we don't need to change for that condition right!
	if maxworkers != 15 {
		args = append(args, "--max", strconv.Itoa(maxworkers))
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

func progressBar(max int) {
	for i := 0; i <= 100; i += 10 {
		// eg 15 becomes 1500
		time.Sleep(time.Duration(max*100) * time.Millisecond)

		// Construct a simple progress bar string (e.g., "[##########] 50%")
		barLength := i / 10
		bar := "[" + strings.Repeat("#", barLength) + strings.Repeat(" ", 10-barLength) + "]"

		// Use \r (carriage return) to move the cursor to the start of the line
		// and overwrite the previous output.
		// Use fmt.Printf instead of fmt.Println to avoid a newline.
		fmt.Printf("\rProgress: %s %d%%", bar, i)
	}

	// After the loop finishes (at 100%), print a final newline
	// so the next prompt or output appears on a new line.
	fmt.Println("\nAlmost there!!")
}
