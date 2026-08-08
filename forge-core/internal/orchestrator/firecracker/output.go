package firecracker

import (
	"strings"
	"sync"

	"forgeos/forge-core/internal/orchestrator/sandbox"
)

// serialCapture is the host-side Firecracker stdout/stderr sink. It never
// writes guest serial output to disk, retains at most limit bytes in memory,
// and signals the runner as soon as the limit is crossed so the VM is stopped.
type serialCapture struct {
	mu       sync.Mutex
	limit    int
	buf      []byte
	total    int
	overflow chan struct{}
	once     sync.Once
}

func newSerialCapture(limit int) *serialCapture {
	if limit <= 0 {
		limit = sandbox.DefaultMaxOutputBytes
	}
	return &serialCapture{limit: limit, overflow: make(chan struct{})}
}

func (c *serialCapture) Write(p []byte) (int, error) {
	c.mu.Lock()
	c.total += len(p)
	if room := c.limit - len(c.buf); room > 0 {
		if len(p) < room {
			room = len(p)
		}
		c.buf = append(c.buf, p[:room]...)
	}
	overflow := c.total > c.limit
	c.mu.Unlock()
	if overflow {
		c.once.Do(func() { close(c.overflow) })
	}
	return len(p), nil
}

func (c *serialCapture) snapshot() (string, int) {
	c.mu.Lock()
	defer c.mu.Unlock()
	return string(append([]byte(nil), c.buf...)), c.total
}

func (c *serialCapture) limitError() error {
	_, total := c.snapshot()
	if total <= c.limit {
		return nil
	}
	return &sandbox.OutputLimitError{Limit: c.limit, Total: total}
}

// guestOutput extracts guest stdout from the bounded serial capture: the
// section between the FORGE-GUEST-START and FORGE-GUEST-DONE markers.
func guestOutput(log string) string {
	start := strings.Index(log, "FORGE-GUEST-START")
	if start < 0 {
		return ""
	}
	end := strings.Index(log[start:], "FORGE-GUEST-DONE")
	if end < 0 {
		end = len(log) - start
	}
	section := log[start+len("FORGE-GUEST-START") : start+end]
	lines := strings.Split(section, "\n")
	kept := make([]string, 0, len(lines))
	for _, line := range lines {
		// Strip ONLY a leading kernel timestamp ("[    0.123456] ..."),
		// never an arbitrary "] " inside guest output.
		kept = append(kept, stripKernelTimestamp(line))
	}
	return strings.TrimSpace(strings.Join(kept, "\n"))
}

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
