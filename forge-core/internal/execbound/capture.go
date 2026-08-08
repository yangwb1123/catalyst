package execbound

import "time"

// waitDelay bounds how long Wait blocks AFTER the context is cancelled and the
// kill has been issued, before os/exec forcibly closes the command's I/O pipes
// so cmd.Run() returns. It is the LAST-RESORT backstop, not the primary
// mechanism: on unix the SIGKILL-to-the-whole-group should already have reaped
// every process holding the pipe; on non-unix it is the ONLY protection against
// a grandchild that inherited the pipe and survives the direct-child kill. The
// grace covers the residual race where a just-forked grandchild inherited the
// stdout/stderr pipe but had not yet been signalled — Wait would otherwise
// block until that writer closed the pipe on its own, which for a wedged
// grandchild is never. With WaitDelay, os/exec closes the pipes after this
// window and Run returns, so a tripped Timeout can never hang the caller. Kept
// short (the group is already dead in the normal case); only the pathological
// inherited-pipe race consumes it.
//
// Portable by design: set in COMMON code so the non-unix builds get the same
// pipe-close backstop (the orchestrator's original unix-only placement left
// Windows with no protection against the grandchild-pipe hang).
const waitDelay = 2 * time.Second

// cappedBuffer is an io.Writer that retains at most cap bytes of what is
// written, silently discarding the overflow — so a runaway command's UNBOUNDED
// stdout/stderr cannot OOM the host the way an unbounded CombinedOutput would.
// It tracks the TOTAL bytes seen so truncation is reported honestly. Write
// never errors or short-writes (a short write would make os/exec treat the
// pipe as broken and could wedge the child mid-stream); it lets the command
// run to its natural end (or the deadline) while simply not retaining the
// excess. Used as BOTH cmd.Stdout and cmd.Stderr (the same pointer), os/exec
// serializes the writes, so no lock is needed.
type cappedBuffer struct {
	cap   int
	buf   []byte
	total int
}

func (b *cappedBuffer) Write(p []byte) (int, error) {
	b.total += len(p)
	if room := b.cap - len(b.buf); room > 0 {
		if len(p) <= room {
			b.buf = append(b.buf, p...)
		} else {
			b.buf = append(b.buf, p[:room]...)
		}
	}
	return len(p), nil
}

