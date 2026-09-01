package firecracker

import (
	"math"
	"sync"

	"forgeos/forge-core/internal/orchestrator/sandbox"
)

const serialDiagnosticLimit = 1 << 20

// serialCapture is the host-side Firecracker stdout/stderr sink. It never
// writes guest serial output to disk, retains at most limit bytes in memory,
// and signals the runner as soon as the limit is crossed so the VM is stopped.
type serialCapture struct {
	mu            sync.Mutex
	limit         int
	buf           []byte
	total         int64
	countOverflow bool
	overflow      chan struct{}
	once          sync.Once
}

func newSerialCapture(limit int) *serialCapture {
	if limit <= 0 {
		limit = sandbox.DefaultMaxOutputBytes
	}
	return &serialCapture{limit: limit, overflow: make(chan struct{})}
}

func (c *serialCapture) Write(p []byte) (int, error) {
	c.mu.Lock()
	if c.countOverflow || c.total > math.MaxInt64-int64(len(p)) {
		c.total = math.MaxInt64
		c.countOverflow = true
	} else {
		c.total += int64(len(p))
	}
	if room := c.limit - len(c.buf); room > 0 {
		if len(p) < room {
			room = len(p)
		}
		c.buf = append(c.buf, p[:room]...)
	}
	overflow := c.countOverflow || c.total > int64(c.limit)
	c.mu.Unlock()
	if overflow {
		c.once.Do(func() { close(c.overflow) })
	}
	return len(p), nil
}

func (c *serialCapture) snapshot() (string, int64, bool) {
	c.mu.Lock()
	defer c.mu.Unlock()
	return string(append([]byte(nil), c.buf...)), c.total, c.countOverflow
}

func (c *serialCapture) limitError() error {
	_, total, countOverflow := c.snapshot()
	if !countOverflow && total <= int64(c.limit) {
		return nil
	}
	return &sandbox.OutputLimitError{
		Limit: int64(c.limit), Total: total, CountOverflow: countOverflow,
	}
}
