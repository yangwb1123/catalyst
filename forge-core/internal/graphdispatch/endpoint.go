package graphdispatch

import (
	"net/netip"
	"net/url"
	"strconv"
	"strings"
)

const httpsEndpointPrefix = "https://"

// validEndpoint accepts a conservative subset that both Go net/url and Rust
// url::Url preserve byte-for-byte. It deliberately excludes URL spellings
// whose host, port, or path either parser could normalize.
func validEndpoint(value string) bool {
	if !validText(value, MaxEndpointBytes) ||
		!strings.HasPrefix(value, httpsEndpointPrefix) {
		return false
	}
	authority, path := endpointAuthorityAndPath(value[len(httpsEndpointPrefix):])
	host, port, valid := splitEndpointAuthority(authority)
	if !valid || !validEndpointHost(host) ||
		(port != "" && !validEndpointPort(port)) ||
		!validEndpointPath(path) {
		return false
	}
	parsed, err := url.Parse(value)
	return err == nil && parsed.Scheme == "https" && parsed.Host == authority &&
		parsed.Hostname() == host && parsed.Port() == port && parsed.User == nil &&
		!parsed.OmitHost && !parsed.ForceQuery && parsed.RawQuery == "" &&
		parsed.Fragment == "" && parsed.RawFragment == "" && parsed.RawPath == "" &&
		parsed.Path == path && parsed.String() == value
}

func endpointAuthorityAndPath(value string) (string, string) {
	authority, path, hasPath := strings.Cut(value, "/")
	if !hasPath {
		return authority, ""
	}
	return authority, "/" + path
}

func splitEndpointAuthority(authority string) (string, string, bool) {
	if authority == "" || strings.Count(authority, ":") > 1 {
		return "", "", false
	}
	host, port, hasPort := strings.Cut(authority, ":")
	if !hasPort {
		return host, "", host != ""
	}
	return host, port, host != "" && port != ""
}

func validEndpointHost(host string) bool {
	if len(host) == 0 || len(host) > 253 {
		return false
	}
	if isNumericHost(host) {
		address, err := netip.ParseAddr(host)
		return err == nil && address.Is4() && address.String() == host
	}
	labels := strings.Split(host, ".")
	if !endpointLabelHasLetter(labels[len(labels)-1]) {
		return false
	}
	for _, label := range labels {
		if !validEndpointDNSLabel(label) {
			return false
		}
	}
	return true
}

func isNumericHost(host string) bool {
	for index := range len(host) {
		if character := host[index]; character != '.' &&
			(character < '0' || character > '9') {
			return false
		}
	}
	return true
}

func endpointLabelHasLetter(label string) bool {
	for index := range len(label) {
		if label[index] >= 'a' && label[index] <= 'z' {
			return true
		}
	}
	return false
}

func validEndpointDNSLabel(label string) bool {
	if len(label) == 0 || len(label) > 63 ||
		!isEndpointDNSAlphanumeric(label[0]) ||
		!isEndpointDNSAlphanumeric(label[len(label)-1]) {
		return false
	}
	for index := range len(label) {
		if !isEndpointDNSAlphanumeric(label[index]) && label[index] != '-' {
			return false
		}
	}
	return true
}

func isEndpointDNSAlphanumeric(character byte) bool {
	return character >= 'a' && character <= 'z' ||
		character >= '0' && character <= '9'
}

func validEndpointPort(port string) bool {
	if port == "443" || port == "" || port[0] == '0' {
		return false
	}
	for index := range len(port) {
		if port[index] < '0' || port[index] > '9' {
			return false
		}
	}
	value, err := strconv.ParseUint(port, 10, 16)
	return err == nil && value >= 1
}

func validEndpointPath(path string) bool {
	if path == "" {
		return true
	}
	if path[0] != '/' {
		return false
	}
	for _, segment := range strings.Split(path, "/") {
		if segment == "." || segment == ".." {
			return false
		}
		for index := range len(segment) {
			if !isEndpointPathByte(segment[index]) {
				return false
			}
		}
	}
	return true
}

func isEndpointPathByte(character byte) bool {
	return isEndpointDNSAlphanumeric(character) ||
		character >= 'A' && character <= 'Z' ||
		character == '-' || character == '.' || character == '_' || character == '~'
}
