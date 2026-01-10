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

	"github.com/codewiththiha/vmate-cli/lib/network"
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
// func RunTest(ctx context.Context, paths []string, verbose bool, maxworkers int, limit int, timeout int, progressChan chan<- int) []VPN {
// 	succeedConfigs := []VPN{}

// 	// Create a cancelable context for the Limit logic.
// 	// If limit is reached, we cancel this context to stop spawning new workers.
// 	limitCtx, cancelLimit := context.WithCancel(ctx)
// 	defer cancelLimit()

// 	sem := make(chan struct{}, maxworkers)
// 	var wg sync.WaitGroup
// 	var mu sync.Mutex
// LOOP:
// 	for _, path := range paths {
// 		// Check if we should stop starting new tests (Limit reached or Ctrl+C)
// 		if limitCtx.Err() != nil {
// 			break
// 		}

// 		wg.Add(1)

// 		// Acquire worker slot
// 		select {
// 		case sem <- struct{}{}:
// 		case <-limitCtx.Done():
// 			wg.Done()
// 			break LOOP
// 		}

// 		go func(p string) {
// 			defer wg.Done()
// 			defer func() { <-sem }() // Release worker slot

// 			// Always send a signal to progress bar when done (success or fail)
// 			if progressChan != nil {
// 				defer func() { progressChan <- 1 }()
// 			}

// 			// Check context again before running heavy process
// 			if limitCtx.Err() != nil {
// 				return
// 			}

// 			if testVPN(limitCtx, p, timeout) {
// 				mu.Lock()
// 				// Double check limit inside lock to prevent race condition
// 				if len(succeedConfigs) < limit {
// 					c := network.GetLocation(p)
// 					succeedConfigs = append(succeedConfigs, VPN{
// 						Path:    p,
// 						Country: c,
// 					})
// 					if verbose {
// 						fmt.Printf("\n[SUCCESS] %s --- %s\n", p, c)
// 					}

// 					// If we hit the limit, stop everything
// 					if len(succeedConfigs) >= limit {
// 						cancelLimit()
// 					}
// 				}
// 				mu.Unlock()
// 			} else {
// 				if verbose {
// 					fmt.Printf("\n[FAILED] %s\n", p)
// 				}
// 			}
// 		}(path)
// 	}

// 	wg.Wait()
// 	return succeedConfigs
// }

