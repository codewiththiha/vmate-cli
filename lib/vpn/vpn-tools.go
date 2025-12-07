package vpn

import (
	"bufio"
	"context"
	"fmt"
	"os/exec"
	"strings"
	"sync"
	"syscall"
	"time"
	"vmate/lib/network"
)

// TODO if stuck at "Initial packet from" should wait
// nah change the concept if it stuck at certain situation will skip it in a while but for the other conditions let

var ErrorKeywords = []string{
	"No route to host",
	"TLS key negotiation failed",
	"Connection timed out",
	"Connection refused",
	"AUTH_FAILED",
	"Network unreachable",
	"Host is down",
	"Name or service not known",
	"VERIFY ERROR",
	"certificate verify failed",
	"Inactivity timeout",
	"Ping timeout",
	"Cannot open TUN/TAP dev",
	"write to TUN/TAP: Input/output error",
	"read: Connection reset by peer",
	"handshake failure",
	"fatal error",
	"process exiting",
	"killed",
}

func getArgs(fun string, filePath string) []string {
	if fun == "test" {
		return []string{
			"--config", filePath,
			"--route-noexec",
			"--ifconfig-noexec",
			"--nobind",
			"--auth-nocache",
		}
	}
	return []string{}
}

// Update 1: Pass parent context to allow Ctrl+C to propagate immediately,
// while keeping the specific timeout logic here.
func testVPN(ctx context.Context, dir string, timeoutSec int) bool {
	// Create a child context that expires after X seconds OR if parent is canceled
	ctx, cancel := context.WithTimeout(ctx, time.Duration(timeoutSec)*time.Second)
	defer cancel()

	cmd := exec.CommandContext(ctx, "openvpn", getArgs("test", dir)...)
	cmd.SysProcAttr = &syscall.SysProcAttr{Setpgid: true}
	stdPipe, err := cmd.StdoutPipe()
	if err != nil {
		return false
	}
	cmd.Stderr = cmd.Stdout

	if err := cmd.Start(); err != nil {
		return false
	}

	resultChan := make(chan bool, 1)

	go func() {
		// Ensure we don't leak this goroutine if context dies
		defer close(resultChan)

		scanner := bufio.NewScanner(stdPipe)
		for scanner.Scan() {
			line := strings.TrimSpace(scanner.Text())
			if strings.Contains(line, "Initialization Sequence Completed") {
				// Non-blocking send
				select {
				case resultChan <- true:
				default:
				}
				return
			}
			for _, keyword := range ErrorKeywords {
				if strings.Contains(line, keyword) {
					select {
					case resultChan <- false:
					default:
					}
					return
				}
			}
		}
		// If scanner ends without success
		select {
		case resultChan <- false:
		default:
		}
	}()

	var success bool
	select {
	case success = <-resultChan:
		// Process finished naturally
	case <-ctx.Done():
		// Timeout or Ctrl+C
		success = false
	}

	// Cleanup: Kill the process group to ensure OpenVPN dies
	if cmd.Process != nil {
		syscall.Kill(-cmd.Process.Pid, syscall.SIGKILL)
	}

	return success
}

type VPN struct {
	Path    string
	Country string
}

// RunTest now accepts a Context (for Ctrl+C), a Progress Channel, and the timeout duration
func RunTest(ctx context.Context, paths []string, verbose bool, maxworkers int, limit int, timeout int, progressChan chan<- int) []VPN {
	succeedConfigs := []VPN{}

	// Create a cancelable context for the Limit logic.
	// If limit is reached, we cancel this context to stop spawning new workers.
	limitCtx, cancelLimit := context.WithCancel(ctx)
	defer cancelLimit()

	sem := make(chan struct{}, maxworkers)
	var wg sync.WaitGroup
	var mu sync.Mutex
LOOP:
	for _, path := range paths {
		// Check if we should stop starting new tests (Limit reached or Ctrl+C)
		if limitCtx.Err() != nil {
			break
		}

		wg.Add(1)

		// Acquire worker slot
		select {
		case sem <- struct{}{}:
		case <-limitCtx.Done():
			wg.Done()
			break LOOP
		}

		go func(p string) {
			defer wg.Done()
			defer func() { <-sem }() // Release worker slot

			// Always send a signal to progress bar when done (success or fail)
			if progressChan != nil {
				defer func() { progressChan <- 1 }()
			}

			// Check context again before running heavy process
			if limitCtx.Err() != nil {
				return
			}

			if testVPN(limitCtx, p, timeout) {
				mu.Lock()
				// Double check limit inside lock to prevent race condition
				if len(succeedConfigs) < limit {
					c := network.GetLocation(p)
					succeedConfigs = append(succeedConfigs, VPN{
						Path:    p,
						Country: c,
					})
					if verbose {
						fmt.Printf("\n[SUCCESS] %s --- %s\n", p, c)
					}

					// If we hit the limit, stop everything
					if len(succeedConfigs) >= limit {
						cancelLimit()
					}
				}
				mu.Unlock()
			} else {
				if verbose {
					fmt.Printf("\n[FAILED] %s\n", p)
				}
			}
		}(path)
	}

	wg.Wait()
	return succeedConfigs
}

// This function will complain the restart errors
// add verbose
func ConnectAndMonitor(ctx context.Context, configPath string, c string, preconnect *bool, verbose bool) error {
	cmd := exec.CommandContext(ctx, "openvpn", "--config", configPath)
	cmd.SysProcAttr = &syscall.SysProcAttr{Setpgid: true}

	stdPipe, err := cmd.StdoutPipe()
	if err != nil {
		fmt.Println("Can't read the output")
		return err
	}
	cmd.Stderr = cmd.Stdout
	if err := cmd.Start(); err != nil {
		return err
	}

	errorChannel := make(chan error, 1)

	go func(config string) {
		// var connected bool
		scanner := bufio.NewScanner(stdPipe)
		for scanner.Scan() {
			line := strings.TrimSpace(scanner.Text())
			if strings.Contains(line, "Initialization Sequence Completed") {
				// connected = true
				// if succeed after second times then reconnect should be try again so false
				*preconnect = false
				fmt.Println("Connected successfully to", c)
				// continue
			}

			if strings.Contains(line, "Restart pause") {
				errorChannel <- fmt.Errorf("restart detected")
				return
			}

			for _, keyword := range ErrorKeywords {
				if strings.Contains(line, keyword) {

					errorChannel <- fmt.Errorf("restart detected")
					return
				}
			}

			if verbose {
				fmt.Println(line)
			}
		}

	}(configPath)

	select {
	case err := <-errorChannel:
		syscall.Kill(-cmd.Process.Pid, syscall.SIGKILL)
		return err
	case <-ctx.Done():
		// User pressed Ctrl+C
		syscall.Kill(-cmd.Process.Pid, syscall.SIGKILL)
		return nil // No error, just normal exit

	}

}
