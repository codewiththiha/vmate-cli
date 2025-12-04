package vpn

import (
	"bufio"
	"context"
	"fmt"
	"os/exec"
	"strings"
	"sync"
	"syscall"
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

func testVPN(dir string, ctx context.Context) bool {
	// fmt.Println("in the test")
	cmd := exec.CommandContext(ctx, "openvpn", getArgs("test", dir)...)
	cmd.SysProcAttr = &syscall.SysProcAttr{Setpgid: true}
	stdPipe, err := cmd.StdoutPipe()
	if err != nil {
		fmt.Println("Can't generate output")
	}
	// the error is also from the output (basically we are combining)
	cmd.Stderr = cmd.Stdout
	if err := cmd.Start(); err != nil {
		fmt.Println("Can't execute the command")
		return false
	}

	resultChan := make(chan bool)

	go func() {
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

	}

	return success

}

type VPN struct {
	Path    string
	Country string
}

// add more flage params like timeout ,
func RunTest(paths []string, ctx context.Context, cancel context.CancelFunc) []VPN {
	// actually we can pass the paths from the root directly
	// paths, err := fileUtil.GetConfigs(dir)
	// if err != nil {
	// 	fmt.Print("Can't get the configs")
	// ctx, cancel := context.WithCancel(ctx)
	// }
	succeedConfigs := []VPN{}
	sem := make(chan struct{}, len(paths))
	var wg sync.WaitGroup
	var mu sync.Mutex

	for _, path := range paths {
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
			if ctx.Err() != nil {
				fmt.Printf("⏹️ Skipped %s (context cancelled)\n", p)
				return
			}

			if testVPN(p, ctx) {
				mu.Lock()
				succeedConfigs = append(succeedConfigs, VPN{
					Path:    p,
					Country: network.GetLocation(p),
				})
				mu.Unlock()

			} else {
			}
		}(path)

	}
	wg.Wait()
	defer cancel()

	return succeedConfigs

}

// // To actually test the function
// func advancedTestVPN(filePath string, ctx context.Context) bool {
// 	// fmt.Println("here is the path", filePath)

// 	args := getArgs("test", filePath)
// 	cmd := exec.CommandContext(ctx, "openvpn", args...)
// 	cmd.SysProcAttr = &syscall.SysProcAttr{Setpgid: true}
// 	stdOutdPipe, err := cmd.StdoutPipe()
// 	if err != nil {
// 		fmt.Println("Can't generate output")
// 		return false
// 	}
// 	cmd.Stderr = cmd.Stdout
// 	if err := cmd.Start(); err != nil {
// 		fmt.Println("Can't start the cmd 2.0", err)
// 		return false
// 	}

// 	resultChan := make(chan bool)

// 	go func() {
// 		defer syscall.Kill(-cmd.Process.Pid, syscall.SIGKILL)
// 		scanner := bufio.NewScanner(stdOutdPipe)
// 		for scanner.Scan() {
// 			line := strings.TrimSpace(scanner.Text())
// 			if strings.Contains(line, "Initialization Sequence Completed") {
// 				resultChan <- true
// 				return
// 			}

// 			for _, keyword := range ErrorKeywords {
// 				if strings.Contains(line, keyword) {
// 					resultChan <- false
// 					return
// 				}
// 			}

// 		}

// 		resultChan <- false

// 	}()

// 	var success bool
// 	select {
// 	case success = <-resultChan:
// 	case <-ctx.Done():
// 		success = false
// 	}

// 	return success

// }
