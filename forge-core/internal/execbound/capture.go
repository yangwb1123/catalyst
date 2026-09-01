package execbound

import (
	"math"
	"sync"
	"time"
)

// waitDelay bounds how long process wait and independently controlled raw
// output drains may continue after cancellation or direct-process exit before
// the drain controller closes its readers. It is the LAST-RESORT backstop, not the primary
// mechanism: on supported Linux hosts, SIGKILL-to-the-whole-group normally
// stops processes that did not escape that group; it is not containment. On
// other hosts it is the only protection against a grandchild that inherited
// the pipe and survives the direct-child kill. The
// grace covers the residual race where a just-forked grandchild inherited the
// stdout/stderr pipe but had not yet been signalled — Wait would otherwise
// block until that writer closed the pipe on its own, which for a wedged
// grandchild is never. After this window Run closes the readers and returns,
// so a tripped Timeout cannot hang on inherited output pipes. Kept
// short (the group is already dead in the normal case); only the pathological
// inherited-pipe race consumes it.
//
// Portable by design: set in common code so every build gets the same
// pipe-close backstop.
const waitDelay = 2 * time.Second

// cappedBuffer is an io.Writer that retains at most cap bytes of what is
// written, silently discarding the overflow — so a runaway command's UNBOUNDED
// stdout/stderr cannot OOM the host the way an unbounded CombinedOutput would.
// It tracks the TOTAL bytes seen so truncation is reported honestly. Write
// never errors or short-writes (a short write would make os/exec treat the
// pipe as broken and could wedge the child mid-stream); it lets the command
// run to its natural end (or the deadline) while simply not retaining the
// excess. CaptureCombined drains one shared raw pipe into this buffer, so the
// child's write order is retained without scheduler-dependent cross-drain
// interleaving. The mutex keeps cappedBuffer safe as a general io.Writer.
type cappedBuffer struct {
	mu            sync.Mutex
	cap           int
	buf           []byte
	total         int64
	countOverflow bool
}

func (b *cappedBuffer) Write(p []byte) (int, error) {
	b.mu.Lock()
	defer b.mu.Unlock()
	b.total, b.countOverflow = addByteCount(b.total, len(p), b.countOverflow)
	if room := b.cap - len(b.buf); room > 0 {
		if len(p) <= room {
			b.buf = append(b.buf, p...)
		} else {
			b.buf = append(b.buf, p[:room]...)
		}
	}
	return len(p), nil
}

func addByteCount(total int64, added int, overflow bool) (int64, bool) {
	if overflow || added < 0 || total > math.MaxInt64-int64(added) {
		return math.MaxInt64, true
	}
	return total + int64(added), false
}

func combinedByteCount(left, right *cappedBuffer) (int64, bool) {
	if left.countOverflow || right.countOverflow || left.total > math.MaxInt64-right.total {
		return math.MaxInt64, true
	}
	return left.total + right.total, false
}
