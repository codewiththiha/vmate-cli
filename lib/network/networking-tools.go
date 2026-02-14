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

// ExtractHost parses an .ovpn file to find the 'remote' host address.
// TODO: Support multiple remote addresses if defined in the config.
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

// IpResolve resolves a hostname to an IP address.
// TODO: Implement IPv6 support and handle Multiple IP addresses gracefully.
func IpResolve(host string) (string, error) {
	ips, err := net.LookupIP(host)
	if err != nil {
		fmt.Println("IP resolve err", err)
		return "", err
	}
	// fmt.Println(string(ips[0]))
	return ips[0].String(), nil
}

// GetLocation fetches the country code for a given .ovpn config using ipinfo.io API.
// TODO: Add support for local GeoIP databases for offline use or as a fallback.
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
