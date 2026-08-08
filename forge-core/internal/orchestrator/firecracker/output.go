package firecracker

import "strings"

// stripKernelTimestamp removes a leading Linux kernel timestamp prefix;
// anything else is returned unchanged.
func stripKernelTimestamp(line string) string {
	trimmed := strings.TrimSpace(line)
	rest, ok := strings.CutPrefix(trimmed, "[")
	if !ok {
		return line
	}
	end := strings.Index(rest, "]")
	if end <= 0 {
		return line
	}
	stamp := strings.TrimSpace(rest[:end])
	if !isKernelTimestamp(stamp) {
		return line
	}
	return strings.TrimSpace(rest[end+1:])
}

// isKernelTimestamp reports whether stamp looks like a kernel time
// (digits with an optional decimal fraction), e.g. "0.000000" or "12.5".
func isKernelTimestamp(stamp string) bool {
	for _, part := range strings.Split(stamp, ".") {
		if part == "" {
			return false
		}
		for _, r := range part {
			if r < '0' || r > '9' {
				return false
			}
		}
	}
	return true
}