// New Function that's also use go-routines for ipinfo fetching
func RunTest(ctx context.Context, paths []string, verbose bool, maxworkers int, limit int, timeout int, progressChan chan<- int) []VPN {
	succeedConfigs := []VPN{}

	// Create a cancelable context for the Limit logic.
	limitCtx, cancelLimit := context.WithCancel(ctx)
	defer cancelLimit()

	sem := make(chan struct{}, maxworkers)
	var wg sync.WaitGroup
	var mu sync.Mutex

LOOP:
	for _, path := range paths {
		// Check if we should stop starting new tests
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

			// Always send a signal to progress bar when done
			if progressChan != nil {
				defer func() { progressChan <- 1 }()
			}

			if limitCtx.Err() != nil {
				return
			}

			// 1. Run the VPN Test (Respects Timeout)
			if testVPN(limitCtx, p, timeout) {

				// 2. Optimization: Check limit *before* doing the expensive API call.
				// We use a read-lock or just a quick check. It's okay if it's slightly "loose"
				// to avoid blocking, but strictly we should lock.
				mu.Lock()
				if len(succeedConfigs) >= limit {
					mu.Unlock()
					cancelLimit() // Stop others
					return
				}
				mu.Unlock()

				// 3. FETCH LOCATION CONCURRENTLY (Outside the lock!)
				// This is where your bottleneck was. Now it runs in parallel.
				c := network.GetLocation(p)

				// 4. Save the result safely
				mu.Lock()
				if len(succeedConfigs) < limit {
					succeedConfigs = append(succeedConfigs, VPN{
						Path:    p,
						Country: c,
					})
					if verbose {
						fmt.Printf("\n[SUCCESS] %s --- %s\n", p, c)
					}
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
	// 1. Start command with PARENT context (ctx), not the timeout context.
	// This ensures the process isn't auto-killed after 5 seconds.
	cmd := exec.CommandContext(ctx, "openvpn", "--config", configPath)
	cmd.SysProcAttr = &syscall.SysProcAttr{Setpgid: true}

	stdPipe, err := cmd.StdoutPipe()
	if err != nil {
		return err
	}
	cmd.Stderr = cmd.Stdout
	if err := cmd.Start(); err != nil {
		return err
	}

	// Channels to signal status from the monitoring goroutine
	successChan := make(chan bool, 1)
	errorChan := make(chan error, 1)

	// Monitor Goroutine
	go func() {
		scanner := bufio.NewScanner(stdPipe)
		isConnected := false

		for scanner.Scan() {
			line := strings.TrimSpace(scanner.Text())

			// A. Check for Success
			if !isConnected && strings.Contains(line, "Initialization Sequence Completed") {
				isConnected = true
				// Signal success non-blocking
				select {
				case successChan <- true:
				default:
				}
			}

			// B. Check for Errors
			for _, keyword := range ErrorKeywords {
				if strings.Contains(line, keyword) {
					errorChan <- fmt.Errorf("error keyword found: %s", keyword)
					return
				}
			}
			if strings.Contains(line, "Restart pause") {
				errorChan <- fmt.Errorf("restart pause detected")
				return
			}

			if verbose {
				fmt.Println(line)
			}
		}
		// If scanner exits (process died unexpectedly)
		errorChan <- fmt.Errorf("process exited unexpectedly")
	}()

	// --- PHASE 1: The Handshake (Strict 5s Limit) ---
	select {
	case <-successChan:
		// SUCCESS! We connected within 5 seconds.
		fmt.Println("Connected successfully to", c)
		*preconnect = false
		// DO NOT RETURN. We now proceed to Phase 2 to keep the connection alive.

	case <-time.After(5 * time.Second):
		// TIMEOUT! It took too long. Kill it.
		syscall.Kill(-cmd.Process.Pid, syscall.SIGKILL)
		*preconnect = true
		return fmt.Errorf("connection timed out (exceeded 5s)")

	case err := <-errorChan:
		// ERROR! Immediate failure (Auth failed, etc.)
		syscall.Kill(-cmd.Process.Pid, syscall.SIGKILL)
		*preconnect = true
		return err

	case <-ctx.Done():
		// User pressed Ctrl+C
		syscall.Kill(-cmd.Process.Pid, syscall.SIGKILL)
		return nil
	}

	// --- PHASE 2: Monitoring (Indefinite "Maximized" Time) ---
	// We only reach here if Phase 1 succeeded. Now we wait forever.
	select {
	case err := <-errorChan:
		// The VPN crashed/disconnected LATER (after the first 5s)
		syscall.Kill(-cmd.Process.Pid, syscall.SIGKILL)
		*preconnect = false // Mark as failed so you can retry if desired
		return err

	case <-ctx.Done():
		// User pressed Ctrl+C
		syscall.Kill(-cmd.Process.Pid, syscall.SIGKILL)
		return nil
	}
}

//// Previous logic

// func ConnectAndMonitor(ctx context.Context, configPath string, c string, preconnect *bool, verbose bool) error {
// 	timeout := 5

// 	sctx, stop := context.WithTimeout(ctx, time.Duration(timeout)*time.Second)
// 	defer stop()

// 	cmd := exec.CommandContext(sctx, "openvpn", "--config", configPath)
// 	cmd.SysProcAttr = &syscall.SysProcAttr{Setpgid: true}

// 	stdPipe, err := cmd.StdoutPipe()
// 	if err != nil {
// 		fmt.Println("Can't read the output")
// 		return err
// 	}
// 	cmd.Stderr = cmd.Stdout
// 	if err := cmd.Start(); err != nil {
// 		return err
// 	}

// 	errorChannel := make(chan error, 1)

// 	go func(config string) {
// 		// var connected bool
// 		*preconnect = false
// 		scanner := bufio.NewScanner(stdPipe)
// 		for scanner.Scan() {
// 			line := strings.TrimSpace(scanner.Text())
// 			if strings.Contains(line, "Initialization Sequence Completed") {
// 				//////Want to maximize the timeout back to 3600 1 hr for the sctx

// 				// connected = true
// 				// if succeed after second times then reconnect should be try again so false
// 				*preconnect = false
// 				fmt.Println("Connected successfully to", c)
// 				// continue
// 			}

// 			if strings.Contains(line, "Restart pause") {
// 				errorChannel <- fmt.Errorf("restart detected")
// 				return
// 			}

// 			for _, keyword := range ErrorKeywords {
// 				if strings.Contains(line, keyword) {

// 					errorChannel <- fmt.Errorf("restart detected")
// 					return
// 				}
// 			}

// 			if verbose {
// 				fmt.Println(line)
// 			}
// 		}

// 	}(configPath)

// 	select {
// 	case err := <-errorChannel:
// 		syscall.Kill(-cmd.Process.Pid, syscall.SIGKILL)
// 		return err
// 	case <-ctx.Done():
// 		// User pressed Ctrl+C
// 		syscall.Kill(-cmd.Process.Pid, syscall.SIGKILL)
// 		return nil // No error, just normal exit
// 	case <-sctx.Done():
// 		*preconnect = true
// 		syscall.Kill(-cmd.Process.Pid, syscall.SIGKILL)
// 		fmt.Println("returning err")
// 		return err

// 	}

// }
