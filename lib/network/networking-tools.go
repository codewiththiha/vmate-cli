package network

import (
	"bufio"
	"encoding/json"
	"fmt"
	"net"
	"net/http"
	"os"
	"strings"
	"time"
)

// I know mf :)) they offer free with no limits
const apiKey = "44936a1f60206d"

func ExtractHost(dir string) (string, error) {
	file, err := os.Open(dir)
	if err != nil {
		fmt.Println("Can't open the file(extractHost)")
	}
	scanner := bufio.NewScanner(file)

	for scanner.Scan() {
		line := strings.TrimSpace(scanner.Text())

		if strings.HasPrefix(line, "remote") {
			parts := strings.Fields(line)
			// fmt.Print(parts)
			if len(parts) >= 2 {
				// fmt.Println(parts[1])
				return parts[1], nil
			}

		}

	}

	if err := scanner.Err(); err != nil {
		return "", err
	}

	return "", err
}

func IpResolve(host string) (string, error) {
	ips, err := net.LookupIP(host)
	if err != nil {
		fmt.Println("IP resolve err", err)
		return "", err
	}
	// fmt.Println(string(ips[0]))
	return ips[0].String(), nil
}

func GetLocation(dir string) string {
	host, err := ExtractHost(dir)
	if err != nil {
		fmt.Println("Can't fetch the host")
		return "Unknown"
	}
	ip, err := IpResolve(host)
	if err != nil {
		fmt.Println("Can't resolve the host")
	}

	url := fmt.Sprintf("https://api.ipinfo.io/lite/%s?token=%s", ip, apiKey)
	client := &http.Client{Timeout: 5 * time.Second}

	resp, err := client.Get(url)
	if err != nil {
		fmt.Println("Can't fetch the response")
		return "UNKNOWN"
	}
	if resp.StatusCode != 200 {
		return "ERR_API"
	}

	var info struct {
		CountryCode string `json:"country_code"`
	}
	if err := json.NewDecoder(resp.Body).Decode(&info); err != nil {
		fmt.Println("JSON decoding error")
	}

	return info.CountryCode

}
