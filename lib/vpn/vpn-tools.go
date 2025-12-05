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

var ErrorKeywords = []string{
	"No route to host",
	"TLS key negotiation failed",
	"Connection timed out",
	"Connection refused",
	"AUTH_FAILED",
	// Additional high-priority errors
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
}

// Get related args for the Commands
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
	if fun == "connect" {
		return []string{
			"--config", filePath,
		}
	}

	return []string{}
}

func testVPN(dir string) bool {
	// fmt.Println("in the test")
	// This context is for each process which is actual one
	ctx, cancel := context.WithTimeout(context.Background(), 15*time.Second)

	cmd := exec.CommandContext(ctx, "openvpn", getArgs("test", dir)...)
	cmd.SysProcAttr = &syscall.SysProcAttr{Setpgid: true}
	stdPipe, err := cmd.StdoutPipe()
	if err != nil {
		fmt.Println("Can't generate output")
	}
	// cmd.Stdout is ** already ** the pipe we created.
	// cmd.Stderr = cmd.Stdout makes ** stderr point to the SAME pipe
	cmd.Stderr = cmd.Stdout
	if err := cmd.Start(); err != nil {
		fmt.Println("Can't execute the command")
		cancel()
		return false
	}

	resultChan := make(chan bool)

	go func() {
		defer cancel()
		defer syscall.Kill(-cmd.Process.Pid, syscall.SIGKILL)
		scanner := bufio.NewScanner(stdPipe)

		for scanner.Scan() {
			line := strings.TrimSpace(scanner.Text())
			if strings.Contains(line, "Initialization Sequence Completed") {
				resultChan <- true
				return
			}

			for _, keyword := range ErrorKeywords {
				if strings.Contains(line, keyword) {
					resultChan <- false
					return
				}
			}

		}
		resultChan <- false
	}()

	var success bool
	select {
	case success = <-resultChan:
	case <-ctx.Done():
		success = false
		defer cancel()
		syscall.Kill(-cmd.Process.Pid, syscall.SIGKILL)

	}

	return success

}

type VPN struct {
	Path    string
	Country string
}

// add more flage params like timeout ,
// This context is every combined one
func RunTest(paths []string, verbose bool, maxworkers int) []VPN {

	succeedConfigs := []VPN{}
	sem := make(chan struct{}, maxworkers)
	var wg sync.WaitGroup
	var mu sync.Mutex

	for _, path := range paths {
		// testing limit
		// mu.Lock()
		// // if len(succeedConfigs) == 2 {
		// // 	break
		// // }
		// mu.Unlock()
		wg.Add(1)

		go func(p string) {
			defer wg.Done()
			defer func() {
				// release one capacity after the function done so that new function can take place
				<-sem
			}()
			// fill the capacity with {} and when fully filled the limit this go fun will get blocked
			sem <- struct{}{}
			// if ctx.Err() != nil {
			// 	fmt.Printf("Skipped %s (context cancelled)\n", p)
			// 	return
			// }

			if testVPN(p) {
				var c string
				mu.Lock()
				c = network.GetLocation(p)
				succeedConfigs = append(succeedConfigs, VPN{
					Path:    p,
					Country: c,
				})
				if verbose {
					fmt.Println(p, "---", c)
				}
				mu.Unlock()

			} else {
				if verbose {
					fmt.Println(p, "--- Failed")
				}
			}
		}(path)

	}
	wg.Wait()
	// defer cancel()

	return succeedConfigs

}
